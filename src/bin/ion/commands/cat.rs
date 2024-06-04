use std::fs::File;
use std::io::{stdin, stdout, BufReader, StdinLock, Write};

use anyhow::{Context, Result};
use clap::{Arg, ArgAction, ArgMatches, Command};
use ion_rs::*;

use crate::auto_decompress::auto_decompressing_reader;
use crate::commands::{IonCliCommand, WithIonCliArgument};
use crate::transcribe::write_all_as;

pub struct CatCommand;

const BUF_READER_CAPACITY: usize = 2 << 20; // 1 MiB
const INFER_HEADER_LENGTH: usize = 8;

impl IonCliCommand for CatCommand {
    fn name(&self) -> &'static str {
        "cat"
    }

    fn about(&self) -> &'static str {
        "Prints Ion in the requested format."
    }

    fn configure_args(&self, command: Command) -> Command {
        command.with_input().with_output().with_format().arg(
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
                    write_all_as(&mut reader, &mut output, format)?;
                } else {
                    let bfile = BufReader::with_capacity(BUF_READER_CAPACITY, file);
                    let zfile = auto_decompressing_reader(bfile, INFER_HEADER_LENGTH)?;
                    let mut reader = Reader::new(AnyEncoding, zfile)?;
                    write_all_as(&mut reader, &mut output, format)?;
                };
            }
        } else {
            let input: StdinLock = stdin().lock();
            if let Some(true) = args.get_one::<bool>("no-auto-decompress") {
                let mut reader = Reader::new(AnyEncoding, input)?;
                write_all_as(&mut reader, &mut output, format)?;
            } else {
                let zinput = auto_decompressing_reader(input, INFER_HEADER_LENGTH)?;
                let mut reader = Reader::new(AnyEncoding, zinput)?;
                write_all_as(&mut reader, &mut output, format)?;
            };
        }

        output.flush()?;
        Ok(())
    }
}
