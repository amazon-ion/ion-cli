use clap::{App, ArgMatches};

pub mod beta;
pub mod dump;

// Creates a Vec of CLI configurations for all of the available built-in commands
pub fn built_in_commands() -> Vec<App<'static, 'static>> {
    vec![dump::app(), beta::app()]
}

// Maps the given command name to the entry point for that command if it exists
pub fn runner_for_built_in_command(command_name: &str) -> Option<fn(&str, &ArgMatches<'static>)> {
    let runner = match command_name {
        "dump" => dump::run,
        "beta" => beta::run,
        _ => return None,
    };
    Some(runner)
}
