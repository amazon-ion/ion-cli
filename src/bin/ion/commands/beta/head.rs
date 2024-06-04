use std::fs::File;
use std::io::{stdin, stdout, BufReader, StdinLock, Write};

use anyhow::{Context, Result};
use clap::{value_parser, Arg, ArgMatches, Command};
use ion_rs::{AnyEncoding, Reader};

use crate::auto_decompress::auto_decompressing_reader;
use crate::commands::cat::CatCommand;
use crate::commands::IonCliCommand;
use crate::transcribe::write_n_as;

pub struct HeadCommand;

const BUF_READER_CAPACITY: usize = 2 << 20; // 1 MiB
const INFER_HEADER_LENGTH: usize = 8;

impl IonCliCommand for HeadCommand {
    fn name(&self) -> &'static str {
        "head"
    }

    fn about(&self) -> &'static str {
        "Prints the specified number of top-level values in the input stream."
    }

    fn configure_args(&self, command: Command) -> Command {
        // Same flags as `cat`, but with an added `--values` flag to specify the number of values to
        // write.
        CatCommand.configure_args(command).arg(
            Arg::new("values")
                .long("values")
                .short('n')
                .value_parser(value_parser!(usize))
                .allow_negative_numbers(false)
                .default_value("10")
                .help("Specifies the number of output top-level values."),
        )
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        //TODO: Multiple file handling in classic `head` includes a header per file.
        // https://github.com/amazon-ion/ion-cli/issues/48

        // --format pretty|text|lines|binary
        // `clap` validates the specified format and provides a default otherwise.
        let format = args.get_one::<String>("format").unwrap();

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

        let num_values = *args.get_one::<usize>("values").unwrap();

        if let Some(input_file_iter) = args.get_many::<String>("input") {
            for input_file in input_file_iter {
                let file = File::open(input_file)
                    .with_context(|| format!("Could not open file '{}'", input_file))?;
                if let Some(true) = args.get_one::<bool>("no-auto-decompress") {
                    let mut reader = Reader::new(AnyEncoding, file)?;
                    write_n_as(&mut reader, &mut output, format, num_values)?;
                } else {
                    let bfile = BufReader::with_capacity(BUF_READER_CAPACITY, file);
                    let zfile = auto_decompressing_reader(bfile, INFER_HEADER_LENGTH)?;
                    let mut reader = Reader::new(AnyEncoding, zfile)?;
                    write_n_as(&mut reader, &mut output, format, num_values)?;
                };
            }
        } else {
            let input: StdinLock = stdin().lock();
            if let Some(true) = args.get_one::<bool>("no-auto-decompress") {
                let mut reader = Reader::new(AnyEncoding, input)?;
                write_n_as(&mut reader, &mut output, format, num_values)?;
            } else {
                let zinput = auto_decompressing_reader(input, INFER_HEADER_LENGTH)?;
                let mut reader = Reader::new(AnyEncoding, zinput)?;
                write_n_as(&mut reader, &mut output, format, num_values)?;
            };
        }

        output.flush()?;
        Ok(())
    }
}
