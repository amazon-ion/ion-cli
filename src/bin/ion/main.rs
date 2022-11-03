mod commands;

use crate::commands::{built_in_commands, runner_for_built_in_command};
use anyhow::Result;
use clap::{crate_authors, crate_version, Command};

const PROGRAM_NAME: &str = "ion";

fn main() -> Result<()> {
    let mut app = Command::new(PROGRAM_NAME)
        .version(crate_version!())
        .author(crate_authors!())
        .subcommand_required(true);

    for command in built_in_commands() {
        app = app.subcommand(command);
    }

    let args = app.get_matches();
    let (command_name, command_args) = args.subcommand().unwrap();

    if let Some(runner) = runner_for_built_in_command(command_name) {
        // If a runner is registered for the given command name, command_args is guaranteed to
        // be defined.
        runner(command_name, command_args)?;
    } else {
        let message = format!(
            "The requested command ('{}') is not supported and clap did not generate an error message.",
            command_name
        );
        unreachable!("{}", message);
    }
    Ok(())
}
