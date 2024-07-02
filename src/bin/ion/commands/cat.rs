use anyhow::{bail, Result};
use clap::{ArgMatches, Command};
use ion_rs::*;

use crate::commands::{CommandIo, IonCliCommand, WithIonCliArgument, ION_VERSION_ARG_ID};
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
            .with_ion_version()
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        // --format pretty|text|lines|binary
        // `clap` validates the specified format and provides a default otherwise.
        let format: Format = match args.get_one::<String>("format").unwrap().as_str() {
            "text" => Format::Text(TextFormat::Compact),
            "lines" => Format::Text(TextFormat::Lines),
            "pretty" => Format::Text(TextFormat::Pretty),
            "binary" => Format::Binary,
            unrecognized => bail!("unsupported format '{unrecognized}'"),
        };
        let encoding = match (
            args.get_one::<String>(ION_VERSION_ARG_ID).unwrap().as_str(),
            format,
        ) {
            ("1.0", Format::Text(_)) => IonEncoding::Text_1_0,
            ("1.0", Format::Binary) => IonEncoding::Binary_1_0,
            ("1.1", Format::Text(_)) => IonEncoding::Text_1_1,
            ("1.1", Format::Binary) => IonEncoding::Binary_1_1,
            (unrecognized, _) => bail!("unrecognized Ion version '{unrecognized}'"),
        };

        CommandIo::new(args).for_each_input(|output, input| {
            let mut reader = Reader::new(AnyEncoding, input.into_source())?;
            write_all_as(&mut reader, output, encoding, format)?;
            Ok(())
        })
    }
}
