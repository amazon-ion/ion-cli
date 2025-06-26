use anyhow::Result;
use clap::{ArgMatches, Command};
use ion_rs::*;

use crate::commands::{CommandIo, IonCliCommand, WithIonCliArgument};
use crate::transcribe::write_all_as;

pub struct CatCommand;

impl IonCliCommand for CatCommand {
    fn name(&self) -> &'static str {
        "cat"
    }

    fn about(&self) -> &'static str {
        "Prints all Ion input files to the specified output in the requested format."
    }

    fn is_stable(&self) -> bool {
        true
    }

    fn is_porcelain(&self) -> bool {
        false
    }

    fn configure_args(&self, command: Command) -> Command {
        command
            .alias("dump")
            .with_input()
            .with_output()
            .with_format()
            .with_color()
            .with_ion_version()
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        CommandIo::new(args)?.for_each_input(|output, input| {
            let mut reader = Reader::new(AnyEncoding, input.into_source())?;
            let encoding = *output.encoding();
            let format = *output.format();
            write_all_as(&mut reader, output, encoding, format)?;
            Ok(())
        })
    }
}
