use crate::commands::CommandRunner;
use anyhow::Result;
use clap::{ArgMatches, Command};

pub mod json;

// Creates a Vec of CLI configurations for all of the available built-in commands
pub fn subcommands() -> Vec<Command> {
    vec![json::app()]
}

// Maps the given command name to the entry point for that command if it exists
pub fn runner_for_to_command(command_name: &str) -> Option<CommandRunner> {
    let runner = match command_name {
        "json" => json::run,
        _ => return None,
    };
    Some(runner)
}

// The functions below are used by the parent `beta` command when `to` is invoked.
pub fn run(_command_name: &str, matches: &ArgMatches) -> Result<()> {
    //     ^-- At this level of dispatch, this command will always be the text `to`.
    // We want to evaluate the name of the subcommand that was invoked --v
    let (command_name, command_args) = matches.subcommand().unwrap();
    if let Some(runner) = runner_for_to_command(command_name) {
        runner(command_name, command_args)?;
    } else {
        let message = format!(
            "The requested `to` command ('{}') is not supported and clap did not generate an error message.",
            command_name
        );
        unreachable!("{}", message);
    }
    Ok(())
}

pub fn app() -> Command {
    Command::new("to")
        .about("'to' is a namespace for commands that convert Ion to another data format.")
        .subcommands(subcommands())
}
