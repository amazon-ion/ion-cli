use anyhow::Result;
use clap::{arg, ArgMatches, Command};
use ion_rs::{AnyEncoding, Reader};

use crate::commands::timestamp_conversion::convert_timestamps;
use crate::commands::{CommandIo, IonCliCommand, WithIonCliArgument};
use crate::transcribe::write_all_as;

pub struct FromJsonCommand;

impl IonCliCommand for FromJsonCommand {
    fn name(&self) -> &'static str {
        "json"
    }

    fn about(&self) -> &'static str {
        "Converts data from JSON to Ion."
    }

    fn is_stable(&self) -> bool {
        false // TODO: Should this be true?
    }

    fn is_porcelain(&self) -> bool {
        false
    }

    fn configure_args(&self, command: Command) -> Command {
        command
            .arg(arg!(-t --"detect-timestamps" "Preserve Ion timestamps when going from Ion to JSON to Ion"))
            .with_input()
            .with_output()
            .with_format()
            .with_ion_version()
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        // Because JSON data is valid Ion, the `cat` command may be reused for converting JSON.
        let detect_timestamps = args.get_flag("detect-timestamps");

        CommandIo::new(args)?.for_each_input(|output, input| {
            let mut reader = Reader::new(AnyEncoding, input.into_source())?;
            let mapper = detect_timestamps.then_some(convert_timestamps);
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
