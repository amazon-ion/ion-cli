use crate::commands::{dump, IonCliCommand, WithIonCliArgument};
use anyhow::Result;
use clap::{value_parser, Arg, ArgAction, ArgMatches, Command};

pub struct HeadCommand;

impl IonCliCommand for HeadCommand {
    fn name(&self) -> &'static str {
        "head"
    }

    fn about(&self) -> &'static str {
        "Prints the specified number of top-level values in the input stream."
    }

    fn configure_args(&self, command: Command) -> Command {
        command
            .with_input()
            .with_output()
            .with_format()
            .arg(
                Arg::new("values")
                    .long("values")
                    .short('n')
                    .value_parser(value_parser!(usize))
                    .allow_negative_numbers(false)
                    .default_value("10")
                    .help("Specifies the number of output top-level values."),
            )
            .arg(
                Arg::new("no-auto-decompress")
                    .long("no-auto-decompress")
                    .action(ArgAction::SetTrue)
                    .help("Turn off automatic decompression detection."),
            )
    }

    fn run(&self, command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        //TODO: Extract common value-handling logic for both `head` and `dump`
        // https://github.com/amazon-ion/ion-cli/issues/49
        //TODO: Multiple file handling in classic `head` includes a header per file.
        // https://github.com/amazon-ion/ion-cli/issues/48
        dump::run(command_path.last().unwrap(), args)
    }
}
