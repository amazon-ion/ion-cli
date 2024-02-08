use crate::commands::{IonCliCommand, WithIonCliArgument};
use anyhow::{bail, Context, Result};
use clap::{Arg, ArgAction, ArgMatches, Command};
use ion_rs::RawBinaryReader;
use ion_rs::{IonReader, IonResult, IonType, SystemReader, SystemStreamItem};
use memmap::MmapOptions;
use std::fs::File;
use std::io::{stdout, BufWriter, Write};

pub struct SymtabFilterCommand;

impl IonCliCommand for SymtabFilterCommand {
    fn name(&self) -> &'static str {
        "filter"
    }

    fn about(&self) -> &'static str {
        // XXX Currently only supports binary input
        "Filters user data out of a binary Ion stream, leaving only the symbol table(s) behind."
    }

    fn configure_args(&self, command: Command) -> Command {
        command.with_input()
            .with_output()
            .arg(Arg::new("lift")
                .long("lift")
                .short('l')
                .required(false)
                .action(ArgAction::SetTrue)
                .help("Remove the `$ion_symbol_table` annotation from symtabs, turning them into visible user data")
            )
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        let mut output: Box<dyn Write> = if let Some(output_file) = args.get_one::<String>("output")
        {
            let file = File::create(output_file).with_context(|| {
                format!(
                    "could not open file output file '{}' for writing",
                    output_file
                )
            })?;
            Box::new(BufWriter::new(file))
        } else {
            Box::new(stdout().lock())
        };

        let lift_requested = args.get_flag("lift");

        if let Some(input_file_names) = args.get_many::<String>("input") {
            // Input files were specified, run the converter on each of them in turn
            for input_file in input_file_names {
                let file = File::open(input_file.as_str())
                    .with_context(|| format!("Could not open file '{}'", &input_file))?;

                let mmap = unsafe {
                    MmapOptions::new()
                        .map(&file)
                        .with_context(|| format!("Could not mmap '{}'", input_file))?
                };

                // Treat the mmap as a byte array.
                let ion_data: &[u8] = &mmap[..];
                let raw_reader = RawBinaryReader::new(ion_data);
                let mut system_reader = SystemReader::new(raw_reader);
                omit_user_data(ion_data, &mut system_reader, &mut output, lift_requested)?;
            }
        } else {
            bail!("this command does not yet support reading from STDIN")
        }

        output.flush()?;
        Ok(())
    }
}

pub fn omit_user_data(
    ion_data: &[u8],
    reader: &mut SystemReader<RawBinaryReader<&[u8]>>,
    output: &mut Box<dyn Write>,
    lift_requested: bool,
) -> IonResult<()> {
    loop {
        match reader.next()? {
            SystemStreamItem::VersionMarker(major, minor) => {
                output.write_all(&[0xE0, major, minor, 0xEA])?;
            }
            SystemStreamItem::SymbolTableValue(IonType::Struct) => {
                if !lift_requested {
                    output.write_all(reader.raw_annotations_bytes().unwrap_or(&[]))?;
                }
                output.write_all(reader.raw_header_bytes().unwrap())?;
                let body_range = reader.value_range();
                let body_bytes = &ion_data[body_range];
                output.write_all(body_bytes)?;
            }
            SystemStreamItem::Nothing => return Ok(()),
            _ => {}
        }
    }
}
