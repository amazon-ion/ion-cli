use anyhow::Result;
use clap::{ArgMatches, Command};

use crate::commands::cat::CatCommand;
use crate::commands::{IonCliCommand, WithIonCliArgument};

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

    fn run(&self, command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        // Because JSON data is valid Ion, the `cat` command may be reused for converting JSON.
        // TODO ideally, this would perform some smarter "up-conversion".
        CatCommand.run(command_path, args)
    }
}
