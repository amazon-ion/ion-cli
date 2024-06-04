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

    fn configure_args(&self, command: Command) -> Command {
        command
            .with_input()
            .with_output()
            .with_format()
            .with_compression_control()
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        // --format pretty|text|lines|binary
        // `clap` validates the specified format and provides a default otherwise.
        let format = args.get_one::<String>("format").unwrap();

        CommandIo::new(args).for_each_input(|output, input| {
            let mut reader = Reader::new(AnyEncoding, input.into_source())?;
            write_all_as(&mut reader, output, format)?;
            Ok(())
        })
    }
}
