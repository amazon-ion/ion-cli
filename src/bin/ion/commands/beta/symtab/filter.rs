use std::io::Write;

use anyhow::{bail, Result};
use clap::{Arg, ArgAction, ArgMatches, Command};
use ion_rs::*;

use crate::commands::{CommandIo, IonCliCommand, WithIonCliArgument};
use crate::output::CommandOutput;

pub struct SymtabFilterCommand;

impl IonCliCommand for SymtabFilterCommand {
    fn name(&self) -> &'static str {
        "filter"
    }

    fn about(&self) -> &'static str {
        "Filters user data out of an Ion stream, leaving only the symbol table(s) behind."
    }

    fn configure_args(&self, command: Command) -> Command {
        command.with_input()
            .with_output()
            .with_compression_control()
            .arg(Arg::new("lift")
                .long("lift")
                .short('l')
                .required(false)
                .action(ArgAction::SetTrue)
                .help("Remove the `$ion_symbol_table` annotation from symtabs, turning them into visible user data")
            )
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        let lift_requested = args.get_flag("lift");
        CommandIo::new(args).for_each_input(|output, input| {
            let mut system_reader = SystemReader::new(AnyEncoding, input.into_source());
            filter_out_user_data(&mut system_reader, output, lift_requested)
        })
    }
}

pub fn filter_out_user_data(
    reader: &mut SystemReader<AnyEncoding, impl IonInput>,
    output: &mut CommandOutput,
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
