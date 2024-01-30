use crate::commands::{IonCliCommand, WithIonCliArgument};
use anyhow::{bail, Context, Result};
use clap::{ArgMatches, Command};
use ion_rs::*;
use memmap::MmapOptions;
use std::fs::File;

pub struct SymbolNumberCommand;

impl IonCliCommand for SymbolNumberCommand {
    fn name(&self) -> &'static str {
        "symbol_count"
    }

    fn about(&self) -> &'static str {
        "Prints the number of symbols."
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
                symbol_count(&mut system_reader)
                    .expect("Failed to get the number of symbols from the input ion stream.");
            }
        } else {
            bail!("this command does not yet support reading from STDIN")
        }
        Ok(())
    }
}

fn symbol_count(reader: &mut SystemReader<RawBinaryReader<&[u8]>>) -> Result<()> {
    let mut count = 0;
    loop {
        match reader.next()? {
            SystemStreamItem::Value(_) => {
                let symbols_len = reader.symbol_table().symbols().iter().len();
                // Reduce the number of system symbols.
                count += symbols_len - 10;
            }
            SystemStreamItem::Nothing => break,
            _ => {}
        }
    }
    println!("The number of symbols is {}", count);
    Ok(())
}
