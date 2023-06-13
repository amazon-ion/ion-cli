use crate::commands::IonCliCommand;
use anyhow::{Context, Result};
use clap::{value_parser, Arg, ArgAction, ArgMatches, Command};
use ion_rs::*;
use std::fs::File;
use std::io::{stdin, stdout, StdinLock, Write};

pub struct DumpCommand;

impl IonCliCommand for DumpCommand {
    fn name(&self) -> &'static str {
        "dump"
    }

    fn about(&self) -> &'static str {
        "Prints Ion in the requested format"
    }

    fn clap_command(&self) -> Command {
        Command::new(self.name())
            .about(self.about())
            //TODO: Remove `values` after https://github.com/amazon-ion/ion-cli/issues/49
            .arg(
                Arg::new("values")
                    .long("values")
                    .short('n')
                    .value_parser(value_parser!(usize))
                    .allow_negative_numbers(false)
                    .hide(true)
                    .help("Specifies the number of output top-level values."),
            )
            .arg(
                Arg::new("format")
                    .long("format")
                    .short('f')
                    .default_value("pretty")
                    .value_parser(["binary", "text", "pretty", "lines"])
                    .help("Output format"),
            )
            .arg(
                Arg::new("output")
                    .long("output")
                    .short('o')
                    .help("Output file [default: STDOUT]"),
            )
            .arg(
                // All argv entries after the program name (argv[0])
                // and any `clap`-managed options are considered input files.
                Arg::new("input")
                    .index(1)
                    .help("Input file [default: STDIN]")
                    .action(ArgAction::Append)
                    .trailing_var_arg(true),
            )
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        // --format pretty|text|lines|binary
        // `clap` validates the specified format and provides a default otherwise.
        let format = args.get_one::<String>("format").unwrap();

        // --values <n>
        // this value is supplied when `dump` is invoked as `head`
        let values: Option<usize> = args.get_one::<usize>("values").copied();

        // -o filename
        let mut output: Box<dyn Write> = if let Some(output_file) = args.get_one::<String>("output")
        {
            let file = File::create(output_file).with_context(|| {
                format!(
                    "could not open file output file '{}' for writing",
                    output_file
                )
            })?;
            Box::new(file)
        } else {
            Box::new(stdout().lock())
        };

        if let Some(input_file_iter) = args.get_many::<String>("input") {
            for input_file in input_file_iter {
                let file = File::open(input_file)
                    .with_context(|| format!("Could not open file '{}'", input_file))?;
                let mut reader = ReaderBuilder::new().build(file)?;
                write_in_format(&mut reader, &mut output, format, values)?;
            }
        } else {
            let input: StdinLock = stdin().lock();
            let mut reader = ReaderBuilder::new().build(input)?;
            write_in_format(&mut reader, &mut output, format, values)?;
        }

        output.flush()?;
        Ok(())
    }
}

// TODO: This is a compatibility shim. Several commands refer to dump::run(); that functionality
//       now lives in `dump`'s implementation of the IonCliCommand trait. These referents should
//       be updated to refer to functionality in a common module so this can be removed.
//       See: https://github.com/amazon-ion/ion-cli/issues/49
pub(crate) fn run(_command: &str, args: &ArgMatches) -> Result<()> {
    DumpCommand.run(&mut Vec::new(), args)
}

/// Constructs the appropriate writer for the given format, then writes all values found in the
/// Reader to the new Writer. If `count` is specified will write at most `count` values.
pub(crate) fn write_in_format(
    reader: &mut Reader,
    output: &mut Box<dyn Write>,
    format: &str,
    count: Option<usize>,
) -> IonResult<usize> {
    // XXX: The text formats below each have additional logic to append a newline because the
    //      ion-rs writer doesn't handle this automatically like it should.
    //TODO: Solve these newline issues, get rid of hack
    // https://github.com/amazon-ion/ion-cli/issues/36
    // https://github.com/amazon-ion/ion-rust/issues/437
    const NEWLINE: u8 = 0x0A;
    let written = match format {
        "pretty" => {
            let mut writer = TextWriterBuilder::pretty().build(output)?;
            let values_written = transcribe_n_values(reader, &mut writer, count)?;
            writer.output_mut().write_all(&[NEWLINE])?;
            Ok(values_written)
        }
        "text" => {
            let mut writer = TextWriterBuilder::default().build(output)?;
            let values_written = transcribe_n_values(reader, &mut writer, count)?;
            writer.output_mut().write_all(&[NEWLINE])?;
            Ok(values_written)
        }
        "lines" => {
            let mut writer = TextWriterBuilder::lines().build(output)?;
            let values_written = transcribe_n_values(reader, &mut writer, count)?;
            writer.output_mut().write_all(&[NEWLINE])?;
            Ok(values_written)
        }
        "binary" => {
            let mut writer = BinaryWriterBuilder::new().build(output)?;
            transcribe_n_values(reader, &mut writer, count)
        }
        unrecognized => unreachable!(
            "'format' was '{}' instead of 'pretty', 'text', 'lines', or 'binary'",
            unrecognized
        ),
    }?;
    Ok(written)
}

/// Writes each value encountered in the Reader to the provided IonWriter. If `count` is specified
/// will write at most `count` values.
fn transcribe_n_values<W: IonWriter>(
    reader: &mut Reader,
    writer: &mut W,
    count: Option<usize>,
) -> IonResult<usize> {
    const FLUSH_EVERY_N: usize = 100;
    let mut values_since_flush: usize = 0;
    let mut annotations = vec![];
    let mut index = 0;
    loop {
        // Could use Option::is_some_and if that reaches stable
        if reader.depth() == 0 && matches!(count, Some(n) if n <= index) {
            break;
        }

        match reader.next()? {
            StreamItem::Value(ion_type) | StreamItem::Null(ion_type) => {
                if reader.has_annotations() {
                    annotations.clear();
                    for annotation in reader.annotations() {
                        annotations.push(annotation?);
                    }
                    writer.set_annotations(&annotations);
                }

                if reader.parent_type() == Some(IonType::Struct) {
                    writer.set_field_name(reader.field_name()?);
                }

                if reader.is_null() {
                    writer.write_null(ion_type)?;
                    continue;
                }

                use IonType::*;
                match ion_type {
                    Null => unreachable!("null values are handled prior to this match"),
                    Bool => writer.write_bool(reader.read_bool()?)?,
                    Int => writer.write_int(&reader.read_int()?)?,
                    Float => {
                        let float64 = reader.read_f64()?;
                        let float32 = float64 as f32;
                        if float32 as f64 == float64 {
                            // No data lost during cast; write it as an f32
                            writer.write_f32(float32)?;
                        } else {
                            writer.write_f64(float64)?;
                        }
                    }
                    Decimal => writer.write_decimal(&reader.read_decimal()?)?,
                    Timestamp => writer.write_timestamp(&reader.read_timestamp()?)?,
                    Symbol => writer.write_symbol(reader.read_symbol()?)?,
                    String => writer.write_string(reader.read_string()?)?,
                    Clob => writer.write_clob(reader.read_clob()?)?,
                    Blob => writer.write_blob(reader.read_blob()?)?,
                    List => {
                        reader.step_in()?;
                        writer.step_in(List)?;
                    }
                    SExp => {
                        reader.step_in()?;
                        writer.step_in(SExp)?;
                    }
                    Struct => {
                        reader.step_in()?;
                        writer.step_in(Struct)?;
                    }
                }
            }
            StreamItem::Nothing if reader.depth() > 0 => {
                reader.step_out()?;
                writer.step_out()?;
            }
            StreamItem::Nothing => break,
        }
        if reader.depth() == 0 {
            index += 1;
            values_since_flush += 1;
            if values_since_flush == FLUSH_EVERY_N {
                writer.flush()?;
                values_since_flush = 0;
            }
        }
    }
    writer.flush()?;
    Ok(index)
}
