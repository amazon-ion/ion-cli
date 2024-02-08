use std::cmp::min;
use std::fmt::{Display, Write};
use std::fs::File;
use std::io;
use std::io::BufWriter;
use std::ops::Range;
use std::str::{from_utf8_unchecked, FromStr};

use crate::commands::{IonCliCommand, WithIonCliArgument};
use anyhow::{bail, Context, Result};
use clap::{Arg, ArgMatches, Command};
use colored::Colorize;
use ion_rs::*;
use memmap::MmapOptions;
#[cfg(not(target_os = "windows"))]
use pager::Pager;

pub struct InspectCommand;

impl IonCliCommand for InspectCommand {
    fn name(&self) -> &'static str {
        "inspect"
    }

    fn about(&self) -> &'static str {
        "Displays hex-encoded binary Ion alongside its equivalent text for human-friendly debugging."
    }

    fn configure_args(&self, command: Command) -> Command {
        command
            .with_input()
            .with_output()
            .arg(
                // This is named `skip-bytes` instead of `skip` to accommodate a future `skip-values` option.
                Arg::new("skip-bytes")
                    .long("skip-bytes")
                    .short('s')
                    .default_value("0")
                    .hide_default_value(true)
                    .help("Do not display any user values for the first `n` bytes of Ion data.")
                    .long_help(
                        "When specified, the inspector will skip ahead `n` bytes before
beginning to display the contents of the stream. System values like
Ion version markers and symbol tables in the bytes being skipped will
still be displayed. If the requested number of bytes falls in the
middle of a value, the whole value (complete with field ID and
annotations if applicable) will be displayed. If the value is nested
in one or more containers, those containers will be displayed too.",
                    ),
            )
            .arg(
                // This is named `limit-bytes` instead of `limit` to accommodate a future `limit-values` option.
                Arg::new("limit-bytes")
                    .long("limit-bytes")
                    .short('l')
                    .default_value("0")
                    .hide_default_value(true)
                    .help("Only display the next 'n' bytes of Ion data.")
                    .long_help(
                        "When specified, the inspector will stop printing values after
processing `n` bytes of Ion data. If `n` falls within a value, the
complete value will be displayed.",
                    ),
            )
    }

    #[cfg(not(target_os = "windows"))] // TODO find a cross-platform pager implementation.
    fn set_up_pager(&self) {
        // Direct output to the pager specified by the PAGER environment variable, or "less -FIRX"
        // if the environment variable is not set. Note: a pager is not used if the output is not
        // a TTY.
        Pager::with_default_pager("less -FIRX").setup();
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        self.set_up_pager();

        // --skip-bytes has a default value, so we can unwrap this safely.
        let skip_bytes_arg = args.get_one::<String>("skip-bytes").unwrap().as_str();

        let bytes_to_skip = usize::from_str(skip_bytes_arg)
            // The `anyhow` crate allows us to augment a given Result with some arbitrary context that
            // will be displayed if it bubbles up to the end user.
            .with_context(|| format!("Invalid value for '--skip-bytes': '{}'", skip_bytes_arg))?;

        // --limit-bytes has a default value, so we can unwrap this safely.
        let limit_bytes_arg = args.get_one::<String>("limit-bytes").unwrap().as_str();

        let mut limit_bytes = usize::from_str(limit_bytes_arg)
            .with_context(|| format!("Invalid value for '--limit-bytes': '{}'", limit_bytes_arg))?;

        // If unset, --limit-bytes is effectively usize::MAX. However, it's easier on users if we let
        // them specify "0" on the command line to mean "no limit".
        if limit_bytes == 0 {
            limit_bytes = usize::MAX
        }

        // If the user has specified an output file, use it.
        let mut output: OutputRef = if let Some(file_name) = args.get_one::<String>("output") {
            let output_file = File::create(file_name)
                .with_context(|| format!("Could not open '{}'", file_name))?;
            let buf_writer = BufWriter::new(output_file);
            Box::new(buf_writer)
        } else {
            // Otherwise, write to STDOUT.
            Box::new(io::stdout().lock())
        };

        // Run the inspector on each input file that was specified.
        if let Some(input_file_iter) = args.get_many::<String>("input") {
            for input_file_name in input_file_iter {
                let input_file = File::open(input_file_name)
                    .with_context(|| format!("Could not open '{}'", input_file_name))?;
                inspect_file(
                    input_file_name,
                    input_file,
                    &mut output,
                    bytes_to_skip,
                    limit_bytes,
                )?;
            }
        } else {
            // If no input file was specified, run the inspector on STDIN.

            // The inspector expects its input to be a byte array or mmap()ed file acting as a byte
            // array. If the user wishes to provide data on STDIN, we'll need to copy those bytes to
            // a temporary file and then read from that.

            // Create a temporary file that will delete itself when the program ends.
            let mut input_file = tempfile::tempfile().with_context(|| {
                concat!(
                    "Failed to create a temporary file to store STDIN.",
                    "Try passing an --input flag instead."
                )
            })?;

            // Pipe the data from STDIN to the temporary file.
            let mut writer = BufWriter::new(input_file);
            io::copy(&mut io::stdin(), &mut writer)
                .with_context(|| "Failed to copy STDIN to a temp file.")?;
            // Get our file handle back from the BufWriter
            input_file = writer
                .into_inner()
                .with_context(|| "Failed to read from temp file containing STDIN data.")?;
            // Read from the now-populated temporary file.
            inspect_file(
                "STDIN temp file",
                input_file,
                &mut output,
                bytes_to_skip,
                limit_bytes,
            )?;
        }
        Ok(())
    }
}

// Create a type alias to simplify working with a shared reference to our output stream.
type OutputRef = Box<dyn io::Write>;
// * The output stream could be STDOUT or a file handle, so we use `dyn io::Write` to abstract
//   over the two implementations.
// * The Drop implementation will ensure that the output stream is flushed when the last reference
//   is dropped, so we don't need to do that manually.

// Given a file, try to mmap() it and run the inspector over the resulting byte array.
fn inspect_file(
    input_file_name: &str,
    input_file: File,
    output: &mut OutputRef,
    bytes_to_skip: usize,
    limit_bytes: usize,
) -> Result<()> {
    // mmap involves operating system interactions that inherently place its usage outside of Rust's
    // safety guarantees. If the file is unexpectedly truncated while it's being read, for example,
    // problems could arise.
    let mmap = unsafe {
        MmapOptions::new()
            .map(&input_file)
            .with_context(|| format!("Could not mmap '{}'", input_file_name))?
    };

    // Treat the mmap as a byte array.
    let ion_data: &[u8] = &mmap[..];
    // Confirm that the input data is binary Ion, then run the inspector.
    match ion_data {
        // Pattern match the byte array to verify it starts with an IVM
        [0xE0, 0x01, 0x00, 0xEA, ..] => {
            write_header(output)?;
            let mut inspector = IonInspector::new(ion_data, output, bytes_to_skip, limit_bytes)?;
            // This inspects all values at the top level, recursing as necessary.
            inspector.inspect_level()?;
        }
        _ => {
            // bail! constructs an `anyhow::Result` with the given context and returns.
            bail!(
                "Input file '{}' does not appear to be binary Ion.",
                input_file_name
            );
        }
    };
    Ok(())
}

const IVM_HEX: &str = "e0 01 00 ea";
const IVM_TEXT: &str = "// Ion 1.0 Version Marker";
// System events (IVM, symtabs) are always at the top level.
const SYSTEM_EVENT_INDENTATION: &str = "";
const LEVEL_INDENTATION: &str = "  "; // 2 spaces per level
const TEXT_WRITER_INITIAL_BUFFER_SIZE: usize = 128;

struct IonInspector<'a> {
    output: &'a mut OutputRef,
    reader: SystemReader<RawBinaryReader<&'a [u8]>>,
    bytes_to_skip: usize,
    limit_bytes: usize,
    // Reusable buffer for formatting bytes as hex
    hex_buffer: String,
    // Reusable buffer for formatting text
    text_buffer: String,
    // Reusable buffer for colorizing text
    color_buffer: String,
    // Reusable buffer for tracking indentation
    indentation_buffer: String,
    // Text Ion writer for formatting scalar values
    text_ion_writer: RawTextWriter<Vec<u8>>,
}

impl<'a> IonInspector<'a> {
    fn new<'b>(
        input: &'b [u8],
        out: &'b mut OutputRef,
        bytes_to_skip: usize,
        limit_bytes: usize,
    ) -> IonResult<IonInspector<'b>> {
        let reader = SystemReader::new(RawBinaryReader::new(input));
        let text_ion_writer = RawTextWriterBuilder::new(TextKind::Compact)
            .build(Vec::with_capacity(TEXT_WRITER_INITIAL_BUFFER_SIZE))?;
        let inspector = IonInspector {
            output: out,
            reader,
            bytes_to_skip,
            limit_bytes,
            hex_buffer: String::new(),
            text_buffer: String::new(),
            color_buffer: String::new(),
            indentation_buffer: String::new(),
            text_ion_writer,
        };
        Ok(inspector)
    }

    // Returns the offset of the first byte that pertains to the value on which the reader is
    // currently parked.
    fn first_value_byte_offset(&self) -> usize {
        if let Some(offset) = self.reader.field_id_offset() {
            return offset;
        }
        if let Some(offset) = self.reader.annotations_offset() {
            return offset;
        }
        self.reader.header_offset()
    }

    // Returns the byte offset range containing the current value and its annotations/field ID if
    // applicable.
    fn complete_value_range(&self) -> Range<usize> {
        let start = self.first_value_byte_offset();
        let end = self.reader.value_range().end;
        start..end
    }

    // Displays all of the values (however deeply nested) at the current level.
    fn inspect_level(&mut self) -> Result<()> {
        self.increase_indentation();

        // Per-level bytes skipped are tracked so we can add them to the text Ion comments that
        // appear each time some number of values is skipped.
        let mut bytes_skipped_this_level = 0;

        loop {
            let ion_type = match self.reader.next()? {
                SystemStreamItem::Nothing => break,
                SystemStreamItem::VersionMarker(major, minor) => {
                    if major != 1 || minor != 0 {
                        bail!(
                            "Only Ion 1.0 is supported. Found IVM for v{}.{}",
                            major,
                            minor
                        );
                    }
                    output(
                        self.output,
                        None,
                        Some(4),
                        SYSTEM_EVENT_INDENTATION,
                        IVM_HEX,
                        IVM_TEXT.dimmed(),
                    )
                    .expect("output() failure from on_ivm()");
                    continue;
                }
                // We don't care if this is a system or user-level value; that distinction
                // is handled inside the SystemReader.
                SystemStreamItem::SymbolTableValue(ion_type)
                | SystemStreamItem::Value(ion_type)
                | SystemStreamItem::SymbolTableNull(ion_type)
                | SystemStreamItem::Null(ion_type) => ion_type,
            };
            // See if we've already processed `bytes_to_skip` bytes; if not, move to the next value.
            let complete_value_range = self.complete_value_range();
            if complete_value_range.end <= self.bytes_to_skip {
                bytes_skipped_this_level += complete_value_range.len();
                continue;
            }

            // Saturating subtraction: if the result would underflow, the answer will be zero.
            let bytes_processed = complete_value_range
                .start
                .saturating_sub(self.bytes_to_skip);
            // See if we've already processed `limit_bytes`; if so, stop processing.
            if bytes_processed >= self.limit_bytes {
                let limit_message = if self.reader.depth() > 0 {
                    "// --limit-bytes reached, stepping out."
                } else {
                    "// --limit-bytes reached, ending."
                };
                output(
                    self.output,
                    None,
                    None,
                    &self.indentation_buffer,
                    "...",
                    limit_message.dimmed(),
                )?;
                self.decrease_indentation();
                return Ok(());
            }

            // We're no longer skip-scanning to `bytes_to_skip`. If we skipped values at this depth
            // to get to this point, make a note of it in the output.
            if bytes_skipped_this_level > 0 {
                self.text_buffer.clear();
                write!(
                    &mut self.text_buffer,
                    "// Skipped {} bytes of user-level data",
                    bytes_skipped_this_level
                )?;
                output(
                    self.output,
                    None,
                    None,
                    &self.indentation_buffer,
                    "...",
                    &self.text_buffer.dimmed(),
                )?;
                bytes_skipped_this_level = 0;
            }

            self.write_field_if_present()?;
            self.write_annotations_if_present()?;
            // Print the value or, if it's a container, its opening delimiter: {, (, or [
            self.write_value()?;

            // If the current value is a container, step into it and inspect its contents.
            match ion_type {
                IonType::List | IonType::SExp | IonType::Struct => {
                    self.reader.step_in()?;
                    self.inspect_level()?;
                    self.reader.step_out()?;
                    // Print the container's closing delimiter: }, ), or ]
                    self.text_buffer.clear();
                    self.text_buffer.push_str(closing_delimiter_for(ion_type));
                    if ion_type != IonType::SExp && self.reader.depth() > 0 {
                        self.text_buffer.push(',');
                    }
                    output(
                        self.output,
                        None,
                        None,
                        &self.indentation_buffer,
                        "",
                        &self.text_buffer,
                    )?;
                }
                _ => {}
            }
        }

        self.decrease_indentation();
        Ok(())
    }

    fn increase_indentation(&mut self) {
        // Add a level's worth of indentation to the buffer.
        if self.reader.depth() > 0 {
            self.indentation_buffer.push_str(LEVEL_INDENTATION);
        }
    }

    fn decrease_indentation(&mut self) {
        // Remove a level's worth of indentation from the buffer.
        if self.reader.depth() > 0 {
            let new_length = self.indentation_buffer.len() - LEVEL_INDENTATION.len();
            self.indentation_buffer.truncate(new_length);
        }
    }

    fn write_field_if_present(&mut self) -> Result<()> {
        if self.reader.parent_type() != Some(IonType::Struct) {
            // We're not in a struct; nothing to do.
            return Ok(());
        }
        let field_token = self.reader.raw_field_name_token()?;
        let field_id = field_token.local_sid().expect("No SID for field name.");
        self.hex_buffer.clear();
        to_hex(
            &mut self.hex_buffer,
            self.reader.raw_field_id_bytes().unwrap(),
        );

        let field_name_result = self.reader.field_name();
        let field_name = field_name_result
            .as_ref()
            .ok()
            .and_then(|name| name.text())
            .unwrap_or("<UNKNOWN>");

        self.text_buffer.clear();
        write!(&mut self.text_buffer, "'{}':", field_name)?;

        self.color_buffer.clear();
        write!(&mut self.color_buffer, " // ${}:", field_id)?;
        write!(&mut self.text_buffer, "{}", &self.color_buffer.dimmed())?;
        output(
            self.output,
            self.reader.field_id_offset(),
            self.reader.field_id_length(),
            &self.indentation_buffer,
            &self.hex_buffer,
            &self.text_buffer,
        )?;

        if field_name_result.is_err() {
            // If we had to write <UNKNOWN> for the field name above, return a fatal error now.
            bail!("Encountered a field ID (${}) with unknown text.", field_id);
        }

        Ok(())
    }

    fn write_annotations_if_present(&mut self) -> IonResult<()> {
        let num_annotations = self.reader.raw_annotations().count();
        if num_annotations > 0 {
            self.hex_buffer.clear();
            to_hex(
                &mut self.hex_buffer,
                self.reader.raw_annotations_bytes().unwrap(),
            );

            self.text_buffer.clear();
            join_into(&mut self.text_buffer, "::", self.reader.annotations())?;
            write!(&mut self.text_buffer, "::")?;

            self.color_buffer.clear();
            write!(&mut self.color_buffer, " // $")?;
            join_into(
                &mut self.color_buffer,
                "::$",
                self.reader
                    .raw_annotations()
                    .map(|a| a.map(|token| token.local_sid().unwrap())),
            )?;
            write!(&mut self.color_buffer, "::")?;

            write!(self.text_buffer, "{}", self.color_buffer.dimmed())?;
            output(
                self.output,
                self.reader.annotations_offset(),
                self.reader.annotations_length(),
                &self.indentation_buffer,
                &self.hex_buffer,
                &self.text_buffer,
            )?;
        }
        Ok(())
    }

    fn write_value(&mut self) -> IonResult<()> {
        self.text_buffer.clear();
        // Populates `self.text_buffer` with the Ion text representation of the current value
        // if it is a scalar. If the value is a container, format_value() will write the opening
        // delimiter of that container instead.
        self.format_value()?;

        self.hex_buffer.clear();
        to_hex(
            &mut self.hex_buffer,
            self.reader.raw_header_bytes().unwrap(),
        );
        // Only write the bytes representing the body of the value if it is a scalar.
        // If it is a container, `inspect_level` will handle stepping into it and writing any
        // nested values.
        if !self.reader.ion_type().unwrap().is_container() {
            self.hex_buffer.push(' ');
            to_hex(&mut self.hex_buffer, self.reader.raw_value_bytes().unwrap());
        }

        let length = self.reader.header_length() + self.reader.value_length();
        output(
            self.output,
            Some(self.reader.header_offset()),
            Some(length),
            &self.indentation_buffer,
            &self.hex_buffer,
            &self.text_buffer,
        )
    }

    fn format_value(&mut self) -> IonResult<()> {
        use ion_rs::IonType::*;

        // Destructure `self` to get multiple simultaneous mutable references to its constituent
        // fields. This freezes `self`; it cannot be referred to for the rest of the function call.
        let IonInspector {
            ref mut reader,
            ref mut text_ion_writer,
            ref mut text_buffer,
            ref mut color_buffer,
            ..
        } = self;

        // If we need to write comments alongside any of the values, we'll add them here so we can
        // colorize them separately.
        let comment_buffer = color_buffer;
        comment_buffer.clear();

        let writer = text_ion_writer; // Local alias for brevity.
        let ion_type = reader
            .ion_type()
            .expect("format_value() called when reader was exhausted");
        if reader.is_null() {
            writer.write_null(reader.ion_type().unwrap())?;
        } else {
            match ion_type {
                Null => writer.write_null(ion_type),
                Bool => writer.write_bool(reader.read_bool()?),
                Int => writer.write_i64(reader.read_i64()?),
                Float => writer.write_f64(reader.read_f64()?),
                Decimal => writer.write_decimal(&reader.read_decimal()?),
                Timestamp => writer.write_timestamp(&reader.read_timestamp()?),
                Symbol => {
                    // TODO: Make this easier in the reader
                    let symbol_token = reader.read_raw_symbol()?;
                    let sid = symbol_token.local_sid().unwrap();
                    let text = reader
                        .symbol_table()
                        .text_for(sid)
                        .unwrap_or_else(|| panic!("Could not resolve text for symbol ID ${}", sid));
                    write!(comment_buffer, " // ${}", sid)?;
                    writer.write_symbol(text)
                }
                String => writer.write_string(reader.read_str()?),
                Clob => writer.write_clob(reader.read_clob()?),
                Blob => writer.write_blob(reader.read_blob()?),
                // The containers don't use the RawTextWriter to format anything. They simply write
                // the appropriate opening delimiter.
                List => {
                    write!(text_buffer, "[")?;
                    return Ok(());
                }
                SExp => {
                    write!(text_buffer, "(")?;
                    return Ok(());
                }
                Struct => {
                    write!(text_buffer, "{{")?;
                    return Ok(());
                }
            }?;
        }
        // This is writing to a Vec, so flush() will always succeed.
        let _ = writer.flush();
        // The writer produces valid UTF-8, so there's no need to re-validate it.
        let value_text = unsafe { from_utf8_unchecked(writer.output().as_slice()) };
        write!(text_buffer, "{}", value_text.trim_end())?;
        // If we're in a container, add a delimiting comma. Text Ion allows trailing commas, so we
        // don't need to treat the last value as a special case.
        if self.reader.depth() > 0 {
            write!(text_buffer, ",")?;
        }
        write!(text_buffer, "{}", comment_buffer.dimmed())?;
        // Clear the writer's output Vec. We encode each scalar independently of one another.
        writer.output_mut().clear();
        Ok(())
    }
}

const COLUMN_DELIMITER: &str = " | ";
const CHARS_PER_HEX_BYTE: usize = 3;
const HEX_BYTES_PER_ROW: usize = 8;
const HEX_COLUMN_SIZE: usize = HEX_BYTES_PER_ROW * CHARS_PER_HEX_BYTE;

fn write_header(output: &mut OutputRef) -> IonResult<()> {
    let line = "-".repeat(24 + 24 + 9 + 9 + (COLUMN_DELIMITER.len() * 3));

    writeln!(output, "{}", line)?;
    write!(
        output,
        "{:^9}{}",
        "Offset".bold().bright_white(),
        COLUMN_DELIMITER
    )?;
    write!(
        output,
        "{:^9}{}",
        "Length".bold().bright_white(),
        COLUMN_DELIMITER
    )?;
    write!(
        output,
        "{:^24}{}",
        "Binary Ion".bold().bright_white(),
        COLUMN_DELIMITER
    )?;
    writeln!(output, "{:^24}", "Text Ion".bold().bright_white())?;
    writeln!(output, "{}", line)?;
    Ok(())
}

// Accepting a `T` allows us to pass in `&str`, `&String`, `&ColoredString`, etc as out text_column
// TODO: This could be a method on IonInspector
fn output<T: Display>(
    output: &mut OutputRef,
    offset: Option<usize>,
    length: Option<usize>,
    indentation: &str,
    hex_column: &str,
    text_column: T,
) -> IonResult<()> {
    // The current implementation always writes a single line of output for the offset, length,
    // and text columns. Only the hex column can span multiple rows.
    // TODO: It would be nice to allow important hex bytes (e.g. type descriptors or lengths)
    //       to be color-coded. This complicates the output function, however, as the length
    //       of a colored string is not the same as its display length. We would need to pass
    //       uncolored strings to the output function paired with the desired color/style so
    //       the output function could break the text into the necessary row lengths and then apply
    //       the provided colors just before writing.

    // Write the offset column
    if let Some(offset) = offset {
        write!(output, "{:9}{}", offset, COLUMN_DELIMITER)?;
    } else {
        write!(output, "{:9}{}", "", COLUMN_DELIMITER)?;
    }

    // Write the length column
    if let Some(length) = length {
        write!(output, "{:9}{}", length, COLUMN_DELIMITER)?;
    } else {
        write!(output, "{:9}{}", "", COLUMN_DELIMITER)?;
    }

    // If the hex string is short enough to fit in a single row...
    if hex_column.len() < HEX_COLUMN_SIZE {
        // ...print the hex string...
        write!(output, "{}", hex_column)?;
        // ...and then write enough padding spaces to fill the rest of the row.
        for _ in 0..(HEX_COLUMN_SIZE - hex_column.len()) {
            write!(output, " ")?;
        }
    } else {
        // Otherwise, write the first row's worth of the hex string.
        write!(output, "{}", &hex_column[..HEX_COLUMN_SIZE])?;
    }
    // Write a delimiter, the write the text Ion as the final column.
    write!(output, "{}", COLUMN_DELIMITER)?;
    write!(output, " ")?;
    writeln!(output, "{}{}", indentation, text_column)?;

    // Revisit our hex column. Write as many additional rows as needed.
    let mut col_1_written = HEX_COLUMN_SIZE;
    while col_1_written < hex_column.len() {
        // Padding for offset column
        write!(output, "{:9}{}", "", COLUMN_DELIMITER)?;
        // Padding for length column
        write!(output, "{:9}{}", "", COLUMN_DELIMITER)?;
        let remaining_bytes = hex_column.len() - col_1_written;
        let bytes_to_write = min(remaining_bytes, HEX_COLUMN_SIZE);
        let next_slice_to_write = &hex_column[col_1_written..(col_1_written + bytes_to_write)];
        write!(output, "{}", next_slice_to_write)?;
        for _ in 0..(HEX_COLUMN_SIZE - bytes_to_write) {
            write!(output, " ")?;
        }
        writeln!(output, "{}", COLUMN_DELIMITER)?;
        col_1_written += HEX_COLUMN_SIZE;
        // No need to write anything for the text column since it's the last one.
    }
    Ok(())
}

fn closing_delimiter_for(container_type: IonType) -> &'static str {
    match container_type {
        IonType::List => "]",
        IonType::SExp => ")",
        IonType::Struct => "}",
        _ => panic!("Attempted to close non-container type {:?}", container_type),
    }
}

fn to_hex(buffer: &mut String, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    write!(buffer, "{:02x}", bytes[0]).unwrap();
    for byte in &bytes[1..] {
        write!(buffer, " {:02x}", *byte).unwrap();
    }
}

fn join_into<T: Display>(
    buffer: &mut String,
    delimiter: &str,
    mut values: impl Iterator<Item = IonResult<T>>,
) -> IonResult<()> {
    if let Some(first) = values.next() {
        write!(buffer, "{}", first?).unwrap();
    }
    for value in values {
        write!(buffer, "{}{}", delimiter, value?).unwrap();
    }
    Ok(())
}
