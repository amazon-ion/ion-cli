use anyhow::Result;
use clap::{ArgMatches, Command};
pub mod json;

pub type CommandRunner = fn(&str, &ArgMatches) -> Result<()>;

// Creates a Vec of CLI configurations for all of the available built-in commands
pub fn built_in_commands() -> Vec<Command> {
    vec![json::app()]
}

// Maps the given command name to the entry point for that command if it exists
pub fn runner_for_built_in_command(command_name: &str) -> Option<CommandRunner> {
    let runner = match command_name {
        "json" => json::run,
        _ => return None,
    };
    Some(runner)
}
