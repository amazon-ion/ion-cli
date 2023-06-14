use crate::commands::{dump, IonCliCommand, WithIonCliArgument};
use anyhow::Result;
use clap::{ArgMatches, Command};

pub struct FromJsonCommand;

impl IonCliCommand for FromJsonCommand {
    fn name(&self) -> &'static str {
        "json"
    }

    fn about(&self) -> &'static str {
        "Converts data from JSON to Ion."
    }

    fn configure_args(&self, command: Command) -> Command {
        command.with_input().with_output().with_format()
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        // Because JSON data is valid Ion, the `dump` command may be reused for converting JSON.
        // TODO ideally, this would perform some smarter "up-conversion".
        dump::run("json", args)
    }
}
