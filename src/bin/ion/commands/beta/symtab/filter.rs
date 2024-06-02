use crate::commands::{IonCliCommand, WithIonCliArgument};
use anyhow::{bail, Context, Result};
use clap::{Arg, ArgAction, ArgMatches, Command};
use ion_rs::*;
use std::fs::File;
use std::io;
use std::io::{stdout, BufWriter, Write};

pub struct SymtabFilterCommand;

impl IonCliCommand for SymtabFilterCommand {
    fn name(&self) -> &'static str {
        "filter"
    }

    fn about(&self) -> &'static str {
        // XXX Currently only supports binary input
        "Filters user data out of an Ion stream, leaving only the symbol table(s) behind."
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
                let mut system_reader = SystemReader::new(AnyEncoding, file);
                filter_out_user_data(&mut system_reader, &mut output, lift_requested)?;
            }
        } else {
            let mut system_reader = SystemReader::new(AnyEncoding, io::stdin().lock());
            filter_out_user_data(&mut system_reader, &mut output, lift_requested)?;
        }

        output.flush()?;
        Ok(())
    }
}

pub fn filter_out_user_data(
    reader: &mut SystemReader<AnyEncoding, impl IonInput>,
    output: &mut Box<dyn Write>,
    lift_requested: bool,
) -> Result<()> {
    loop {
        match reader.next_item()? {
            SystemStreamItem::VersionMarker(marker) => {
                output.write_all(marker.span().bytes())?;
            }
            SystemStreamItem::SymbolTable(symtab) => {
                let Some(raw_value) = symtab.as_value().raw() else {
                    // This symbol table came from a macro expansion; there are no encoded bytes
                    // to pass through.
                    bail!("found an ephemeral symbol table, which is not yet supported")
                };
                if lift_requested {
                    // Only pass through the value portion of the symbol table, stripping off the
                    // `$ion_symbol_table` annotation.
                    output.write_all(raw_value.value_span().bytes())?;
                } else {
                    // Pass through the complete symbol table, preserving the `$ion_symbol_table`
                    // annotation.
                    output.write_all(raw_value.span().bytes())?;
                }
            }
            SystemStreamItem::Value(_) => continue,
            SystemStreamItem::EndOfStream(_) => {
                return Ok(());
            }
            _ => unreachable!("#[non_exhaustive] enum, current variants covered"),
        };
        // If this is a text encoding, then we need delimiting space to separate
        // IVMs from their neighboring system stream items. Consider:
        //     $ion_1_0$ion_1_0
        // or
        //     $ion_symbol_table::{}$ion_1_0$ion_symbol_table::{}
        if reader.detected_encoding().is_text() {
            output.write_all(&[b'\n']).unwrap()
        }
    }
}
