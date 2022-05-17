pub mod load;
pub mod validate;

use anyhow::Result;
use clap::{App, ArgMatches};
use crate::commands::{CommandRunner, CommandConfig};

// To add a schema subcommand, add your new command to the `schema_subcommands`
// and `runner_for_schema_subcommands` functions.

// Creates a Vec of CLI configurations for all of the available built-in subcommands for schema
pub fn schema_subcommands() -> Vec<CommandConfig> {
    vec![
        load::app(),
        validate::app()
    ]
}

pub fn runner_for_schema_subcommand(command_name: &str) -> Option<CommandRunner> {
    let runner = match command_name {
        "load" => load::run,
        "validate" => validate::run,
        _ => return None
    };
    Some(runner)
}

// The functions below are used by the `beta` subcommand when `schema` is invoked.
pub fn run(_command_name: &str, matches: &ArgMatches<'static>) -> Result<()> {
    // We want to evaluate the name of the subcommand that was invoked
    let (command_name, command_args) = matches.subcommand();
    if let Some(runner) = runner_for_schema_subcommand(command_name) {
        // If a runner is registered for the given command name, command_args is guaranteed to
        // be defined; we can safely unwrap it.
        runner(command_name, command_args.unwrap())?;
    } else {
        let message = format!(
            "The requested schema command ('{}') is not supported and clap did not generate an error message.",
            command_name
        );
        unreachable!("{}", message);
    }
    Ok(())
}

pub fn app() -> CommandConfig {
    App::new("schema")
        .about(
            "The 'schema' command is a namespace for commands that are related to schema sandbox",
        )
        .subcommands(schema_subcommands())
}
