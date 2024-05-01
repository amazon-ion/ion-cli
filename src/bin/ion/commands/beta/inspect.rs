use std::fmt::Display;
use std::fs::File;
use std::io;
use std::io::Write;
use std::str::FromStr;

use anyhow::{Context, Result};
use clap::{Arg, ArgMatches, Command};
use new_ion_rs::*;

use crate::commands::{IonCliCommand, WithIonCliArgument};

// The `inspect` command uses the `termcolor` crate to colorize its text when STDOUT is a TTY.
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, StandardStreamLock, WriteColor};
// When writing to a named file instead of STDOUT, `inspect` will use a `FileWriter` instead.
// `FileWriter` ignores all requests to emit TTY color escape codes.
use crate::file_writer::FileWriter;


// * The output stream could be STDOUT or a file handle, so we use `dyn io::Write` to abstract
//   over the two implementations.
// * The Drop implementation will ensure that the output stream is flushed when the last reference
//   is dropped, so we don't need to do that manually.
type OutputRef<'a> = Box<dyn WriteColor + 'a>;

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
beginning to display the contents of the stream. If the requested number
of bytes falls in the middle of a scalar, the whole value (complete with
field ID and annotations if applicable) will be displayed. If the value
is nested in one or more containers, the opening delimiters of those
containers be displayed.",
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
processing `n` bytes of Ion data. If `n` falls within a scalar, the
complete value will be displayed. If `n` falls within one or more containers,
the closing delimiters for those containers will be displayed. If this flag
is used with `--skip-bytes`, `n` is counted from the beginning of the first
value start after `--skip-bytes`.
",
                    ),
            )
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        // On macOS and Linux, the `inspect` command's output will automatically be rerouted to a paging
        // utility like `less` when STDOUT is a TTY.
        // TODO find a cross-platform pager implementation.
        #[cfg(not(target_os = "windows"))]
        {
            // If STDOUT is a TTY, direct output to the pager specified by the PAGER environment
            // variable, or "less -FIRX" if the environment variable is not set.
            pager::Pager::with_default_pager("less -FIRX").setup();
        }

        // `--skip-bytes` has a default value, so we can unwrap this safely.
        let skip_bytes_arg = args.get_one::<String>("skip-bytes").unwrap().as_str();

        let bytes_to_skip = usize::from_str(skip_bytes_arg)
            // The `anyhow` crate allows us to augment a given Result with some arbitrary context that
            // will be displayed if it bubbles up to the end user.
            .with_context(|| format!("Invalid value for '--skip-bytes': '{}'", skip_bytes_arg))?;

        // `--limit-bytes` has a default value, so we can unwrap this safely.
        let limit_bytes_arg = args.get_one::<String>("limit-bytes").unwrap().as_str();

        let mut limit_bytes = usize::from_str(limit_bytes_arg)
            .with_context(|| format!("Invalid value for '--limit-bytes': '{}'", limit_bytes_arg))?;

        // If unset, --limit-bytes is effectively usize::MAX. However, it's easier on users if we let
        // them specify "0" on the command line to mean "no limit".
        if limit_bytes == 0 {
            limit_bytes = usize::MAX;
        }

        // These types are provided by the `termcolor` crate. They wrap the normal `io::Stdout` and
        // `io::StdOutLock` types, making it possible to write colorful text to the output stream when
        // it's a TTY that understands formatting escape codes. These variables are declared here so
        // the lifetime will extend through the remainder of the function. Unlike `io::StdoutLock`,
        // the `StandardStreamLock` does not have a static lifetime.
        let stdout: StandardStream;
        let stdout_lock: StandardStreamLock<'_>;

        // If the user has specified an output file, use it.
        let mut output: OutputRef = if let Some(file_name) = args.get_one::<String>("output") {
            let output_file = File::create(file_name)
                .with_context(|| format!("Could not open output file '{file_name}' for writing"))?;
            let file_writer = FileWriter::new(output_file);
            Box::new(file_writer)
        } else {
            // Otherwise, write to STDOUT.
            stdout = StandardStream::stdout(ColorChoice::Always);
            stdout_lock = stdout.lock();
            Box::new(stdout_lock)
        };

        // Run the inspector on each input file that was specified.
        if let Some(input_file_iter) = args.get_many::<String>("input") {
            for input_file_name in input_file_iter {
                let input_file = File::open(input_file_name)
                    .with_context(|| format!("Could not open '{}'", input_file_name))?;
                inspect_input(
                    input_file_name,
                    input_file,
                    &mut output,
                    bytes_to_skip,
                    limit_bytes,
                )?;
            }
        } else {
            let stdin_lock = io::stdin().lock();
            // If no input file was specified, run the inspector on STDIN.
            inspect_input(
                "STDIN",
                stdin_lock,
                &mut output,
                bytes_to_skip,
                limit_bytes,
            )?;
        }
        Ok(())
    }
}

/// Prints a table showing the offset, length, binary encoding, and text encoding of the Ion stream
/// contained in `input`.
fn inspect_input<Input: IonInput>(
    input_name: &str,
    input: Input,
    output: &mut OutputRef,
    bytes_to_skip: usize,
    limit_bytes: usize,
) -> Result<()> {
    let mut reader = SystemReader::new(AnyEncoding, input);
    let mut inspector = IonInspector::new(output, bytes_to_skip, limit_bytes)?;
    // This inspects all values at the top level, recursing as necessary.
    inspector.inspect_top_level(&mut reader)
        .with_context(|| format!("input: {input_name}"))?;
    Ok(())
}

// See the Wikipedia page for Unicode Box Drawing[1] for other potentially useful glyphs.
// [1] https://en.wikipedia.org/wiki/Box-drawing_characters#Unicode
const VERTICAL_LINE: &str = "│";
const START_OF_HEADER: &str = "┌──────────────┬──────────────┬─────────────────────────┬──────────────────────┐";
const END_OF_HEADER: &str = "├──────────────┼──────────────┼─────────────────────────┼──────────────────────┘";
const ROW_SEPARATOR: &str = r#"├──────────────┼──────────────┼─────────────────────────┤
"#;
const END_OF_TABLE: &str = r#"└──────────────┴──────────────┴─────────────────────────┘
"#;

struct IonInspector<'a, 'b> {
    output: &'a mut OutputRef<'b>,
    bytes_to_skip: usize,
    skip_complete: bool,
    limit_bytes: usize,
    // Text Ion writer for formatting scalar values
    text_writer: v1_0::RawTextWriter<Vec<u8>>,
}

// This buffer is used by the IonInspector's `text_writer` to format scalar values.
const TEXT_WRITER_INITIAL_BUFFER_SIZE: usize = 128;

// The number of hex-encoded bytes to show in each row of the `Binary Ion` column.
const BYTES_PER_ROW: usize = 8;

impl<'a, 'b> IonInspector<'a, 'b> {
    fn new(
        out: &'a mut OutputRef<'b>,
        bytes_to_skip: usize,
        limit_bytes: usize,
    ) -> IonResult<IonInspector<'a, 'b>> {
        let text_writer = WriteConfig::<v1_0::Text>::new(TextFormat::Compact)
            .build_raw_writer(Vec::with_capacity(TEXT_WRITER_INITIAL_BUFFER_SIZE))?;
        let inspector = IonInspector {
            output: out,
            bytes_to_skip,
            skip_complete: bytes_to_skip == 0,
            limit_bytes,
            text_writer,
        };
        Ok(inspector)
    }

    /// Iterates over the items in `reader`, printing a table section for each top level value.
    fn inspect_top_level<Input: IonInput>(&mut self, reader: &mut SystemReader<AnyEncoding, Input>) -> Result<()> {
        self.write_table_header()?;
        let mut is_first_item = true;
        let mut has_printed_skip_message = false;
        loop {
            let item = reader.next_item()?;
            // If the next item isn't `EndOfStream`, check to see whether its final byte offset is
            // beyond the configured number of bytes to skip before printing can begin (the value of
            // the `--set-bytes` flag).
            if !matches!(item, SystemStreamItem::EndOfStream(_)) && self.should_skip(item.raw_stream_item()) {
                // If we need to skip it, print a message indicating that some number of items have been skipped.
                if !has_printed_skip_message {
                    self.write_skipping_message(0, "stream items")?;
                    // We only print this message once, so remember that we've already done this.
                    has_printed_skip_message = true;
                }
                // Skip ahead to the next stream item.
                continue;
            }

            // Also check the final byte offset to see if it goes beyond the processing limit set by
            // the `--limit-bytes` flag.
            if self.is_past_limit(item.raw_stream_item()) {
                self.write_limiting_message(0, "ending")?;
                // If the limit is reached at the top level, there's nothing more to do.
                return Ok(());
            }

            // In most cases, we would take this opportunity to print a row separator to create
            // a new table section for this top-level item. However, there are two exceptions we
            // need to check for:
            // 1. The first stream item follows the header and so does not require a row separator.
            if !is_first_item
                // 2. The end of the stream prints the end of the table, not a row separator.
                && !matches!(item, SystemStreamItem::EndOfStream(_)) {
                // If this item is neither the first nor last in the stream, print a row separator.
                write!(self.output, "{ROW_SEPARATOR}")?;
            }

            match item {
                SystemStreamItem::SymbolTable(lazy_struct) => {
                    let lazy_value = lazy_struct.as_value();
                    self.inspect_value(0, "", lazy_value)?;
                }
                SystemStreamItem::Value(lazy_value) => {
                    self.inspect_value(0, "", lazy_value)?;
                }
                SystemStreamItem::VersionMarker(marker) => {
                    self.inspect_ivm(marker)?;
                }
                SystemStreamItem::EndOfStream(_) => {
                    break;
                }
                // `SystemStreamItem` is marked `#[non_exhaustive]`, so this branch is needed.
                // The arms above cover all of the existing variants at the time of writing.
                _ => unimplemented!("a new SystemStreamItem variant was added")
            }

            // Notice that we wait until _after_ the item has been inspected above to set the
            // `skip_complete` flag. This is because the offset specified by `--skip-bytes` may
            // have been located somewhere inside the item and the inspector needed to look for
            // that point within its nested values. If this happens, the inspector will set the
            // `skip_complete` flag when it reaches that offset at a deeper level of nesting.
            // When it reaches this point, `skip_complete` will already be true. However, if the
            // offset fell at the beginning of a top level value, the line below will set the flag
            // for the first time.
            self.skip_complete = true;
            is_first_item = false;
        }
        self.output.write_all(END_OF_TABLE.as_bytes())?;
        Ok(())
    }

    /// If `maybe_item` is:
    ///    * `Some(entity)`, checks to see if the entity's final byte offset is beyond the configured
    ///                      number of bytes to skip.
    ///    * `None`, then there is no stream-level entity backing the item (that is: it was the result
    ///              of a macro expansion). Checks to see if the inspector has already completed its
    ///              skipping phase on an earlier item.
    fn should_skip<T: HasRange>(&mut self, maybe_item: Option<T>) -> bool {
        match maybe_item {
            // If this item came from an input literal, see if the input literal ends after
            // the requested number of bytes to skip. If not, we'll move to the next one.
            Some(item) => item.range().end <= self.bytes_to_skip,
            // If this item came from a macro, there's no corresponding input literal. If we
            // haven't finished skipping input literals, we'll skip this ephemeral value.
            None => !self.skip_complete
        }
    }

    /// If `maybe_item` is:
    ///    * `Some(entity)`, checks to see if the entity's final byte offset is beyond the configured
    ///                      number of bytes to inspect.
    ///    * `None`, then there is no stream-level entity backing the item. These will always be
    ///              inspected; if the e-expression that produced the value was not beyond the limit,
    ///              none of the ephemeral values it produces are either.
    fn is_past_limit<T: HasRange>(&mut self, maybe_item: Option<T>) -> bool {
        maybe_item.map(|item| item.range().start >= self.bytes_to_skip + self.limit_bytes).unwrap_or(false)
    }

    /// Convenience method to set the output stream to the specified color/style for the duration of `write_fn`
    /// and then reset it upon completion.
    fn with_style(&mut self, style: ColorSpec, write_fn: impl FnOnce(&mut OutputRef) -> Result<()>) -> Result<()> {
        self.output.set_color(&style)?;
        write_fn(&mut self.output)?;
        self.output.reset()?;
        Ok(())
    }

    /// Convenience method to set the output stream to the specified color/style, write `text`,
    /// and then reset the output stream's style again.
    fn write_with_style(&mut self, style: ColorSpec, text: &str) -> Result<()> {
        self.with_style(style, |out| {
            out.write_all(text.as_bytes())?;
            Ok(())
        })
    }

    /// Inspects an Ion Version Marker.
    fn inspect_ivm(&mut self, marker: LazyRawAnyVersionMarker<'_>) -> Result<()> {
        const BINARY_IVM_LENGTH: usize = 4;
        let mut formatter = BytesFormatter::new(BYTES_PER_ROW, vec![IonBytes::new(BytesKind::VersionMarker, marker.span().bytes())]);
        self.write_offset_length_and_bytes(marker.range().start, BINARY_IVM_LENGTH, &mut formatter)?;
        self.with_style(BytesKind::VersionMarker.style(), |out| {
            let (major, minor) = marker.version();
            write!(out, "$ion_{major}_{minor}")?;
            Ok(())
        })?;

        self.with_style(comment_style(), |out| {
            write!(out, " // Version marker\n")?;
            Ok(())
        })?;
        self.output.reset()?;
        Ok(())
    }

    /// Inspects all values (however deeply nested) starting at the current level.
    fn inspect_value(&mut self, depth: usize, delimiter: &str, value: LazyValue<'_, AnyEncoding>) -> Result<()> {
        use ValueRef::*;
        if value.has_annotations() {
            self.inspect_annotations(depth, value)?;
        }
        match value.read()? {
            SExp(sexp) => self.inspect_sexp(depth, delimiter, sexp),
            List(list) => self.inspect_list(depth, delimiter, list),
            Struct(struct_) => self.inspect_struct(depth, delimiter, struct_),
            _ => self.inspect_scalar(depth, delimiter, value),
        }
    }

    /// Inspects the scalar `value`. If this value appears in a list or struct, the caller can set
    /// `delimiter` to a comma (`","`) and it will be appended to the value's text representation.
    fn inspect_scalar<'x>(&mut self, depth: usize, delimiter: &str, value: LazyValue<'x, AnyEncoding>) -> Result<()> {
        use ExpandedValueSource::*;
        let value_literal = match value.lower().source() {
            ValueLiteral(value_literal) => value_literal,
            // In Ion 1.0, there are no template values or constructed values so we can defer
            // implementing these.
            Template(_, _) => { todo!("Ion 1.1 template values") }
            Constructed(_, _) => { todo!("Ion 1.1 constructed values") }
        };

        use LazyRawValueKind::*;
        // Check what encoding this is. At the moment, only binary Ion 1.0 is supported.
        match value_literal.kind() {
            Binary_1_0(bin_val) => {
                self.inspect_binary_1_0_scalar(depth, delimiter, value, bin_val)
            }
            Binary_1_1(_) => todo!("Binary Ion 1.1 scalars"),
            Text_1_0(_) | Text_1_1(_) => unreachable!("text value")
        }
    }

    /// Inspects the s-expression `sexp`, including all of its child values. If this sexp appears
    /// in a list or struct, the caller can set `delimiter` to a comma (`","`) and it will be appended
    /// to the sexp's text representation.
    fn inspect_sexp<'x>(&mut self, depth: usize, delimiter: &str, sexp: LazySExp<'x, AnyEncoding>) -> Result<()> {
        use ExpandedSExpSource::*;
        let raw_sexp = match sexp.lower().source() {
            ValueLiteral(raw_sexp) => raw_sexp,
            Template(_, _, _, _) => todo!("Ion 1.1 template SExp")
        };

        use LazyRawSExpKind::*;
        match raw_sexp.kind() {
            Text_1_0(_) | Text_1_1(_) => unreachable!("text value"),
            Binary_1_0(v) => self.inspect_binary_1_0_sexp(depth, delimiter, sexp, v),
            Binary_1_1(_) => todo!("Binary Ion 1.1 SExp"),
        }
    }

    /// Inspects the list `list`, including all of its child values. If this list appears inside
    /// a list or struct, the caller can set `delimiter` to a comma (`","`) and it will be appended
    /// to the list's text representation.
    fn inspect_list<'x>(&mut self, depth: usize, delimiter: &str, list: LazyList<'x, AnyEncoding>) -> Result<()> {
        use ExpandedListSource::*;
        let raw_list = match list.lower().source() {
            ValueLiteral(raw_list) => raw_list,
            Template(_, _, _, _) => todo!("Ion 1.1 template List")
        };

        use LazyRawListKind::*;
        match raw_list.kind() {
            Text_1_0(_) | Text_1_1(_) => unreachable!("text value"),
            Binary_1_0(v) => self.inspect_binary_1_0_list(depth, delimiter, list, v),
            Binary_1_1(_) => todo!("Binary Ion 1.1 List"),
        }
    }

    /// Inspects the struct `struct_`, including all of its fields. If this struct appears inside
    /// a list or struct, the caller can set `delimiter` to a comma (`","`) and it will be appended
    /// to the struct's text representation.
    fn inspect_struct(&mut self, depth: usize, delimiter: &str, struct_: LazyStruct<'_, AnyEncoding>) -> Result<()> {
        let raw_struct = match struct_.lower().source() {
            ExpandedStructSource::ValueLiteral(raw_struct) => raw_struct,
            ExpandedStructSource::Template(_, _, _, _, _) => todo!("Ion 1.1 template Struct")
        };

        use LazyRawValueKind::*;
        match raw_struct.as_value().kind() {
            Binary_1_0(v) => self.inspect_binary_1_0_struct(depth, delimiter, struct_, raw_struct, v),
            Binary_1_1(_) => todo!("Binary Ion 1.1 Struct"),
            Text_1_0(_) | Text_1_1(_) => unreachable!("text value"),
        }
    }

    fn inspect_annotations(&mut self, depth: usize, value: LazyValue<AnyEncoding>) -> Result<()> {
        let raw_value = match value.lower().source() {
            ExpandedValueSource::ValueLiteral(raw_value) => raw_value,
            ExpandedValueSource::Template(_, _) => todo!("Ion 1.1 template value annotations"),
            ExpandedValueSource::Constructed(_, _) => todo!("Ion 1.1 constructed value annotations")
        };

        use LazyRawValueKind::*;
        match raw_value.kind() {
            Binary_1_0(v) => self.inspect_binary_1_0_annotations(depth, value, v),
            Binary_1_1(_) => todo!("Binary Ion 1.1 annotations"),
            Text_1_0(_) | Text_1_1(_) => unreachable!("text value"),
        }
    }

    // ===== Binary Ion 1.0 ======

    fn inspect_binary_1_0_sexp<'x>(&mut self, depth: usize, delimiter: &str, sexp: LazySExp<'x, AnyEncoding>, raw_sexp: v1_0::LazyRawBinarySExp<'x>) -> Result<()> {
        self.inspect_binary_1_0_sequence(depth, "(", "", ")", delimiter, sexp.iter(), raw_sexp, raw_sexp.as_value())
    }

    fn inspect_binary_1_0_list<'x>(&mut self, depth: usize, delimiter: &str, list: LazyList<'x, AnyEncoding>, raw_list: v1_0::LazyRawBinaryList<'x>) -> Result<()> {
        self.inspect_binary_1_0_sequence(depth, "[", ",", "]", delimiter, list.iter(), raw_list, raw_list.as_value())
    }

    fn inspect_binary_1_0_sequence<'x>(&mut self,
                                       depth: usize,
                                       opening_delimiter: &str,
                                       value_delimiter: &str,
                                       closing_delimiter: &str,
                                       trailing_delimiter: &str,
                                       nested_values: impl IntoIterator<Item=IonResult<LazyValue<'x, AnyEncoding>>>,
                                       nested_raw_values: impl LazyRawSequence<'x, v1_0::Binary>,
                                       raw_value: v1_0::LazyRawBinaryValue,
    ) -> Result<()> {
        let encoding = raw_value.encoded_data();
        let range = encoding.range();

        let opcode_bytes: &[u8] = raw_value.encoded_data().opcode_span().bytes();
        let mut formatter = BytesFormatter::new(BYTES_PER_ROW, vec![
            IonBytes::new(BytesKind::Opcode, opcode_bytes),
            IonBytes::new(BytesKind::TrailingLength, raw_value.encoded_data().trailing_length_span().bytes()),
        ]);

        self.write_offset_length_and_bytes(range.start, range.len(), &mut formatter)?;

        self.write_indentation(depth)?;
        self.with_style(text_ion_style(), |out| {
            write!(out, "{opening_delimiter}\n")?;
            Ok(())
        })?;

        let mut has_printed_skip_message = false;
        for (raw_value_res, value_res) in nested_raw_values.iter().zip(nested_values) {
            let (raw_nested_value, nested_value) = (raw_value_res?, value_res?);
            if self.should_skip(Some(raw_nested_value)) {
                if !has_printed_skip_message {
                    self.write_skipping_message(depth + 1, "values")?;
                    has_printed_skip_message = true;
                }
                continue;
            }
            if self.is_past_limit(Some(raw_nested_value)) {
                self.write_limiting_message(depth + 1, "stepping out")?;
                break;
            }
            self.inspect_value(depth + 1, value_delimiter, nested_value)?;
            self.skip_complete = true;
        }

        self.write_blank_offset_length_and_bytes(depth)?;
        self.with_style(text_ion_style(), |out| {
            write!(out, "{closing_delimiter}{trailing_delimiter}\n")?;
            Ok(())
        })
    }

    fn inspect_binary_1_0_struct(&mut self, depth: usize, delimiter: &str, struct_: LazyStruct<AnyEncoding>, raw_struct: LazyRawAnyStruct, raw_value: v1_0::LazyRawBinaryValue) -> Result<()> {
        let encoding = raw_value.encoded_data();
        let range = encoding.range();

        let mut formatter = BytesFormatter::new(BYTES_PER_ROW, vec![
            IonBytes::new(BytesKind::Opcode, encoding.opcode_span().bytes()),
            IonBytes::new(BytesKind::TrailingLength, encoding.trailing_length_span().bytes()),
        ]);

        self.write_offset_length_and_bytes(range.start, range.len(), &mut formatter)?;

        self.write_indentation(depth)?;
        self.with_style(text_ion_style(), |out| {
            write!(out, "{{\n")?;
            Ok(())
        })?;
        let mut has_printed_skip_message = false;
        for (raw_field_result, field_result) in raw_struct.iter().zip(struct_.iter()) {
            let (raw_field, field) = (raw_field_result?, field_result?);
            let (raw_name, raw_value) = raw_field.expect_name_value()?;
            let name = field.name()?;

            if self.should_skip(Some(raw_value)) {
                if !has_printed_skip_message {
                    self.write_skipping_message(depth + 1, "fields")?;
                    has_printed_skip_message = true;
                }
                continue;
            }
            self.skip_complete = true;

            if self.is_past_limit(Some(raw_field)) {
                self.write_limiting_message(depth + 1, "stepping out")?;
                break;
            }

            // ===== Field name =====
            let range = raw_name.range();
            let raw_name_bytes = raw_name.span().bytes();
            let offset = range.start;
            let length = range.len();
            let mut formatter = BytesFormatter::new(BYTES_PER_ROW, vec![
                IonBytes::new(BytesKind::FieldId, raw_name_bytes)
            ]);
            self.write_offset_length_and_bytes(offset, length, &mut formatter)?;

            self.write_indentation(depth + 1)?;
            self.with_style(field_id_style(), |out| {
                IoValueFormatter::new(out).value_formatter().format_symbol(name)?;
                Ok(())
            })?;
            write!(self.output, ": ")?;
            // Print a text Ion comment showing how the field name was encoded, ($SID or text)
            self.with_style(comment_style(), |out| {
                match raw_name.read()? {
                    RawSymbolRef::SymbolId(sid) => {
                        write!(out, " // ${sid}\n")
                    }
                    RawSymbolRef::Text(_) => {
                        write!(out, " // <text>\n")
                    }
                }?;
                Ok(())
            })?;

            // ===== Field value =====
            self.inspect_value(depth + 1, ",", field.value())?;
        }
        // ===== Closing delimiter =====
        self.write_blank_offset_length_and_bytes(depth)?;
        self.with_style(text_ion_style(), |out| {
            write!(out, "}}{delimiter}\n")?;
            Ok(())
        })
    }

    fn inspect_binary_1_0_annotations(&mut self, depth: usize, value: LazyValue<AnyEncoding>, raw_value: v1_0::LazyRawBinaryValue) -> Result<()> {
        let encoding = raw_value.encoded_annotations().unwrap();
        let range = encoding.range();

        let mut formatter = BytesFormatter::new(BYTES_PER_ROW, vec![
            IonBytes::new(BytesKind::AnnotationsHeader, encoding.header_span().bytes()),
            IonBytes::new(BytesKind::AnnotationsSequence, encoding.sequence_span().bytes()),
        ]);
        self.write_offset_length_and_bytes(range.start, range.len(), &mut formatter)?;

        self.write_indentation(depth)?;
        self.with_style(annotations_style(), |out| {
            for annotation in value.annotations() {
                IoValueFormatter::new(&mut *out).value_formatter().format_symbol(annotation?)?;
                write!(out, "::")?;
            }
            Ok(())
        })?;

        self.with_style(comment_style(), |out| {
            write!(out, " // ")?;
            for (index, raw_annotation) in raw_value.annotations().enumerate() {
                if index > 0 {
                    write!(out, ", ")?;
                }
                match raw_annotation? {
                    RawSymbolRef::SymbolId(sid) => write!(out, "${sid}"),
                    RawSymbolRef::Text(_) => write!(out, "<text>"),
                }?;
            }
            write!(out, "\n")?;
            Ok(())
        })?;

        Ok(())
    }

    fn inspect_binary_1_0_scalar(&mut self, depth: usize, delimiter: &str, value: LazyValue<AnyEncoding>, raw_value: v1_0::LazyRawBinaryValue) -> Result<()> {
        let encoding = raw_value.encoded_data();
        let range = encoding.range();

        let opcode_bytes = IonBytes::new(BytesKind::Opcode, encoding.opcode_span().bytes());
        let length_bytes = IonBytes::new(BytesKind::TrailingLength, encoding.trailing_length_span().bytes());
        let body_bytes = IonBytes::new(BytesKind::ValueBody, encoding.body_span().bytes());

        let mut formatter = BytesFormatter::new(BYTES_PER_ROW, vec![opcode_bytes, length_bytes, body_bytes]);

        self.write_offset_length_and_bytes(range.start, range.len(), &mut formatter)?;
        self.write_indentation(depth)?;

        let style = text_ion_style();
        self.output.set_color(&style)?;
        self.text_writer
            .write(value.read()?)
            .expect("failed to write text value to in-memory buffer")
            .flush()?;

        let encoded = self.text_writer.output_mut();
        if encoded.ends_with(&[b' ']) {
            let _ = encoded.pop();
        }
        self.output.write_all(self.text_writer.output().as_slice())?;
        self.text_writer.output_mut().clear();
        self.output.write_all(delimiter.as_bytes())?;
        self.output.reset()?;
        write!(self.output, "\n")?;

        while !formatter.is_empty() {
            self.write_offset_length_and_bytes("", "", &mut formatter)?;
            self.write_indentation(depth)?;
            write!(self.output, "\n")?;
        }

        Ok(())
    }

    // ===== Table-writing methods =====

    /// Prints the header of the output table
    fn write_table_header(&mut self) -> Result<()> {
        self.output.write_all(START_OF_HEADER.as_bytes())?;
        write!(self.output, "\n{VERTICAL_LINE}")?;
        self.write_with_style(header_style(), "    Offset    ")?;
        write!(self.output, "{VERTICAL_LINE}")?;
        self.write_with_style(header_style(), "    Length    ")?;
        write!(self.output, "{VERTICAL_LINE}")?;
        self.write_with_style(header_style(), "       Binary Ion        ")?;
        write!(self.output, "{VERTICAL_LINE}")?;
        self.write_with_style(header_style(), "       Text Ion       ")?;
        write!(self.output, "{VERTICAL_LINE}\n")?;
        self.output.write_all(END_OF_HEADER.as_bytes())?;
        write!(self.output, "\n")?;
        Ok(())
    }

    /// Writes a spacing string `depth` times.
    fn write_indentation(&mut self, depth: usize) -> Result<()> {
        // This spacing string includes a unicode dot to make it easy to see what level of depth
        // the current value is found at. This dot is displayed with a muted color; its appearance
        // is subtle.
        const INDENTATION_WITH_GUIDE: &'static str = "· ";

        let mut color_spec = ColorSpec::new();
        color_spec.set_dimmed(false).set_intense(true).set_bold(true).set_fg(Some(Color::Rgb(100, 100, 100)));
        self.with_style(color_spec, |out| {
            for _ in 0..depth {
                out.write_all(INDENTATION_WITH_GUIDE.as_bytes())?;
            }
            Ok(())
        })
    }

    /// Prints the given `offset` and `length` in the first and second table columns, then uses the
    /// `formatter` to print a single row of hex-encoded bytes in the third column ("Binary Ion").
    /// The `offset` and `length` are typically `usize`, but can be anything that implements `Display`.
    fn write_offset_length_and_bytes(&mut self, offset: impl Display, length: impl Display, formatter: &mut BytesFormatter) -> Result<()> {
        write!(self.output, "{VERTICAL_LINE} {offset:12} {VERTICAL_LINE} {length:12} {VERTICAL_LINE} ")?;
        formatter.write_row(self.output)?;
        write!(self.output, "{VERTICAL_LINE} ")?;
        Ok(())
    }

    /// Prints a row with blank fiends in the `Offset`, `Length`, and `Binary Ion` columns. This method
    /// does not print a trailing newline, allowing the caller to populate the `Text Ion` column as needed.
    fn write_blank_offset_length_and_bytes(&mut self, depth: usize) -> Result<()> {
        let mut formatter = BytesFormatter::new(BYTES_PER_ROW, vec![]);
        self.write_offset_length_and_bytes("", "", &mut formatter)?;
        self.write_indentation(depth)
    }

    /// Prints a row with an ellipsis (`...`) in the first three columns, and a text Ion comment in
    /// the final column indicating what is being skipped over.
    fn write_skipping_message(&mut self, depth: usize, name_of_skipped_item: &str) -> Result<()> {
        write!(self.output, "{VERTICAL_LINE} {:>12} {VERTICAL_LINE} {:>12} {VERTICAL_LINE} {:23} {VERTICAL_LINE} ", "...", "...", "...")?;
        self.write_indentation(depth)?;
        self.with_style(comment_style(), |out| {
            write!(out, "// ...skipping {name_of_skipped_item}...\n")?;
            Ok(())
        })
    }

    /// Prints a row with an ellipsis (`...`) in the first three columns, and a text Ion comment in
    /// the final column indicating that we have reached the maximum number of bytes to process
    /// as determined by the `--limit-bytes` flag.
    fn write_limiting_message(&mut self, depth: usize, action: &str) -> Result<()> {
        write!(self.output, "{VERTICAL_LINE} {:>12} {VERTICAL_LINE} {:>12} {VERTICAL_LINE} {:23} {VERTICAL_LINE} ", "...", "...", "...")?;
        self.write_indentation(depth)?;
        let limit_bytes = self.limit_bytes;
        self.with_style(comment_style(), |out| {
            write!(out, "// --limit-bytes {} reached, {action}.\n", limit_bytes)?;
            Ok(())
        })
    }
}

// ===== Named styles =====

fn header_style() -> ColorSpec {
    let mut style = ColorSpec::new();
    style.set_bold(true).set_intense(true);
    style
}

fn comment_style() -> ColorSpec {
    let mut style = ColorSpec::new();
    style.set_dimmed(true);
    style
}

fn text_ion_style() -> ColorSpec {
    let mut style = ColorSpec::new();
    style.set_fg(Some(Color::Rgb(255, 255, 255)));
    style
}

fn field_id_style() -> ColorSpec {
    let mut style = ColorSpec::new();
    style.set_fg(Some(Color::Cyan)).set_intense(true);
    style
}

fn annotations_style() -> ColorSpec {
    let mut style = ColorSpec::new();
    style.set_fg(Some(Color::Magenta));
    style
}

/// Kinds of encoding primitives found in a binary Ion stream.
#[derive(Copy, Clone, Debug)]
enum BytesKind {
    FieldId,
    Opcode,
    TrailingLength,
    ValueBody,
    AnnotationsHeader,
    AnnotationsSequence,
    VersionMarker,
}

impl BytesKind {
    /// Returns a [`ColorSpec`] that should be used when printing bytes of the specified `BytesKind`.
    fn style(&self) -> ColorSpec {
        use BytesKind::*;
        let mut color = ColorSpec::new();
        match self {
            VersionMarker =>
                color
                    .set_fg(Some(Color::Yellow))
                    .set_intense(true),
            FieldId =>
                color
                    .set_fg(Some(Color::Cyan))
                    .set_intense(true),
            Opcode =>
                color
                    .set_bold(true)
                    .set_fg(Some(Color::Rgb(0, 0, 0)))
                    .set_bg(Some(Color::Rgb(255, 255, 255))),

            TrailingLength =>
                color
                    .set_bold(true)
                    .set_underline(true)
                    .set_fg(Some(Color::White))
                    .set_intense(true),
            ValueBody =>
                color.set_bold(false)
                    .set_fg(Some(Color::White))
                    .set_intense(false),
            AnnotationsHeader =>
                color.set_bold(false)
                    .set_fg(Some(Color::Black))
                    .set_bg(Some(Color::Magenta)),
            AnnotationsSequence =>
                color.set_bold(false)
                    .set_fg(Some(Color::Magenta)),
        };
        color
    }
}

/// A slice of Ion bytes to be printed in the `Binary Ion` column.
///
/// Each `IonBytes` has a `BytesKind` that maps to a display style as well as a counter tracking
/// how many of its bytes have been printed so far.
#[derive(Copy, Clone, Debug)]
struct IonBytes<'a> {
    // The actual slice of bytes
    pub bytes: &'a [u8],
    // What the slice of bytes represents in Ion
    pub kind: BytesKind,
    // How many of this slice's bytes have been printed so far.
    pub bytes_written: usize,
}

impl<'a> IonBytes<'a> {
    fn new(kind: BytesKind, bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            kind,
            bytes_written: 0,
        }
    }

    fn mark_bytes_written(&mut self, num_bytes: usize) {
        self.bytes_written += num_bytes
    }

    fn next_n_bytes(&self, num_bytes: usize) -> &[u8] {
        &self.bytes[self.bytes_written..self.bytes_written + num_bytes]
    }

    fn bytes_remaining(&self) -> usize {
        self.bytes.len() - self.bytes_written
    }

    fn is_empty(&self) -> bool {
        self.bytes_remaining() == 0
    }

    fn style(&self) -> ColorSpec {
        self.kind.style()
    }
}

/// Prints bytes as colorized, hex-encoded rows of a configurable size.
///
/// Stores a sequence of [`IonBytes`] instances to display. Upon request, writes out the next `n`
/// colorized, hex-encoded bytes, remembering where to resume when the next row is needed.
struct BytesFormatter<'a> {
    slices: Vec<IonBytes<'a>>,
    slices_written: usize,
    formatted_bytes_per_row: usize,
}

impl<'a> BytesFormatter<'a> {
    pub fn new(formatted_bytes_per_row: usize, slices: Vec<IonBytes<'a>>) -> Self {
        Self { slices, slices_written: 0, formatted_bytes_per_row }
    }

    /// Writes a row of `n` hex-encoded, colorized bytes, where `n` is determined by the
    /// `formatted_bytes_per_row` argument in [`BytesFormatter::new`].
    ///
    /// If there are fewer than `n` bytes remaining, prints all remaining bytes.
    pub fn write_row(&mut self, output: &mut impl WriteColor) -> Result<()> {
        let num_bytes = self.formatted_bytes_per_row;
        let bytes_written = self.write_bytes(num_bytes, output)?;
        let bytes_remaining = num_bytes - bytes_written;
        // If we printed fewer bytes than are needed to make a row, write out enough padding
        // to keep the columns aligned.
        for _ in 0..bytes_remaining {
            write!(output, "   ")?; // Empty space the width of a formatted byte
        }
        Ok(())
    }

    /// Helper method to iterate over the remaining [`IonBytes`], printing their contents until
    /// `num_bytes` is reached.
    fn write_bytes(&mut self, num_bytes: usize, output: &mut impl WriteColor) -> Result<usize> {
        let mut bytes_remaining = num_bytes;
        while bytes_remaining > 0 && !self.is_empty() {
            bytes_remaining -= self.write_bytes_from_current_slice(bytes_remaining, output)?;
            if self.is_empty() {
                // Even though `bytes_remaining` hasn't reached zero, we're out of data.
                break;
            }
        }

        Ok(num_bytes - bytes_remaining)
    }

    /// Helper method to print up to `num_bytes` bytes from the current [`IonBytes`].
    fn write_bytes_from_current_slice(&mut self, num_bytes: usize, output: &mut impl WriteColor) -> Result<usize> {
        let Some(slice) = self.current_slice() else {
            // No more to write
            return Ok(0);
        };

        if slice.bytes.len() == 0 {
            self.slices_written += 1;
            return Ok(0);
        }

        // We're going to write whichever is smaller:
        //   1. the requested number of bytes from the current slice
        //      OR
        //   2. the number of bytes remaining in the current slice
        let bytes_to_write = num_bytes.min(slice.bytes_remaining());

        // Set the appropriate style for this byte slice.
        let style: ColorSpec = slice.style();
        output.set_color(&style)?;
        write!(output, "{}", hex_contents(slice.next_n_bytes(bytes_to_write)))?;
        slice.mark_bytes_written(bytes_to_write);
        output.reset()?;

        // If we completed the slice OR we finished writing all of the requested bytes
        if slice.is_empty() || num_bytes == bytes_to_write {
            write!(output, " ")?;
        }

        if slice.is_empty() {
            // This slice has been exhausted, we should resume from the beginning of the next one.
            self.slices_written += 1;
        }

        Ok(bytes_to_write)
    }

    /// Returns a reference to the [`IonBytes`] from which the next bytes should be pulled.
    fn current_slice(&mut self) -> Option<&mut IonBytes<'a>> {
        if self.is_empty() {
            return None;
        }
        Some(&mut self.slices[self.slices_written])
    }

    /// Returns `true` if all of the slices have been exhausted.
    fn is_empty(&self) -> bool {
        self.slices_written == self.slices.len()
    }
}

/// Converts the given byte slice to a string containing hex-encoded bytes
fn hex_contents(source: &[u8]) -> String {
    if source.is_empty() {
        return String::new();
    }
    use std::fmt::Write;
    let mut buffer = String::new();
    let bytes = source.iter();

    let mut is_first = true;
    for byte in bytes {
        if is_first {
            write!(buffer, "{:02X?}", byte).unwrap();
            is_first = false;
            continue;
        }
        write!(buffer, " {:02X?}", byte).unwrap();
    }
    buffer
}
