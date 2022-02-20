pub mod inspect;
pub mod schema;

use crate::commands::{CommandConfig, CommandRunner};
use anyhow::Result;
use clap::{App, ArgMatches};

// To add a beta subcommand, add your new command to the `beta_subcommands`
// and `runner_for_beta_subcommands` functions.

// Creates a Vec of CLI configurations for all of the available built-in commands
pub fn beta_subcommands() -> Vec<CommandConfig> {
    vec![inspect::app(), schema::app()]
}

pub fn runner_for_beta_subcommand(command_name: &str) -> Option<CommandRunner> {
    let runner = match command_name {
        "inspect" => inspect::run,
        "schema" => schema::run,
        _ => return None,
    };
    Some(runner)
}

// The functions below are used by the top-level `ion` command when `beta` is invoked.
pub fn run(_command_name: &str, matches: &ArgMatches<'static>) -> Result<()> {
    //     ^-- At this level of dispatch, this command will always be the text `beta`.
    // We want to evaluate the name of the subcommand that was invoked --v
    let (command_name, command_args) = matches.subcommand();
    if let Some(runner) = runner_for_beta_subcommand(command_name) {
        // If a runner is registered for the given command name, command_args is guaranteed to
        // be defined; we can safely unwrap it.
        runner(command_name, command_args.unwrap())?;
    } else {
        let message = format!(
            "The requested beta command ('{}') is not supported and clap did not generate an error message.",
            command_name
        );
        unreachable!("{}", message);
    }
    Ok(())
}

pub fn app() -> CommandConfig {
    App::new("beta")
        .about(
            "The 'beta' command is a namespace for commands whose interfaces are not yet stable.",
        )
        .subcommands(beta_subcommands())
}
