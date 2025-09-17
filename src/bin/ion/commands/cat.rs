use anyhow::Result;
use clap::{arg, ArgMatches, Command};
use ion_rs::*;

use crate::commands::timestamp_conversion::convert_timestamps;
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
            .arg(arg!(-t --"detect-timestamps" "Preserve Ion timestamps when going from Ion to JSON to Ion"))
            .with_input()
            .with_output()
            .with_format()
            .with_color()
            .with_ion_version()
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        let mapper = if args.get_flag("detect-timestamps") {
            // no-op that passes the element through unchanged
            convert_timestamps
        } else {
            |element| Ok(element)
        };

        CommandIo::new(args)?.for_each_input(|output, input| {
            let mut reader = Reader::new(AnyEncoding, input.into_source())?;
            write_all_as(
                &mut reader,
                output,
                *output.encoding(),
                *output.format(),
                mapper,
            )?;
            Ok(())
        })
    }
}
