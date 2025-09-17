use anyhow::Result;
use clap::{arg, ArgMatches, Command};
use ion_rs::{AnyEncoding, Element, Reader};

use crate::commands::timestamp_conversion::convert_timestamps;
use crate::commands::{CommandIo, IonCliCommand, WithIonCliArgument};
use crate::input::CommandInput;
use crate::output::CommandOutput;

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
        // TODO ideally, this would perform some smarter "up-conversion".
        let detect_timestamps = args.get_flag("detect-timestamps");
        CommandIo::new(args)?
            .for_each_input(|output, input| convert(input, output, detect_timestamps))
    }
}

pub fn convert(
    input: CommandInput,
    output: &mut CommandOutput,
    detect_timestamps: bool,
) -> Result<()> {
    const FLUSH_EVERY_N: usize = 100;
    let mut writer = output.as_writer()?;
    let mut value_count = 0usize;
    let mut ion_reader = Reader::new(AnyEncoding, input.into_source())?;

    let mapper = if detect_timestamps {
        convert_timestamps
    } else {
        |element| Ok(element) // Identity mapper
    };

    while let Some(lazy_value) = ion_reader.next()? {
        let value_ref = lazy_value.read()?;
        let element = Element::try_from(value_ref)?;
        let converted_element = mapper(element)?;
        writer.write(&converted_element)?;
        value_count += 1;
        if value_count % FLUSH_EVERY_N == 0 {
            writer.flush()?;
        }
    }

    writer.close().map_err(Into::into)
}
