use std::fmt::Display;
use std::fs::File;
use std::io;
use std::io::{BufWriter, Write};
use std::str::FromStr;

use anyhow::{bail, Context, Result};
use clap::{Arg, ArgMatches, Command};
use memmap::MmapOptions;
use new_ion_rs::*;
use new_ion_rs::lazy::any_encoding::{AnyEncoding, LazyRawAnyStruct, LazyRawValueKind};
use new_ion_rs::lazy::binary::raw::value::LazyRawBinaryValue_1_0;
use new_ion_rs::lazy::decoder::{HasRange, HasSpan, LazyRawContainer, LazyRawFieldName, LazyRawStruct};
use new_ion_rs::lazy::encoder::LazyRawWriter;
use new_ion_rs::lazy::encoder::text::v1_0::writer::LazyRawTextWriter_1_0;
use new_ion_rs::lazy::encoding::TextEncoding_1_0;
use new_ion_rs::lazy::expanded::ExpandedValueSource;
use new_ion_rs::lazy::expanded::r#struct::ExpandedStructSource;
use new_ion_rs::lazy::expanded::sequence::{ExpandedListSource, ExpandedSExpSource};
use new_ion_rs::lazy::r#struct::LazyStruct;
use new_ion_rs::lazy::sequence::{LazyList, LazySExp};
use new_ion_rs::lazy::system_reader::LazySystemAnyReader;
use new_ion_rs::lazy::system_stream_item::SystemStreamItem;
use new_ion_rs::lazy::value::LazyValue;
use new_ion_rs::lazy::value_ref::ValueRef;
#[cfg(not(target_os = "windows"))]
use pager::Pager;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

use crate::commands::{IonCliCommand, WithIonCliArgument};

pub struct DissectCommand;

impl IonCliCommand for DissectCommand {
    fn name(&self) -> &'static str {
        "dissect"
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

    fn set_up_pager(&self) {
        // Direct output to the pager specified by the PAGER environment variable, or "less -FIRX"
        // if the environment variable is not set. Note: a pager is not used if the output is not
        // a TTY.
        Pager::with_default_pager("less -FIRX").setup();
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        #[cfg(not(target_os = "windows"))] // TODO find a cross-platform pager implementation.
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
            let file_writer = FileWriter::new(output_file);
            Box::new(file_writer)
        } else {
            // Otherwise, write to STDOUT.
            Box::new(StandardStream::stdout(ColorChoice::Always))
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

// Our own type that we can implement foreign traits on
struct FileWriter {
    inner: BufWriter<File>,
}

impl FileWriter {
    pub fn new(file: File) -> Self {
        Self { inner: BufWriter::new(file) }
    }
}

impl Write for FileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl WriteColor for FileWriter {
    fn supports_color(&self) -> bool {
        false
    }

    fn set_color(&mut self, _spec: &ColorSpec) -> io::Result<()> {
        Ok(())
    }

    fn reset(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// Create a type alias to simplify working with a shared reference to our output stream.
type OutputRef<'a> = Box<dyn WriteColor + 'a>;

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

    colored::control::set_override(true);
    let mut reader = LazySystemAnyReader::new(ion_data);

    match ion_data {
        // Pattern match the byte array to verify it starts with a binary IVM for 1.0 or 1.1
        [0xE0, 0x01, 0x00 | 0x01, 0xEA, ..] => {
            let mut inspector = IonInspector::new(output, bytes_to_skip, limit_bytes)?;
            // This inspects all values at the top level, recursing as necessary.
            inspector.inspect_top_level(&mut reader)?;
        }
        _ => {
            bail!(
                "Input file '{}' does not appear to be binary Ion.",
                input_file_name
            );
        }
    };
    Ok(())
}

const TEXT_WRITER_INITIAL_BUFFER_SIZE: usize = 128;

const VERTICAL_LINE: &str = "│";
const HEADER: &str = r#"┌──────────────┬──────────────┬─────────────────────────┬──────────────────────┐
│    Offset    │    Length    │       Binary Ion        │       Text Ion       │
├──────────────┼──────────────┼─────────────────────────┼──────────────────────┘
"#;

const ROW_SEPARATOR: &str = r#"├──────────────┼──────────────┼─────────────────────────┤
"#;
const FINAL_ROW_SEPARATOR: &str = r#"└──────────────┴──────────────┴─────────────────────────┘
"#;

struct IonInspector<'a, 'b> {
    output: &'a mut OutputRef<'b>,
    bytes_to_skip: usize,
    skip_complete: bool,
    limit_bytes: usize,
    // Text Ion writer for formatting scalar values
    text_writer: LazyRawTextWriter_1_0<Vec<u8>>,
}

const BYTES_PER_ROW: usize = 8;

impl<'a, 'b> IonInspector<'a, 'b> {
    fn new(
        out: &'a mut OutputRef<'b>,
        bytes_to_skip: usize,
        limit_bytes: usize,
    ) -> IonResult<IonInspector<'a, 'b>> {
        let text_writer = WriteConfig::<TextEncoding_1_0>::new(TextKind::Compact)
            .build(Vec::with_capacity(TEXT_WRITER_INITIAL_BUFFER_SIZE))?;
        let inspector = IonInspector {
            output: out,
            bytes_to_skip,
            skip_complete: bytes_to_skip == 0,
            limit_bytes,
            text_writer,
        };
        Ok(inspector)
    }

    fn write_offset_length_and_bytes(&mut self, offset: impl Display, length: impl Display, formatter: &mut BytesFormatter) -> Result<()> {
        write!(self.output, "{VERTICAL_LINE} {offset:12} {VERTICAL_LINE} {length:12} {VERTICAL_LINE} ")?;
        formatter.write_row(self.output)?;
        write!(self.output, "{VERTICAL_LINE} ")?;
        Ok(())
    }

    fn write_blank_offset_length_and_bytes(&mut self) -> Result<()> {
        let mut formatter = BytesFormatter::new(BYTES_PER_ROW, vec![]);
        self.write_offset_length_and_bytes("", "", &mut formatter)
    }

    fn inspect_top_level(&mut self, reader: &mut LazySystemAnyReader<&'a [u8]>) -> Result<()> {
        self.output.write_all(HEADER.as_bytes())?;
        let mut is_first_item = true;
        loop {
            let item = reader.next_item()?;
            if !is_first_item && !matches!(item, SystemStreamItem::EndOfStream) {
                write!(self.output, "{ROW_SEPARATOR}")?;
            }
            match item {
                SystemStreamItem::SymbolTable(lazy_struct) => {
                    let lazy_value = lazy_struct.as_value();
                    if self.should_skip(lazy_value) {
                        // As long as we're skipping top level values, we need to defer writing
                        // the first row separator.
                        is_first_item = true;
                        continue;
                    } else {
                        self.inspect_value(0, "", lazy_value)?;
                    }
                }
                SystemStreamItem::Value(lazy_value) => {
                    if self.should_skip(lazy_value) {
                        // As long as we're skipping top level values, we need to defer writing
                        // the first row separator.
                        is_first_item = true;
                        continue;
                    } else {
                        self.inspect_value(0, "", lazy_value)?;
                    }
                }
                SystemStreamItem::VersionMarker(major, minor) => {
                    self.inspect_ivm(major, minor)?;
                }
                SystemStreamItem::EndOfStream => {
                    self.output.write_all(FINAL_ROW_SEPARATOR.as_bytes())?;
                    break;
                }
                _ => unimplemented!("a new SystemStreamItem variant was added")
            }
            is_first_item = false;
        }
        Ok(())
    }

    fn should_skip(&mut self, value: LazyValue<'_, AnyEncoding>) -> bool {
        match value.lower().source() {
            ExpandedValueSource::ValueLiteral(raw_value) => raw_value.range().end < self.bytes_to_skip,
            _ => todo!(),
        }
    }

    fn inspect_ivm(&mut self, major: u8, minor: u8) -> Result<()> {
        // TODO: Plumb IVM span/range through to the reader. In the meantime, we leave 'offset'
        //       blank and make our own bytes slice
        let offset = "";
        let ivm_bytes = &[0xEA, major, minor, 0xE0];

        let mut formatter = BytesFormatter::new(BYTES_PER_ROW, vec![IonBytes::new(BytesKind::VersionMarker, ivm_bytes)]);
        self.write_offset_length_and_bytes(offset, 4, &mut formatter)?;
        self.with_style(BytesKind::VersionMarker.style(), |out| {
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

    fn write_indentation(&mut self, depth: usize) -> Result<()> {
        const INDENTATION_WITH_GUIDE: &'static str = "· ";
        if depth == 0 {
            return Ok(());
        }

        let mut color_spec = ColorSpec::new();
        color_spec.set_dimmed(false).set_intense(true).set_bold(true).set_fg(Some(Color::Rgb(100, 100, 100)));
        self.with_style(color_spec, |out| {
            for _ in 0..depth {
                out.write_all(INDENTATION_WITH_GUIDE.as_bytes())?;
            }
            Ok(())
        })
    }

    // Displays all of the values (however deeply nested) at the current level.
    fn inspect_value(&mut self, depth: usize, delimiter: &str, value: LazyValue<'_, AnyEncoding>) -> Result<()> {
        use ValueRef::*;
        let value_ref = value.read()?;
        if value.is_container() {
            match value_ref {
                SExp(sexp) => self.inspect_sexp(depth, delimiter, sexp),
                List(list) => self.inspect_list(depth, delimiter, list),
                Struct(struct_) => self.inspect_struct(depth, delimiter, struct_),
                _ => unreachable!("confirmed it was a container before reading")
            }
        } else {
            match value.lower().source() {
                ExpandedValueSource::ValueLiteral(value_literal) => {
                    use LazyRawValueKind::*;
                    match value_literal.kind() {
                        Binary_1_0(bin_val) => {
                            self.inspect_binary_1_0_scalar(depth, delimiter, value, bin_val)?;
                        }
                        Binary_1_1(_) => todo!(),
                        Text_1_0(_) | Text_1_1(_) => unreachable!("text value")
                    }
                }
                ExpandedValueSource::Template(_, _) => { todo!() }
                ExpandedValueSource::Constructed(_, _) => { todo!() }
            }
            Ok(())
        }
    }

    fn inspect_sexp<'x>(&mut self, depth: usize, delimiter: &str, sexp: LazySExp<'x, AnyEncoding>) -> Result<()> {
        use ExpandedSExpSource::*;
        let raw_sexp = match sexp.lower().source() {
            ValueLiteral(raw_sexp) => raw_sexp,
            Template(_, _, _, _) => todo!()
        };

        use LazyRawValueKind::*;
        match raw_sexp.as_value().kind() {
            Text_1_0(_) | Text_1_1(_) => unreachable!("text value"),
            Binary_1_0(v) => self.inspect_binary_1_0_sexp(depth, delimiter, sexp, v),
            Binary_1_1(_) => todo!(),
        }
    }

    fn inspect_list<'x>(&mut self, depth: usize, delimiter: &str, list: LazyList<'x, AnyEncoding>) -> Result<()> {
        use ExpandedListSource::*;
        let raw_list = match list.lower().source() {
            ValueLiteral(raw_list) => raw_list,
            Template(_, _, _, _) => todo!()
        };

        use LazyRawValueKind::*;
        match raw_list.as_value().kind() {
            Text_1_0(_) | Text_1_1(_) => unreachable!("text value"),
            Binary_1_0(v) => self.inspect_binary_1_0_list(depth, delimiter, list, v),
            Binary_1_1(_) => todo!(),
        }
    }

    fn inspect_binary_1_0_sexp<'x>(&mut self, depth: usize, delimiter: &str, sexp: LazySExp<'x, AnyEncoding>, raw_value: LazyRawBinaryValue_1_0) -> Result<()> {
        self.inspect_binary_1_0_sequence(depth, "(", "", ")", delimiter, sexp.iter(), raw_value)
    }

    fn inspect_binary_1_0_list<'x>(&mut self, depth: usize, delimiter: &str, list: LazyList<'x, AnyEncoding>, raw_value: LazyRawBinaryValue_1_0) -> Result<()> {
        self.inspect_binary_1_0_sequence(depth, "[", ",", "]", delimiter, list.iter(), raw_value)
    }

    fn inspect_binary_1_0_sequence<'x>(&mut self,
                                       depth: usize,
                                       opening_delimiter: &str,
                                       value_delimiter: &str,
                                       closing_delimiter: &str,
                                       trailing_delimiter: &str,
                                       values: impl IntoIterator<Item=IonResult<LazyValue<'x, AnyEncoding>>>,
                                       raw_value: LazyRawBinaryValue_1_0,
    ) -> Result<()> {
        let range = raw_value.range();
        let offset = range.start;
        let length = range.len();

        let opcode_bytes: &[u8] = &[raw_value.opcode()];
        let mut formatter = BytesFormatter::new(BYTES_PER_ROW, vec![
            IonBytes::new(BytesKind::Opcode, opcode_bytes),
            IonBytes::new(BytesKind::TrailingLength, raw_value.length_as_var_uint()),
        ]);

        self.write_offset_length_and_bytes(offset, length, &mut formatter)?;

        self.write_indentation(depth)?;
        self.with_style(text_ion_style(), |out| {
            write!(out, "{opening_delimiter}\n")?;
            Ok(())
        })?;

        let mut has_printed_skip_message = false;
        for value_res in values {
            let value = value_res?;
            if self.should_skip(value) {
                if !has_printed_skip_message {
                    self.write_blank_offset_length_and_bytes()?;
                    self.write_indentation(depth)?;
                    self.with_style(comment_style(), |out| {
                        write!(out, " // ...skipping values...\n")?;
                        Ok(())
                    })?;
                    has_printed_skip_message = true;
                }
            }
            self.inspect_value(depth + 1, value_delimiter, value)?;
        }

        self.write_blank_offset_length_and_bytes()?;
        self.write_indentation(depth)?;
        self.with_style(text_ion_style(), |out| {
            write!(out, "{closing_delimiter}{trailing_delimiter}\n")?;
            Ok(())
        })
    }

    fn inspect_struct(&mut self, depth: usize, delimiter: &str, struct_: LazyStruct<'_, AnyEncoding>) -> Result<()> {
        let raw_struct = match struct_.lower().source() {
            ExpandedStructSource::ValueLiteral(raw_struct) => raw_struct,
            ExpandedStructSource::Template(_, _, _, _, _) => todo!()
        };

        use LazyRawValueKind::*;
        match raw_struct.as_value().kind() {
            Binary_1_0(v) => self.inspect_binary_1_0_struct(depth, delimiter, struct_, raw_struct, v),
            Binary_1_1(_) => todo!(),
            Text_1_0(_) | Text_1_1(_) => unreachable!("text value"),
        }
    }

    fn inspect_binary_1_0_struct(&mut self, depth: usize, delimiter: &str, struct_: LazyStruct<AnyEncoding>, raw_struct: LazyRawAnyStruct, raw_value: LazyRawBinaryValue_1_0) -> Result<()> {
        let range = raw_value.range();
        let offset = range.start;
        let length = range.len();

        let opcode_bytes: &[u8] = &[raw_value.opcode()];
        let mut formatter = BytesFormatter::new(BYTES_PER_ROW, vec![
            IonBytes::new(BytesKind::Opcode, opcode_bytes),
            IonBytes::new(BytesKind::TrailingLength, raw_value.length_as_var_uint()),
        ]);

        self.write_offset_length_and_bytes(offset, length, &mut formatter)?;

        self.write_indentation(depth)?;
        self.with_style(text_ion_style(), |out| {
            write!(out, "{{\n")?;
            Ok(())
        })?;
        for (raw_field_result, field_result) in raw_struct.iter().zip(struct_.iter()) {
            let (raw_field, field) = (raw_field_result?, field_result?);
            let (raw_name, _raw_value) = raw_field.expect_name_value()?;
            let name = field.name()?;

            // Field name row
            let range = raw_name.range();
            let raw_name_bytes = raw_name.span();
            let offset = range.start;
            let length = range.len();
            let mut formatter = BytesFormatter::new(BYTES_PER_ROW, vec![
                IonBytes::new(BytesKind::FieldId, raw_name_bytes)
            ]);
            self.write_offset_length_and_bytes(offset, length, &mut formatter)?;

            self.write_indentation(depth + 1)?;
            self.with_style(field_id_style(), |out| {
                IoFmtShim::new(out).value_formatter().format_symbol(name)?;
                Ok(())
            })?;
            write!(self.output, ": ")?;
            self.with_style(comment_style(), |out| {
                match raw_name.read()? {
                    RawSymbolTokenRef::SymbolId(sid) => {
                        write!(out, " // ${sid}\n")
                    }
                    RawSymbolTokenRef::Text(_) => {
                        write!(out, " // <text>\n")
                    }
                }?;
                Ok(())
            })?;
            self.inspect_value(depth + 1, ",", field.value())?;
        }
        self.write_blank_offset_length_and_bytes()?;
        self.write_indentation(depth)?;
        self.with_style(text_ion_style(), |out| {
            write!(out, "}}{delimiter}\n")?;
            Ok(())
        })
    }

    fn inspect_annotations(&mut self, depth: usize, value: LazyValue<AnyEncoding>) -> Result<()> {
        let raw_value = match value.lower().source() {
            ExpandedValueSource::ValueLiteral(raw_value) => raw_value,
            ExpandedValueSource::Template(_, _) => todo!(),
            ExpandedValueSource::Constructed(_, _) => todo!()
        };

        use LazyRawValueKind::*;
        match raw_value.kind() {
            Binary_1_0(v) => self.inspect_binary_1_0_annotations(depth, value, v),
            Binary_1_1(_) => todo!(),
            Text_1_0(_) | Text_1_1(_) => unreachable!("text value"),
        }
    }

    fn inspect_binary_1_0_annotations(&mut self, depth: usize, value: LazyValue<AnyEncoding>, raw_value: LazyRawBinaryValue_1_0) -> Result<()> {
        let encoded_annotations = raw_value.encoded_annotations().unwrap();
        let range = encoded_annotations.range();
        let offset = range.start;
        let length = range.len();

        let mut formatter = BytesFormatter::new(BYTES_PER_ROW, vec![
            IonBytes::new(BytesKind::AnnotationsHeader, encoded_annotations.header_bytes()),
            IonBytes::new(BytesKind::AnnotationsSequence, encoded_annotations.sequence_bytes()),
        ]);
        self.write_offset_length_and_bytes(offset, length, &mut formatter)?;

        self.write_indentation(depth)?;
        self.with_style(annotations_style(), |out| {
            for annotation in value.annotations() {
                IoFmtShim::new(&mut *out).value_formatter().format_symbol(annotation?)?;
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
                    RawSymbolTokenRef::SymbolId(sid) => write!(out, "${sid}"),
                    RawSymbolTokenRef::Text(_) => write!(out, "<text>"),
                }?;
            }
            write!(out, "\n")?;
            Ok(())
        })?;

        Ok(())
    }

    fn inspect_binary_1_0_scalar(&mut self, depth: usize, delimiter: &str, value: LazyValue<AnyEncoding>, raw_value: LazyRawBinaryValue_1_0) -> Result<()> {
        if value.has_annotations() {
            self.inspect_binary_1_0_annotations(depth, value, raw_value)?;
        }

        let opcode_slice = &[raw_value.opcode()];
        let opcode_bytes = IonBytes::new(BytesKind::Opcode, opcode_slice);
        let length_bytes = IonBytes::new(BytesKind::TrailingLength, raw_value.length_as_var_uint());
        let body_bytes = IonBytes::new(BytesKind::ValueBody, raw_value.value_body().unwrap());

        let offset = raw_value.range().start;
        let length = raw_value.range().len();
        let mut formatter = BytesFormatter::new(BYTES_PER_ROW, vec![opcode_bytes, length_bytes, body_bytes]);

        self.write_offset_length_and_bytes(offset, length, &mut formatter)?;
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
            write!(self.output, "{VERTICAL_LINE} {:12} {VERTICAL_LINE} {:12} {VERTICAL_LINE} ", "", "")?;
            formatter.write_row(self.output)?;
            write!(self.output, "{VERTICAL_LINE}\n")?;
        }

        Ok(())
    }

    fn with_style(&mut self, style: ColorSpec, write_fn: impl FnOnce(&mut OutputRef) -> Result<()>) -> Result<()> {
        self.output.set_color(&style)?;
        write_fn(&mut self.output)?;
        self.output.reset()?;
        Ok(())
    }
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

#[derive(Copy, Clone, Debug)]
struct IonBytes<'a> {
    pub bytes: &'a [u8],
    pub kind: BytesKind,
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

struct BytesFormatter<'a> {
    slices: Vec<IonBytes<'a>>,
    slices_written: usize,
    formatted_bytes_per_row: usize,
}

impl<'a> BytesFormatter<'a> {
    pub fn new(formatted_bytes_per_row: usize, slices: Vec<IonBytes<'a>>) -> Self {
        Self { slices, slices_written: 0, formatted_bytes_per_row }
    }

    pub fn write_row(&mut self, output: &mut impl WriteColor) -> Result<()> {
        let num_bytes = self.formatted_bytes_per_row;
        let bytes_written = self.write_bytes(num_bytes, output)?;
        let bytes_remaining = num_bytes - bytes_written;
        for _ in 0..bytes_remaining {
            write!(output, "   ")?; // Empty space the width of a formatted byte
        }
        Ok(())
    }

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
            self.slices_written += 1;
        }

        Ok(bytes_to_write)
    }

    fn current_slice(&mut self) -> Option<&mut IonBytes<'a>> {
        if self.is_empty() {
            return None;
        }
        Some(&mut self.slices[self.slices_written])
    }

    fn is_empty(&self) -> bool {
        self.slices_written == self.slices.len()
    }
}

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

#[test]
fn do_it() -> Result<()> {
    // let bytes = std::fs::read("/tmp/some.ion").unwrap();
    let stdout = StandardStream::stdout(ColorChoice::Always);
    let mut output: Box<dyn WriteColor> = Box::new(stdout.lock());
    inspect_file("/tmp/some.ion", File::open("/tmp/some.ion").unwrap(), &mut output, 0, 0)?;
    Ok(())
}