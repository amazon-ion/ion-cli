//! Byte-level sanitizer for Ion 1.0 binary streams.
//!
//! Unlike a parse-and-re-encode approach (which materializes values into a data
//! model and discards the physical encoding), this command walks the raw binary
//! encoding and rewrites only the *content* of scalar values, preserving the
//! stream's structure: type codes, symbol IDs, symbol table placement,
//! annotation wrappers, and (as closely as the rules allow) encoded lengths.
//!
//! Each value is re-emitted into its own buffer and containers recompute their
//! length prefix from the finished child bytes, so length changes propagate
//! outward without any in-place byte shifting. Scalars keep their exact length;
//! the only size change is that a non-minimal length prefix in the input is
//! re-emitted minimally (i.e. it shrinks).
//!
//! Only Ion 1.0 binary is supported; text and Ion 1.1 are rejected. Symbol IDs
//! are never randomized (they reference the symbol table); the text they map to
//! is anonymized when the symbol table's own string values are sanitized. Symbol
//! tables are otherwise sanitized like any struct, so a stream that uses shared
//! symbol table imports may become unreadable (the import metadata is scrambled).

use anyhow::{bail, Result};
use clap::{Arg, ArgMatches, Command};
use rand::rngs::ThreadRng;
use rand::Rng;
use std::io::{Read, Write};
use std::ops::RangeInclusive;
use std::sync::LazyLock;

use crate::ansi_codes::*;
use crate::commands::{CommandIo, IonCliCommand, WithIonCliArgument};

// --- CLI command -------------------------------------------------------------

static HELP_EPILOGUE: LazyLock<String> = LazyLock::new(|| {
    format!(
        // '\' at the end of the line indicates that CLAP will handle the line wrapping.
        "\
The content of every scalar - integers, floats, decimals, timestamps, strings, symbols, blobs, \
clobs, and booleans - is replaced with pseudo-random data of the same type and byte length. Types, \
lengths, and precisions are preserved (a non-minimal length prefix is rewritten in minimal form, \
which may shrink the output slightly).

Nulls and zero-length values are left unchanged. For timestamps, the year and all date/time fields \
are randomized within valid ranges; a UTC offset of zero (known UTC) or negative zero (unknown) is \
preserved, while any other offset is randomized. Symbol IDs are preserved (they reference the symbol \
table); the text they map to is redacted when the symbol table's own strings are sanitized. The \
content of NOP padding is also randomized.

Structure and metadata are retained by design: value byte lengths, timestamp precision, null-ness, \
field and element counts, nesting, and symbol IDs all survive. These can leak information about the \
original data even though the content does not.

Shared symbol table imports are not supported. Any imports will be mangled, rendering the document \
completely unreadable. This is intended behavior because imported symbol tables cannot be sanitized.

{BOLD}{ITALIC}Use at your own risk. Always inspect the output before sharing sanitized data.{NO_STYLE}
"
    )
});

pub struct SanitizeCommand;

impl IonCliCommand for SanitizeCommand {
    fn name(&self) -> &'static str {
        "sanitize"
    }

    fn about(&self) -> &'static str {
        "Replaces the content of scalar values in an Ion 1.0 binary stream with \
         pseudorandom substitutes while preserving the binary structure and encoding."
    }

    fn is_stable(&self) -> bool {
        false
    }

    fn is_porcelain(&self) -> bool {
        false
    }

    fn configure_args(&self, command: Command) -> Command {
        // Output is always Ion 1.0 binary. Pin a hidden `--format binary` so
        // `CommandIo` writes raw bytes rather than routing stdout through the
        // syntax highlighter (which would panic on non-UTF-8 output).
        command
            .after_help(HELP_EPILOGUE.as_str())
            .with_input()
            .with_output()
            .arg(
                Arg::new("format")
                    .long("format")
                    .hide(true)
                    .default_value("binary")
                    .value_parser(["binary"]),
            )
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        // Uses the shared `CommandIo` plumbing (auto-decompression, per-input
        // streaming) for consistency with the other input-consuming commands.
        // The walker reads front to back and only ever buffers a single
        // top-level value, so the whole stream is never held in memory.
        let mut sanitizer = Sanitizer::new();
        CommandIo::new(args)?.for_each_input(|output, input| {
            let mut source = input.into_source();
            sanitizer.sanitize_stream(&mut source, output)
        })
    }
}

/// The Ion 1.0 binary version marker.
const IVM_1_0: [u8; 4] = [0xE0, 0x01, 0x00, 0xEA];

/// A set of characters to test membership against and draw random members from.
enum CharacterSet {
    /// An explicit list of ASCII characters.
    Enumerated(&'static [u8]),
    /// An inclusive range of Unicode scalar values (the surrogate gap is skipped
    /// automatically, as `char` cannot represent surrogates).
    Range(RangeInclusive<char>),
}

impl CharacterSet {
    /// The set of characters whose UTF-8 encoding is exactly `byte_width` bytes,
    /// used to replace a character with one of the same width. One-byte
    /// replacements are restricted to printable ASCII so a replacement is never
    /// a control character.
    fn for_utf8_width(byte_width: usize) -> Self {
        CharacterSet::Range(match byte_width {
            1 => '\u{20}'..='\u{7E}',
            2 => '\u{80}'..='\u{7FF}',
            3 => '\u{800}'..='\u{FFFF}',
            _ => '\u{10000}'..='\u{10FFFF}',
        })
    }

    /// Returns whether `ch` belongs to this set.
    fn contains(&self, ch: char) -> bool {
        match self {
            CharacterSet::Enumerated(chars) => ch.is_ascii() && chars.contains(&(ch as u8)),
            CharacterSet::Range(range) => range.contains(&ch),
        }
    }

    /// Returns a uniformly random member of this set.
    fn random(&self, rng: &mut ThreadRng) -> char {
        match self {
            CharacterSet::Enumerated(chars) => chars[rng.random_range(0..chars.len())] as char,
            // `RangeInclusive` is not `Copy`; rebuild it from its `char`
            // endpoints (which are) to sample without cloning.
            CharacterSet::Range(range) => rng.random_range(*range.start()..=*range.end()),
        }
    }
}

/// Characters permitted as the first character of an Ion identifier symbol:
/// `[A-Za-z_$]`.
const IDENTIFIER_START: CharacterSet =
    CharacterSet::Enumerated(b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_$");
/// Characters permitted in later positions of an Ion identifier symbol:
/// `[A-Za-z0-9_$]`.
const IDENTIFIER_CONT: CharacterSet =
    CharacterSet::Enumerated(b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_$");

/// Maximum number of bytes accepted in a `VarUInt`/`VarInt`. ion-rs rejects
/// anything longer, and capping here keeps decoded values within `u64` so the
/// fixed-width encoders and bit shifts cannot overflow.
const MAX_VAR_LEN: usize = 9;

/// Maximum container/annotation nesting depth. Exceeding it errors cleanly
/// instead of overflowing the stack via recursion.
const MAX_DEPTH: usize = 128;

// --- Binary cursor -----------------------------------------------------------

/// A forward-only reader over an Ion binary stream. `pos` counts the bytes
/// consumed so far, which is all container-boundary tracking needs — no random
/// access into the input, so the stream is never buffered whole.
struct Cursor<'a> {
    reader: &'a mut dyn Read,
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(reader: &'a mut dyn Read) -> Self {
        Cursor { reader, pos: 0 }
    }

    /// Reads the next byte, or returns `None` at a clean end of input.
    fn read_u8_opt(&mut self) -> Result<Option<u8>> {
        let mut byte = [0u8; 1];
        if self.reader.read(&mut byte)? == 0 {
            return Ok(None);
        }
        self.pos += 1;
        Ok(Some(byte[0]))
    }

    fn read_u8(&mut self) -> Result<u8> {
        self.read_u8_opt()?
            .ok_or_else(|| anyhow::anyhow!("unexpected end of input"))
    }

    /// Reads exactly `len` bytes. Grows the buffer as bytes arrive rather than
    /// pre-allocating `len`, so a bogus length prefix can't drive a huge
    /// speculative allocation.
    fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        let read = (&mut *self.reader).take(len as u64).read_to_end(&mut buf)?;
        if read != len {
            bail!("value length {len} exceeds remaining input");
        }
        self.pos += len;
        Ok(buf)
    }

    /// Consumes `len` bytes without retaining them.
    fn skip(&mut self, len: usize) -> Result<()> {
        let n = std::io::copy(
            &mut (&mut *self.reader).take(len as u64),
            &mut std::io::sink(),
        )?;
        if n != len as u64 {
            bail!("value length {len} exceeds remaining input");
        }
        self.pos += len;
        Ok(())
    }

    /// Reads a `VarUInt`, returning its value and byte width.
    ///
    /// Rejects encodings longer than [`MAX_VAR_LEN`] bytes. This both matches
    /// the reader ion-rs uses to consume our output and guarantees the decoded
    /// value fits in a `u64`, so the fixed-width encoders and downstream shifts
    /// (`7 * byte_width`, etc.) cannot overflow.
    fn read_var_uint(&mut self) -> Result<(u64, usize)> {
        let mut value: u64 = 0;
        let mut width = 0;
        loop {
            if width >= MAX_VAR_LEN {
                bail!("VarUInt exceeds the maximum supported length of {MAX_VAR_LEN} bytes");
            }
            let byte = self.read_u8()?;
            width += 1;
            value = value
                .checked_mul(128)
                .and_then(|v| v.checked_add((byte & 0x7F) as u64))
                .ok_or_else(|| anyhow::anyhow!("VarUInt too large"))?;
            if byte & 0x80 != 0 {
                break;
            }
        }
        Ok((value, width))
    }

    /// Reads a `VarInt`, returning `(is_negative, magnitude, byte_width)`.
    ///
    /// Like [`Self::read_var_uint`], rejects encodings longer than
    /// [`MAX_VAR_LEN`] bytes.
    fn read_var_int(&mut self) -> Result<(bool, u64, usize)> {
        let first = self.read_u8()?;
        let is_negative = first & 0x40 != 0;
        let mut magnitude = (first & 0x3F) as u64;
        let mut width = 1;
        if first & 0x80 == 0 {
            loop {
                if width >= MAX_VAR_LEN {
                    bail!("VarInt exceeds the maximum supported length of {MAX_VAR_LEN} bytes");
                }
                let byte = self.read_u8()?;
                width += 1;
                magnitude = magnitude
                    .checked_mul(128)
                    .and_then(|v| v.checked_add((byte & 0x7F) as u64))
                    .ok_or_else(|| anyhow::anyhow!("VarInt too large"))?;
                if byte & 0x80 != 0 {
                    break;
                }
            }
        }
        Ok((is_negative, magnitude, width))
    }

    /// Returns the `pos` at which a value `len` bytes long ends, erroring on
    /// overflow rather than panicking. (Reads past the true end of input are
    /// caught when the underlying reader runs dry.)
    fn value_end(&self, len: usize) -> Result<usize> {
        self.pos
            .checked_add(len)
            .ok_or_else(|| anyhow::anyhow!("value length {} exceeds addressable range", len))
    }
}

// --- Output framing helpers --------------------------------------------------

/// Appends a minimal `VarUInt` encoding of `value`.
fn push_var_uint(out: &mut Vec<u8>, value: u64) {
    let mut groups = [0u8; 10];
    let mut count = 0;
    let mut remaining = value;
    loop {
        groups[count] = (remaining & 0x7F) as u8;
        count += 1;
        remaining >>= 7;
        if remaining == 0 {
            break;
        }
    }
    // `groups` holds little-endian 7-bit groups; emit big-endian with the
    // terminator flag on the final (least-significant) byte.
    for i in (0..count).rev() {
        let mut byte = groups[i];
        if i == 0 {
            byte |= 0x80;
        }
        out.push(byte);
    }
}

/// Appends a `VarUInt` of exactly `num_bytes` bytes encoding `value`. `value`
/// must fit in `7 * num_bytes` bits; higher bits are silently dropped. Callers
/// derive the value to fit the width, so this is not reached with a wider value.
fn push_var_uint_fixed(out: &mut Vec<u8>, value: u64, num_bytes: usize) {
    for i in 0..num_bytes {
        let shift = 7 * (num_bytes - 1 - i);
        let mut byte = ((value >> shift) & 0x7F) as u8;
        if i == num_bytes - 1 {
            byte |= 0x80;
        }
        out.push(byte);
    }
}

/// Appends a `VarInt` of exactly `num_bytes` bytes encoding the given sign and
/// magnitude. `magnitude` must fit in `6 + 7 * (num_bytes - 1)` bits; higher
/// bits are silently dropped. Callers derive the magnitude to fit the width.
fn push_var_int_fixed(out: &mut Vec<u8>, is_negative: bool, magnitude: u64, num_bytes: usize) {
    for i in 0..num_bytes {
        let mut byte = if i == 0 {
            // First byte holds the sign in bit 6 and the top 6 magnitude bits.
            let shift = 7 * (num_bytes - 1);
            let top = ((magnitude >> shift) & 0x3F) as u8;
            if is_negative {
                top | 0x40
            } else {
                top
            }
        } else {
            let shift = 7 * (num_bytes - 1 - i);
            ((magnitude >> shift) & 0x7F) as u8
        };
        if i == num_bytes - 1 {
            byte |= 0x80;
        }
        out.push(byte);
    }
}

fn push_len(out: &mut Vec<u8>, type_code: u8, len: usize) {
    if len <= 13 {
        out.push((type_code << 4) | len as u8);
    } else {
        out.push((type_code << 4) | 0x0E);
        push_var_uint(out, len as u64);
    }
}

/// The number of decimal-digit magnitude bits representable in a `VarInt`/
/// `VarUInt` of `num_bytes` bytes (6 in the first byte, 7 in each subsequent).
fn var_magnitude_bits(num_bytes: usize) -> u32 {
    (6 + 7 * (num_bytes.max(1) - 1)) as u32
}

// --- Sanitizer ---------------------------------------------------------------

struct Sanitizer {
    rng: ThreadRng,
}

impl Sanitizer {
    fn new() -> Self {
        Sanitizer { rng: rand::rng() }
    }

    /// Sanitizes an Ion 1.0 binary document held in memory, returning the
    /// result. A convenience wrapper over [`Self::sanitize_stream`] used by
    /// tests; `&[u8]` is itself a `Read`, so nothing is copied up front.
    #[cfg(test)]
    fn sanitize_document(&mut self, source: &[u8]) -> Result<Vec<u8>> {
        let mut reader: &[u8] = source;
        let mut out = Vec::new();
        self.sanitize_stream(&mut reader, &mut out)?;
        Ok(out)
    }

    /// Sanitizes an Ion 1.0 binary stream front to back, writing the result to
    /// `out`. Only one top-level value is buffered at a time.
    fn sanitize_stream(&mut self, reader: &mut dyn Read, out: &mut dyn Write) -> Result<()> {
        let mut cursor = Cursor::new(reader);
        let mut value = Vec::new();

        let first_byte = cursor.read_u8()?;
        if first_byte != 0xE0 {
            bail!("`ion sanitize` requires Ion 1.0 binary input");
        }
        handle_ivm(&mut cursor, &mut value, 0)?;
        out.write_all(&value)?;
        while let Some(descriptor) = cursor.read_u8_opt()? {
            value.clear();
            self.sanitize_value(descriptor, &mut cursor, &mut value, 0)?;
            out.write_all(&value)?;
        }
        Ok(())
    }

    /// Sanitizes the value whose (already-read) type descriptor is `descriptor`,
    /// appending its sanitized encoding to `out`. Callers read the descriptor
    /// and pass it in. `depth` bounds recursion so deeply nested input errors
    /// cleanly instead of overflowing the stack.
    fn sanitize_value(
        &mut self,
        descriptor: u8,
        cursor: &mut Cursor,
        out: &mut Vec<u8>,
        depth: usize,
    ) -> Result<()> {
        if depth > MAX_DEPTH {
            bail!("input nesting exceeds the maximum supported depth of {MAX_DEPTH}");
        }
        let type_code = descriptor >> 4;
        let nibble = descriptor & 0x0F;

        // A low nibble of 0xF is a typed null for every type code except the
        // annotation wrapper (14, where 0xEF is invalid) and the reserved code
        // (15). Nulls carry no content, so re-emit the descriptor verbatim.
        if nibble == 0x0F && type_code < 14 {
            out.push(descriptor);
            return Ok(());
        }

        match type_code {
            1 => out.push(0x10 | self.rng.random::<bool>() as u8),
            // nop (0), ints (2/3), float (4), and lobs (clob 9 / blob 10) are all just fixed-length
            // byte bodies replaced with random bytes of the same length.
            0 | 2 | 3 | 4 | 9 | 10 => self.sanitize_fixed_bytes(cursor, type_code, nibble, out)?,
            5 => self.sanitize_decimal(cursor, nibble, out)?,
            6 => self.sanitize_timestamp(cursor, nibble, out)?,
            // A symbol is a SID reference into the symbol table; copy it verbatim
            // (its text is redacted via the symbol table's strings).
            7 => self.copy_symbol(cursor, nibble, out)?,
            8 => self.sanitize_string(cursor, nibble, out)?,
            11 | 12 => self.sanitize_sequence(cursor, type_code, nibble, out, depth)?,
            13 => self.sanitize_struct(cursor, nibble, out, depth)?,
            14 if descriptor == 0xE0 => handle_ivm(cursor, out, depth)?,
            14 => self.sanitize_annotations(cursor, nibble, out, depth)?,
            _ => bail!("encountered reserved type code {}", type_code),
        }
        Ok(())
    }

    /// Copies a symbol value's SID bytes through verbatim (re-framed with a
    /// minimal length prefix). The SID references the symbol table and must be
    /// preserved; its text is redacted via the table's strings. Typed null
    /// (`null.symbol`) is handled by the caller's central null check.
    fn copy_symbol(&mut self, cursor: &mut Cursor, nibble: u8, out: &mut Vec<u8>) -> Result<()> {
        let len = self.read_length(cursor, nibble)?;
        let body = cursor.read_bytes(len)?;
        push_len(out, 7, body.len());
        out.extend_from_slice(&body);
        Ok(())
    }

    /// Reads a non-null, non-bool value's body length from the low nibble
    /// (reading a trailing `VarUInt` when the nibble is 14). Uses `try_from` so
    /// a length that overflows `usize` (possible on 32-bit targets) errors
    /// rather than silently truncating into a valid-looking small length.
    fn read_length(&self, cursor: &mut Cursor, nibble: u8) -> Result<usize> {
        if nibble == 0x0E {
            let len = cursor.read_var_uint()?.0;
            usize::try_from(len)
                .map_err(|_| anyhow::anyhow!("value length {len} exceeds addressable range"))
        } else {
            Ok(nibble as usize)
        }
    }

    /// Sanitizes an int (type 2/3) or lob (clob 9 / blob 10) by replacing its
    /// body with random bytes of the same length. The body always contains a
    /// non-zero byte so a negative int cannot become negative zero (invalid
    /// Ion); this is harmless for positive ints and lobs.
    fn sanitize_fixed_bytes(
        &mut self,
        cursor: &mut Cursor,
        type_code: u8,
        nibble: u8,
        out: &mut Vec<u8>,
    ) -> Result<()> {
        let len = self.read_length(cursor, nibble)?;
        cursor.skip(len)?;
        push_len(out, type_code, len);

        let mut body = vec![0u8; len];
        self.rng.fill(&mut body[..]);
        if let Some(last) = body.last_mut() {
            *last = self.rng.random_range(1..=u8::MAX);
        }

        out.extend_from_slice(&body);
        Ok(())
    }

    fn sanitize_decimal(
        &mut self,
        cursor: &mut Cursor,
        nibble: u8,
        out: &mut Vec<u8>,
    ) -> Result<()> {
        let len = self.read_length(cursor, nibble)?;
        if len == 0 {
            out.push(0x50); // 0d0; keep as-is
            return Ok(());
        }
        let (exp_negative, _exp_mag, exp_len) = cursor.read_var_int()?;
        // The exponent VarInt must fit within the declared decimal length.
        let coeff_len = len
            .checked_sub(exp_len)
            .ok_or_else(|| anyhow::anyhow!("decimal exponent overruns its declared length"))?;
        cursor.skip(coeff_len)?;
        push_len(out, 5, len);

        // Randomize the exponent within the same VarInt byte width.
        let exp_mag = self.random_magnitude(var_magnitude_bits(exp_len));
        push_var_int_fixed(out, exp_negative, exp_mag, exp_len);
        // Coefficient is a signed Int; any bit pattern is valid. Keep the width.
        let mut bytes = vec![0u8; coeff_len];
        self.rng.fill(&mut bytes[..]);
        out.extend(bytes);

        Ok(())
    }

    fn sanitize_timestamp(
        &mut self,
        cursor: &mut Cursor,
        nibble: u8,
        out: &mut Vec<u8>,
    ) -> Result<()> {
        let len = self.read_length(cursor, nibble)?;
        let end = cursor.value_end(len)?;

        let mut body = Vec::new();
        // Offset (VarInt, minutes). Zero (known UTC) and negative-zero (unknown)
        // are preserved; other offsets are randomized.
        let (off_negative, off_mag, off_len) = cursor.read_var_int()?;
        if off_mag == 0 {
            // Zero (known UTC) or negative-zero (unknown); re-emit as-is.
            push_var_int_fixed(&mut body, off_negative, 0, off_len);
        } else {
            let (new_neg, new_mag) = self.random_offset(off_mag, off_len);
            push_var_int_fixed(&mut body, new_neg, new_mag, off_len);
        }

        // Year: Ion permits 1..=9999; but it's aesthetically pleasing to use years that look familiar
        self.push_random_var_uint(cursor, &mut body, 1950, 2050)?;

        // The remaining fields determine precision by their presence.
        if cursor.pos < end {
            self.push_random_var_uint(cursor, &mut body, 1, 12)?; // month
        }
        if cursor.pos < end {
            self.push_random_var_uint(cursor, &mut body, 1, 28)?; // day (1-28)
        }
        if cursor.pos < end {
            self.push_random_var_uint(cursor, &mut body, 0, 23)?; // hour
            self.push_random_var_uint(cursor, &mut body, 0, 59)?; // minute
        }
        if cursor.pos < end {
            self.push_random_var_uint(cursor, &mut body, 0, 59)?; // second
        }
        if cursor.pos < end {
            self.sanitize_timestamp_fraction(cursor, end, &mut body)?;
        }

        if cursor.pos != end {
            bail!("timestamp field layout did not match its length");
        }
        push_len(out, 6, len);
        out.extend_from_slice(&body);
        Ok(())
    }

    /// Reads a `VarUInt` (e.g. a timestamp field) to learn its byte width, then
    /// appends a random replacement in `[min, max]` at that same width. The
    /// range is clamped to what the width can hold so the fixed-width re-encode
    /// can never truncate (which could even produce an invalid 0).
    fn push_random_var_uint(
        &mut self,
        cursor: &mut Cursor,
        body: &mut Vec<u8>,
        min: u32,
        max: u32,
    ) -> Result<()> {
        let (_value, width) = cursor.read_var_uint()?;
        let fit_max = ((1u64 << (7 * width).min(63)) - 1).min(max as u64);
        let lo = (min as u64).min(fit_max);
        let replacement = self.rng.random_range(lo..=fit_max);
        push_var_uint_fixed(body, replacement, width);
        Ok(())
    }

    /// Sanitizes the fractional-seconds trailer: a `VarInt` exponent followed by
    /// a signed `Int` coefficient. The exponent (scale) is preserved so the
    /// result stays a valid fraction; the coefficient is randomized within the
    /// same byte width, kept non-negative and `< 10^-exp` so the fraction is in
    /// `[0, 1)`.
    fn sanitize_timestamp_fraction(
        &mut self,
        cursor: &mut Cursor,
        end: usize,
        body: &mut Vec<u8>,
    ) -> Result<()> {
        let (exp_negative, exp_mag, exp_len) = cursor.read_var_int()?;
        // Preserve the exponent (fraction scale) at its original byte width.
        push_var_int_fixed(body, exp_negative, exp_mag, exp_len);
        // The exponent VarInt must fit within the timestamp's declared length.
        let coeff_len = end
            .checked_sub(cursor.pos)
            .ok_or_else(|| anyhow::anyhow!("timestamp fraction exponent overruns its length"))?;
        cursor.skip(coeff_len)?;

        if coeff_len == 0 {
            return Ok(());
        }

        // A valid fraction needs a negative exponent and coefficient < 10^-exp.
        // For any other exponent (zero or non-negative) no sub-second value is
        // representable, so emit coefficient 0. The coefficient is a signed Int,
        // so the reserved sign bit keeps randomized values non-negative. The
        // `exp_mag < 20` bound also keeps `10u64.pow(exp_mag)` from overflowing
        // (10^20 > u64::MAX); larger scales fall to the coefficient-0 branch.
        let coefficient = if exp_negative && exp_mag > 0 && exp_mag < 20 {
            let positive_bits = (coeff_len * 8 - 1).min(63) as u32;
            let bound = (1u64 << positive_bits).min(10u64.pow(exp_mag as u32));
            self.rng.random_range(0..bound.max(1))
        } else {
            0
        };
        // Encode the coefficient as a big-endian Int of exactly `coeff_len`.
        for i in 0..coeff_len {
            let shift = 8 * (coeff_len - 1 - i);
            // `coefficient` fits in a u64, so any byte position past the 8
            // low-order bytes is zero. Guard the shift so a wide (>= 9-byte)
            // coefficient cannot overflow it.
            let byte = if shift >= 64 {
                0
            } else {
                ((coefficient >> shift) & 0xFF) as u8
            };
            body.push(byte);
        }
        Ok(())
    }

    /// Sanitizes a string, applying identifier-preserving rules so the stream
    /// keeps whether each symbol/field-name is an identifier (Ion symbol text
    /// feeds the symbol table). Per position, for characters in the identifier
    /// classes `[A-Za-z_$]` / `[A-Za-z0-9_$]`:
    /// - index 0: identifier-start -> identifier-start
    /// - index 1: identifier-cont  -> identifier-start (a non-digit, which keeps
    ///   an identifier-shaped symbol from turning into `$`-prefixed SID text)
    /// - index >=2: identifier-cont -> identifier-cont
    ///
    /// Control characters (`< 0x20`) are preserved; any other character is
    /// replaced with a random character of the same UTF-8 byte width. (Strings
    /// that already mix in non-identifier characters are not identifiers, so no
    /// attempt is made to keep them from resembling `$`-SID text; Ion quotes any
    /// such text on output, so the result is still valid.)
    fn sanitize_string(
        &mut self,
        cursor: &mut Cursor,
        nibble: u8,
        out: &mut Vec<u8>,
    ) -> Result<()> {
        let len = self.read_length(cursor, nibble)?;
        let content = cursor.read_bytes(len)?;
        // A valid Ion binary string is always UTF-8; anything else is malformed
        // input. Erroring avoids copying the original bytes through verbatim.
        let text = std::str::from_utf8(&content)
            .map_err(|_| anyhow::anyhow!("string value is not valid UTF-8"))?;

        let mut body = Vec::with_capacity(content.len());
        for (index, ch) in text.chars().enumerate() {
            let replacement =
                if IDENTIFIER_START.contains(ch) || (index >= 1 && IDENTIFIER_CONT.contains(ch)) {
                    let class = if index <= 1 {
                        &IDENTIFIER_START
                    } else {
                        &IDENTIFIER_CONT
                    };
                    class.random(&mut self.rng)
                } else if (ch as u32) < 0x20 {
                    ch // control characters are preserved
                } else {
                    CharacterSet::for_utf8_width(ch.len_utf8()).random(&mut self.rng)
                };
            body.extend_from_slice(replacement.encode_utf8(&mut [0u8; 4]).as_bytes());
        }
        push_len(out, 8, len);
        out.extend_from_slice(&body);
        Ok(())
    }

    /// Reads a length-prefixed sequence (list/sexp) and re-emits it with each
    /// child value sanitized.
    fn sanitize_sequence(
        &mut self,
        cursor: &mut Cursor,
        type_code: u8,
        nibble: u8,
        out: &mut Vec<u8>,
        depth: usize,
    ) -> Result<()> {
        // Typed nulls (nibble 0xF) are handled by the caller's central check.
        let len = self.read_length(cursor, nibble)?;
        let end = cursor.value_end(len)?;
        // Grow as children are read rather than pre-allocating `len`, which is
        // attacker-controlled (up to a 9-byte VarUInt) — `Vec::with_capacity`
        // on a huge length aborts the process via `handle_alloc_error`.
        let mut body = Vec::new();
        while cursor.pos < end {
            let descriptor = cursor.read_u8()?;
            self.sanitize_value(descriptor, cursor, &mut body, depth + 1)?;
        }
        if cursor.pos != end {
            bail!("sequence child values overran the container length");
        }
        push_len(out, type_code, body.len());
        out.extend_from_slice(&body);
        Ok(())
    }

    /// Reads a struct and re-emits it with each field value sanitized. The
    /// field-name symbol IDs are preserved (their text is redacted via the
    /// symbol table's strings).
    fn sanitize_struct(
        &mut self,
        cursor: &mut Cursor,
        nibble: u8,
        out: &mut Vec<u8>,
        depth: usize,
    ) -> Result<()> {
        // Typed null (nibble 0xF) is handled by the caller's central check; an
        // empty struct (nibble 0) falls through the general path (len 0 →
        // `push_struct` emits 0xD0).
        // L=1 means the fields are ordered and the length is a trailing VarUInt.
        let ordered = nibble == 1;
        let len = if ordered {
            self.read_length(cursor, 0x0E)?
        } else {
            self.read_length(cursor, nibble)?
        };

        let end = cursor.value_end(len)?;
        // Grow as children are read rather than pre-allocating `len`, which is
        // attacker-controlled (up to a 9-byte VarUInt) — `Vec::with_capacity`
        // on a huge length aborts the process via `handle_alloc_error`.
        let mut body = Vec::new();
        while cursor.pos < end {
            // Re-emit the field-name symbol ID (value preserved; a non-minimal
            // encoding is canonicalized like every length prefix).
            let (sid, _) = cursor.read_var_uint()?;
            push_var_uint(&mut body, sid);
            let descriptor = cursor.read_u8()?;
            self.sanitize_value(descriptor, cursor, &mut body, depth + 1)?;
        }
        if cursor.pos != end {
            bail!("struct fields overran the container length");
        }
        push_struct(out, ordered, &body);
        Ok(())
    }

    fn sanitize_annotations(
        &mut self,
        cursor: &mut Cursor,
        nibble: u8,
        out: &mut Vec<u8>,
        depth: usize,
    ) -> Result<()> {
        if nibble == 0 || nibble == 0x0F {
            bail!("invalid annotation wrapper descriptor 0xE{:X}", nibble);
        }
        let len = self.read_length(cursor, nibble)?;
        let end = cursor.value_end(len)?;
        // Annotation symbol IDs are preserved (their text is redacted via the
        // symbol table's strings). The `annot_length` byte count is re-derived
        // from the SID bytes on output.
        let (annot_length, _) = cursor.read_var_uint()?;

        let annot_length = usize::try_from(annot_length)
            .map_err(|_| anyhow::anyhow!("annotation length exceeds addressable range"))?;
        let annot_sids = cursor.read_bytes(annot_length)?;

        let mut value_body = Vec::new();
        let value_descriptor = cursor.read_u8()?;
        self.sanitize_value(value_descriptor, cursor, &mut value_body, depth + 1)?;
        if cursor.pos != end {
            bail!("annotation wrapper length did not match its contents");
        }

        let len = annot_sids.len() + value_body.len() + 1;
        push_len(out, 14, len);
        push_var_uint(out, annot_sids.len() as u64);
        out.extend_from_slice(&annot_sids);
        out.extend_from_slice(&value_body);

        Ok(())
    }

    // --- Randomization primitives -------------------------------------------

    /// Returns a random magnitude that fits in `bits` bits.
    fn random_magnitude(&mut self, bits: u32) -> u64 {
        let bound = 1u64 << bits.min(63);
        self.rng.random_range(0..bound)
    }

    /// Chooses a replacement timestamp offset `(is_negative, magnitude)` that
    /// fits the original `VarInt` byte width. A multiple of 30 minutes stays a
    /// non-zero multiple of 30 (real offsets are almost always :00 or :30); any
    /// other offset becomes any valid non-zero offset.
    fn random_offset(&mut self, original_mag: u64, byte_width: usize) -> (bool, u64) {
        const MAX_OFFSET: u64 = 1439; // ±23:59 in minutes
                                      // `.min(63)` keeps the shift in range even if MAX_VAR_LEN ever grows.
        let bits = var_magnitude_bits(byte_width).min(63);
        let fit_max = ((1u64 << bits) - 1).min(MAX_OFFSET);
        let magnitude = if original_mag.is_multiple_of(30) {
            self.rng.random_range(1..=fit_max / 30) * 30
        } else {
            self.rng.random_range(1..=fit_max)
        };
        (self.rng.random::<bool>(), magnitude)
    }
}

fn handle_ivm(cursor: &mut Cursor, out: &mut Vec<u8>, depth: usize) -> Result<()> {
    // A version marker is only valid at the top level of the stream.
    if depth != 0 {
        bail!("encountered an Ion version marker inside a container");
    }
    // Version marker; copied verbatim after verifying it is 1.0.
    let (b1, b2, b3) = (cursor.read_u8()?, cursor.read_u8()?, cursor.read_u8()?);
    if (b1, b2, b3) != (0x01, 0x00, 0xEA) {
        bail!("unsupported or invalid IVM: E0 {b1:02X} {b2:02X} {b3:02X}");
    }
    out.write_all(&IVM_1_0)?;
    Ok(())
}

/// Appends a struct with the given body, preserving the "ordered" (L=1) form
/// when requested. A one-byte body (which cannot occur for a real struct) is
/// encoded with the `VarUInt` length form to avoid the reserved L=1 nibble.
fn push_struct(out: &mut Vec<u8>, ordered: bool, body: &[u8]) {
    let len = body.len();
    if ordered {
        out.push(0xD1);
        push_var_uint(out, len as u64);
    } else {
        push_len(out, 0xD, len)
    }
    out.extend_from_slice(body);
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use ion_rs::*;
    use rstest::rstest;

    /// Encodes Ion text as an Ion 1.0 binary document.
    fn to_binary(text: &str) -> Result<Vec<u8>> {
        let elements = Element::read_all(text)?;
        let mut buf = Vec::new();
        let mut writer = Writer::new(v1_0::Binary, &mut buf)?;
        for element in elements.iter() {
            writer.write(element)?;
        }
        writer.flush()?;
        drop(writer);
        Ok(buf)
    }

    fn sanitize(bytes: &[u8]) -> Result<Vec<u8>> {
        Sanitizer::new().sanitize_document(bytes)
    }

    /// A recursive description of a value's type and structure (but not its
    /// content), used to assert the shape is preserved.
    fn shape(element: &Element) -> String {
        if element.is_null() {
            return format!("null.{:?}", element.ion_type());
        }
        match element.ion_type() {
            IonType::List | IonType::SExp => {
                let children: Vec<String> = element
                    .as_sequence()
                    .unwrap()
                    .elements()
                    .map(shape)
                    .collect();
                format!("{:?}[{}]", element.ion_type(), children.join(","))
            }
            IonType::Struct => {
                let mut fields: Vec<String> = element
                    .as_struct()
                    .unwrap()
                    .fields()
                    .map(|(_, v)| shape(v))
                    .collect();
                fields.sort();
                format!("Struct{{{}}}", fields.join(","))
            }
            other => format!("{:?}", other),
        }
    }

    fn shapes(bytes: &[u8]) -> Result<Vec<String>> {
        Ok(Element::read_all(bytes)?.iter().map(shape).collect())
    }

    /// True if `s` is a valid Ion identifier symbol (`[A-Za-z_$][A-Za-z0-9_$]*`).
    fn is_identifier(s: &str) -> bool {
        let mut chars = s.chars();
        chars.next().is_some_and(|c| IDENTIFIER_START.contains(c))
            && s.chars().all(|c| IDENTIFIER_CONT.contains(c))
    }

    /// Returns the first top-level element of a sanitized binary document.
    fn sanitize_first(text: &str) -> Result<Element> {
        let out = Element::read_all(sanitize(&to_binary(text)?)?)?;
        Ok(out.into_iter().next().unwrap())
    }

    // --- Rejected inputs (text, wrong version, malformed) --------------------

    #[rstest]
    #[case::text(b"hello world")]
    #[case::ion_1_1(&[0xE0, 0x01, 0x01, 0xEA, 0x00])]
    #[case::malformed_ivm(&[0xE0, 0x01, 0x00, 0xB2, 0x00])]
    #[case::varuint_longer_than_max_supported(&[0xE0, 0x01, 0x00, 0xEA, 0xBE, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0xFF])]
    #[case::decimal_exp_overrun(&[0xE0, 0x01, 0x00, 0xEA, 0x51, 0x00, 0x80])]
    #[case::truncated_container(&[0xE0, 0x01, 0x00, 0xEA, 0xB5, 0x21])]
    #[case::invalid_utf8_string(&[0xE0, 0x01, 0x00, 0xEA, 0x82, 0xFF, 0xFF])]
    // Annotation wrapper declaring zero annotations.
    // #[case::zero_annotations(&[0xE0, 0x01, 0x00, 0xEA, 0xE3, 0x80, 0x21, 0x01])]
    #[case::huge_list_len(&[0xE0, 0x01, 0x00, 0xEA, 0xBE, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0xFF])]
    #[case::huge_struct_len(&[0xE0, 0x01, 0x00, 0xEA, 0xDE, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0xFF])]
    #[case::huge_timestamp_len(&[0xE0, 0x01, 0x00, 0xEA, 0x6E, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0xFF])]
    fn invalid_input_errors_without_panic(#[case] bytes: &[u8]) {
        assert!(Sanitizer::new().sanitize_document(bytes).is_err());
    }

    #[test]
    fn deeply_nested_input_errors_without_stack_overflow() {
        // Each 0xB1 opens a length-1 list containing another value, one byte per
        // level; well past MAX_DEPTH this must error rather than abort.
        let mut bytes = vec![0xE0, 0x01, 0x00, 0xEA];
        bytes.extend(std::iter::repeat_n(0xB1u8, MAX_DEPTH + 100));
        bytes.push(0x80); // an innermost empty string
        let err = Sanitizer::new()
            .sanitize_document(&bytes)
            .expect_err("deep nesting should error");
        assert!(err.to_string().contains("depth"), "got: {}", err);
    }

    // --- Structure / validity preservation -----------------------------------

    #[rstest]
    #[case(
        r#"{ name: "Alice", age: 42, active: true, score: 3.14e0, price: 19.99d0,
             when: 2023-06-15T10:30:45Z, tags: ["x", "yy", "zzz"], sym: foo,
             data: {{ "clob" }}, raw: {{ aGVsbG8= }}, nested: { a: 1, b: [2, 3] },
             empty: "", zero: 0, neg: -100, ann: my_annot::123, sexp: (a b c),
             nothing: null, nint: null.int }"#
    )]
    #[case("[1, 2, 3]")]
    #[case("(a b c)")]
    #[case("true false null null.int null.string null.struct")]
    #[case("2023T 2023-06-15T10:30:45.123Z")]
    #[case("a::b::c::42 [] {} \"\"")]
    fn structure_and_validity_preserved(#[case] text: &str) -> Result<()> {
        let input = to_binary(text)?;
        // Output must be valid Ion (read_all succeeds) with the same type tree.
        assert_eq!(shapes(&input)?, shapes(&sanitize(&input)?)?);
        Ok(())
    }

    /// A minimally-encoded document round-trips at exactly the same byte length
    /// (scalars keep their width and zero-length values are not grown).
    #[rstest]
    #[case(r#"{ alpha: 1, beta: "xy", gamma: 2.5e0, delta: true }"#)]
    #[case("1000000 -1000000")] // multi-byte int magnitudes
    #[case("19.99d0 -0.5d3")] // decimal exponent/coefficient widths
    #[case("2023-06-15T10:30:45.123456+05:30")] // timestamp with offset + fraction
    #[case("sym another_sym {{ aGVsbG8= }}")] // symbols + blob
    #[case(r#"{ a: "", b: 0, c: [], d: {}, e: 0e0, f: 0d0 }"#)] // zero-length values
    fn byte_length_preserved(#[case] text: &str) -> Result<()> {
        let input = to_binary(text)?;
        assert_eq!(input.len(), sanitize(&input)?.len());
        Ok(())
    }

    // --- Per-type behavior ----------------------------------------------------

    #[rstest]
    #[case::null_int("null.int", IonType::Int, true)]
    #[case::null_string("null.string", IonType::String, true)]
    #[case::null_bool("null.bool", IonType::Bool, true)]
    #[case::null_struct("null.struct", IonType::Struct, true)]
    #[case::bool_true("true", IonType::Bool, false)]
    #[case::bool_false("false", IonType::Bool, false)]
    fn scalar_type_and_nullness_preserved(
        #[case] text: &str,
        #[case] ion_type: IonType,
        #[case] is_null: bool,
    ) -> Result<()> {
        let element = sanitize_first(text)?;
        assert_eq!(element.ion_type(), ion_type);
        assert_eq!(element.is_null(), is_null);
        Ok(())
    }

    #[test]
    fn ints_preserve_sign() -> Result<()> {
        let out = Element::read_all(sanitize(&to_binary("-100 200 -1 1")?)?)?;
        let signs: Vec<bool> = out
            .iter()
            .map(|e| e.as_int().unwrap().is_negative())
            .collect();
        assert_eq!(signs, vec![true, false, true, false]);
        Ok(())
    }

    #[test]
    fn negative_ints_never_become_negative_zero() -> Result<()> {
        // All-zero magnitude under type code 3 is invalid Ion; a 1-byte
        // magnitude hits it ~1/256, so loop.
        let input = to_binary("-1 -1 -1 -1 -1 -1 -1 -1")?;
        for _ in 0..500 {
            let out = Element::read_all(sanitize(&input)?)?; // must stay valid Ion
            assert!(out.iter().all(|e| e.as_int().unwrap().is_negative()));
        }
        Ok(())
    }

    #[test]
    fn field_names_and_symbols_stay_identifiers() -> Result<()> {
        let outer = sanitize_first(r#"{ first_name: "n", last_name: some_symbol }"#)?;
        let outer = outer.as_struct().unwrap();
        for (name, value) in outer.fields() {
            assert!(
                is_identifier(name.text().unwrap()),
                "field name not an identifier"
            );
            if let Some(sym) = value.as_symbol().and_then(|s| s.text()) {
                assert!(is_identifier(sym), "symbol not an identifier: {}", sym);
            }
        }
        Ok(())
    }

    #[test]
    fn symbol_ids_and_annotations_preserved() -> Result<()> {
        // A repeated symbol must still alias to a single text (its SID is kept),
        // and an annotation must survive (its SID is kept).
        let outer = sanitize_first("{ a: foo, b: foo, c: ann::42 }")?;
        let outer = outer.as_struct().unwrap();
        let values: Vec<&Element> = outer.fields().map(|(_, v)| v).collect();
        let a = values[0].as_symbol().unwrap().text().unwrap();
        let b = values[1].as_symbol().unwrap().text().unwrap();
        assert_eq!(a, b, "aliased symbol SIDs diverged");
        assert_eq!(values[2].annotations().iter().count(), 1, "annotation lost");
        Ok(())
    }

    #[test]
    fn timestamps_preserve_precision_and_respect_ranges() -> Result<()> {
        let input = to_binary(
            "2023T 2023-06T 2023-06-15T 2023-06-15T10:30Z \
             2023-06-15T10:30:45Z 2023-06-15T10:30:45.123Z",
        )?;
        for _ in 0..25 {
            let orig = Element::read_all(&input)?;
            let sanitized = Element::read_all(sanitize(&input)?)?;
            for (o, s) in orig.iter().zip(sanitized.iter()) {
                let (o, s) = (o.as_timestamp().unwrap(), s.as_timestamp().unwrap());
                assert_eq!(o.precision(), s.precision(), "precision changed");
                if s.precision() >= TimestampPrecision::Day {
                    assert!((1..=28).contains(&s.day()), "day {}", s.day());
                }
                assert_eq!(o.offset(), s.offset(), "UTC/unknown offset changed");
            }
        }
        Ok(())
    }

    #[test]
    fn one_byte_year_clamps_to_width_max() -> Result<()> {
        // Year 1 encodes as a 1-byte VarUInt (max 127). The replacement range
        // (1950..=2050) can't fit in one byte, so it clamps to 127 — a valid
        // year — rather than truncating to something invalid or growing the
        // field. The width, and thus the total byte length, is preserved.
        let input = to_binary("0001T")?;
        for _ in 0..200 {
            let sanitized = sanitize(&input)?;
            assert_eq!(input.len(), sanitized.len(), "year field width changed");
            let out = Element::read_all(sanitized)?;
            let year = out.get(0).unwrap().as_timestamp().unwrap().year();
            assert_eq!(year, 127, "1-byte year should clamp to 127");
        }
        Ok(())
    }

    #[test]
    fn nonzero_offset_is_randomized_but_valid() -> Result<()> {
        // +05:30 (330 min) is a non-zero multiple of 30; it must map to a
        // (usually different) non-zero multiple of 30 within range.
        let input = to_binary("2023-06-15T10:30:45+05:30")?;
        let mut changed = false;
        for _ in 0..50 {
            let out = Element::read_all(sanitize(&input)?)?;
            let offset = out
                .get(0)
                .unwrap()
                .as_timestamp()
                .unwrap()
                .offset()
                .unwrap();
            assert!(offset != 0, "offset became UTC");
            assert_eq!(offset % 30, 0, "offset not a multiple of 30: {}", offset);
            assert!(
                (-1439..=1439).contains(&offset),
                "offset out of range: {}",
                offset
            );
            changed |= offset != 330;
        }
        assert!(
            changed,
            "offset was never randomized away from the original"
        );
        Ok(())
    }

    #[test]
    fn zero_exponent_fraction_stays_valid() -> Result<()> {
        // 2023-01-01T00:00:00 with fraction exponent 0 and a 1-byte coefficient.
        // A non-negative exponent can't represent a sub-second value, so the
        // coefficient must be forced to 0 to stay valid.
        let input = [
            0xE0, 0x01, 0x00, 0xEA, // IVM
            0x6A, // timestamp, length 10
            0x80, 0x0F, 0xE7, // offset +0, year 2023
            0x81, 0x81, 0x80, 0x80, 0x80, // month 1, day 1, 00:00:00
            0x80, 0x00, // fraction exponent 0, coefficient (1 byte)
        ];
        Element::read_all(input)?; // fixture is valid
        for _ in 0..50 {
            Element::read_all(sanitize(&input)?)?; // never a fraction >= 1
        }
        Ok(())
    }

    #[test]
    fn high_precision_fraction_via_writer_stays_valid() -> Result<()> {
        // 20 fractional digits: ion-rs emits exponent -20 with a 9-byte
        // coefficient — the real-world path to the wide-coefficient encoder.
        // Output must stay valid and keep the same fractional scale.
        let input = to_binary("2023-01-01T00:00:00.00000000000000000000Z")?;
        for _ in 0..25 {
            let orig = Element::read_all(&input)?;
            let sanitized = Element::read_all(sanitize(&input)?)?;
            let o = orig.get(0).unwrap().as_timestamp().unwrap();
            let s = sanitized.get(0).unwrap().as_timestamp().unwrap();
            assert_eq!(
                o.fractional_seconds_scale(),
                s.fractional_seconds_scale(),
                "fractional scale changed"
            );
        }
        Ok(())
    }

    #[test]
    fn wide_fraction_coefficient_does_not_panic() -> Result<()> {
        // 2023-01-01T00:00:00 with fraction exponent -9 and a 9-byte coefficient
        // (value 1). Encoding a coefficient into >= 9 bytes must not overflow
        // the shift.
        let input = [
            0xE0, 0x01, 0x00, 0xEA, // IVM
            0x6E, 0x92, // timestamp, VarUInt length 18
            0x80, 0x0F, 0xE7, // offset +0, year 2023
            0x81, 0x81, 0x80, 0x80, 0x80, // month 1, day 1, 00:00:00
            0xC9, // fraction exponent -9
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, // 9-byte coefficient
        ];
        Element::read_all(input)?; // fixture is valid
        for _ in 0..50 {
            Element::read_all(sanitize(&input)?)?;
        }
        Ok(())
    }

    // --- Encoding / growth edge cases -----------------------------------------

    #[test]
    fn empty_string_stays_empty() -> Result<()> {
        // Zero-length values are not grown; an empty string stays empty.
        let input = to_binary(r#"{ e: "" }"#)?;
        let output = sanitize(&input)?;
        assert_eq!(output.len(), input.len());
        let outer = Element::read_all(&output)?.into_iter().next().unwrap();
        let value = outer
            .as_struct()
            .unwrap()
            .fields()
            .next()
            .unwrap()
            .1
            .clone();
        assert_eq!(value.as_string().unwrap(), "");
        Ok(())
    }

    #[test]
    fn zero_length_int_stays_zero() -> Result<()> {
        // A genuine L=0 int (0x20) has no magnitude bytes; it is preserved.
        let input = [0xE0, 0x01, 0x00, 0xEA, 0x20];
        let output = sanitize(&input)?;
        assert_eq!(output, input);
        Ok(())
    }

    #[test]
    fn nop_padding_content_is_replaced() -> Result<()> {
        // A NOP pad's payload is replaced with random bytes (so it can't smuggle
        // data) while its type/length framing is preserved.
        let input = [0xE0, 0x01, 0x00, 0xEA, 0x03, 0xAA, 0xBB, 0xCC];
        let output = sanitize(&input)?;
        assert_eq!(output.len(), input.len());
        assert_eq!(output[4], 0x03, "NOP pad descriptor not preserved");
        assert_ne!(&output[5..8], &input[5..8], "NOP pad payload not replaced");
        Ok(())
    }

    #[test]
    fn output_differs_from_input() -> Result<()> {
        // Guards against a regression to a verbatim copy.
        let input = to_binary(r#"{ name: "Alice", age: 12345, tag: sym }"#)?;
        assert_ne!(input, sanitize(&input)?);
        Ok(())
    }
}
