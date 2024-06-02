use crate::commands::{IonCliCommand, WithIonCliArgument};
use anyhow::{Context, Result};
use clap::{value_parser, Arg, ArgAction, ArgMatches, Command};
use ion_rs::*;
use std::fs::File;
use std::io::{self, stdin, stdout, BufRead, BufReader, Chain, Cursor, Read, StdinLock, Write};

pub struct DumpCommand;

const BUF_READER_CAPACITY: usize = 2 << 20; // 1 MiB
const INFER_HEADER_LENGTH: usize = 8;

impl IonCliCommand for DumpCommand {
    fn name(&self) -> &'static str {
        "dump"
    }

    fn about(&self) -> &'static str {
        "Prints Ion in the requested format."
    }

    fn configure_args(&self, command: Command) -> Command {
        //TODO: Remove `values` after https://github.com/amazon-ion/ion-cli/issues/49
        command
            .arg(
                Arg::new("values")
                    .long("values")
                    .short('n')
                    .value_parser(value_parser!(usize))
                    .allow_negative_numbers(false)
                    .hide(true)
                    .help("Specifies the number of output top-level values."),
            )
            .with_input()
            .with_output()
            .with_format()
            .arg(
                Arg::new("no-auto-decompress")
                    .long("no-auto-decompress")
                    .action(ArgAction::SetTrue)
                    .help("Turn off automatic decompression detection."),
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
                if let Some(true) = args.get_one::<bool>("no-auto-decompress") {
                    let mut reader = Reader::new(AnyEncoding, file)?;
                    write_in_format(&mut reader, &mut output, format, values)?;
                } else {
                    let bfile = BufReader::with_capacity(BUF_READER_CAPACITY, file);
                    let zfile = auto_decompressing_reader(bfile, INFER_HEADER_LENGTH)?;
                    let mut reader = Reader::new(AnyEncoding, zfile)?;
                    write_in_format(&mut reader, &mut output, format, values)?;
                };
            }
        } else {
            let input: StdinLock = stdin().lock();
            if let Some(true) = args.get_one::<bool>("no-auto-decompress") {
                let mut reader = Reader::new(AnyEncoding, input)?;
                write_in_format(&mut reader, &mut output, format, values)?;
            } else {
                let zinput = auto_decompressing_reader(input, INFER_HEADER_LENGTH)?;
                let mut reader = Reader::new(AnyEncoding, zinput)?;
                write_in_format(&mut reader, &mut output, format, values)?;
            };
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
pub(crate) fn write_in_format<I: IonInput>(
    reader: &mut Reader<AnyEncoding, I>,
    output: &mut Box<dyn Write>,
    format: &str,
    count: Option<usize>,
) -> IonResult<usize> {
    let written = match format {
        "pretty" => {
            let mut writer = Writer::new(v1_0::Text.with_format(TextFormat::Pretty), output)?;
            transcribe_n_values(reader, &mut writer, count)
        }
        "text" => {
            let mut writer = Writer::new(v1_0::Text.with_format(TextFormat::Compact), output)?;
            transcribe_n_values(reader, &mut writer, count)
        }
        "lines" => {
            let mut writer = Writer::new(v1_0::Text.with_format(TextFormat::Lines), output)?;
            transcribe_n_values(reader, &mut writer, count)
        }
        "binary" => {
            let mut writer = Writer::new(v1_0::Binary, output)?;
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
fn transcribe_n_values<I: IonInput, E: Encoding>(
    reader: &mut Reader<AnyEncoding, I>,
    writer: &mut Writer<E, &mut Box<dyn Write>>,
    count: Option<usize>,
) -> IonResult<usize> {
    const FLUSH_EVERY_N: usize = 100;
    let mut values_since_flush: usize = 0;
    let max_items = count.unwrap_or(usize::MAX);
    let mut index: usize = 0;

    while let Some(value) = reader.next()? {
        if index >= max_items {
            break;
        }

        writer.write(value)?;

        index += 1;
        values_since_flush += 1;
        if values_since_flush == FLUSH_EVERY_N {
            writer.flush()?;
            values_since_flush = 0;
        }
    }

    writer.flush()?;
    Ok(index)
}

/// Auto-detects a compressed byte stream and wraps the original reader
/// into a reader that transparently decompresses.
///
/// To support non-seekable readers like `Stdin`, we could have used a
/// full-blown buffering wrapper with unlimited rewinds, but since we only
/// need the first few magic bytes at offset 0, we cheat and instead make a
/// `Chain` reader from the buffered header followed by the original reader.
///
/// The choice of `Chain` type here is not quite necessary: it could have
/// been simply `dyn BufRead`, but there is no `ToIonDataSource` trait
/// implementation for `dyn BufRead` at the moment.
type AutoDecompressingReader = Chain<Box<dyn BufRead>, Box<dyn BufRead>>;

fn auto_decompressing_reader<R>(
    mut reader: R,
    header_len: usize,
) -> IonResult<AutoDecompressingReader>
where
    R: BufRead + 'static,
{
    // read header
    let mut header_bytes = vec![0; header_len];
    let nread = read_reliably(&mut reader, &mut header_bytes)?;
    header_bytes.truncate(nread);

    // detect compression type and wrap reader in a decompressor
    match infer::get(&header_bytes) {
        Some(t) => match t.extension() {
            "gz" => {
                // "rewind" to let the decompressor read magic bytes again
                let header: Box<dyn BufRead> = Box::new(Cursor::new(header_bytes));
                let chain = header.chain(reader);
                let zreader = Box::new(BufReader::new(flate2::read::GzDecoder::new(chain)));
                // must return a `Chain`, so prepend an empty buffer
                let nothing: Box<dyn BufRead> = Box::new(Cursor::new(&[] as &[u8]));
                Ok(nothing.chain(zreader))
            }
            "zst" => {
                let header: Box<dyn BufRead> = Box::new(Cursor::new(header_bytes));
                let chain = header.chain(reader);
                let zreader = Box::new(BufReader::new(zstd::stream::read::Decoder::new(chain)?));
                let nothing: Box<dyn BufRead> = Box::new(Cursor::new(&[] as &[u8]));
                Ok(nothing.chain(zreader))
            }
            _ => {
                let header: Box<dyn BufRead> = Box::new(Cursor::new(header_bytes));
                Ok(header.chain(Box::new(reader)))
            }
        },
        None => {
            let header: Box<dyn BufRead> = Box::new(Cursor::new(header_bytes));
            Ok(header.chain(Box::new(reader)))
        }
    }
}

/// same as `Read` trait's read() method, but loops in case of fragmented reads
fn read_reliably<R: Read>(reader: &mut R, buf: &mut [u8]) -> io::Result<usize> {
    let mut nread = 0;
    while nread < buf.len() {
        match reader.read(&mut buf[nread..]) {
            Ok(0) => break,
            Ok(n) => nread += n,
            Err(e) => return Err(e),
        }
    }
    Ok(nread)
}
