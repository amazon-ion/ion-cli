use crate::commands::{IonCliCommand, WithIonCliArgument};
use anyhow::{bail, Context, Result};
use clap::{ArgMatches, Command};
use ion_rs::*;
use memmap::MmapOptions;
use std::fs::File;

pub struct SymbolTableCommand;

impl IonCliCommand for SymbolTableCommand {
    fn name(&self) -> &'static str {
        "count"
    }

    fn about(&self) -> &'static str {
        "Prints the number of local symbol tables."
    }

    fn configure_args(&self, command: Command) -> Command {
        command.with_input()
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
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
                count_symbol_tables(&mut system_reader)
                    .expect("Failed to get the number of local symbol tables of input stream.");
            }
        } else {
            bail!("this command does not yet support reading from STDIN")
        }
        Ok(())
    }
}

fn count_symbol_tables(reader: &mut SystemReader<RawBinaryReader<&[u8]>>) -> Result<()> {
    let mut count = 0;
    loop {
        match reader.next()? {
            SystemStreamItem::SymbolTableValue(IonType::Struct) => {
                count += 1;
            }
            SystemStreamItem::Nothing => break,
            _ => {}
        }
    }
    println!("The number of local symbol tables is {} ", count);
    Ok(())
}
