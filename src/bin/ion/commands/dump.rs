use anyhow::Result;
use clap::{ArgMatches, Command};

use crate::commands::cat::CatCommand;
use crate::commands::IonCliCommand;

// This command has been renamed to `cat` but is being preserved for the time being
// for the sake of compatability with existing shell scripts.
pub struct DumpCommand;

impl IonCliCommand for DumpCommand {
    fn name(&self) -> &'static str {
        "dump"
    }

    fn about(&self) -> &'static str {
        "Deprecated alias for the `cat` command."
    }

    fn configure_args(&self, command: Command) -> Command {
        CatCommand.configure_args(command)
    }

    fn run(&self, command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        CatCommand.run(command_path, args)
    }
}
