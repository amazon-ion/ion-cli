use crate::commands::dump;
use anyhow::Result;
use clap::{Arg, ArgAction, ArgMatches, Command};

const ABOUT: &str = "Converts data from JSON to Ion.";

// Creates a `clap` (Command Line Arguments Parser) configuration for the `from` command.
// This function is invoked by the parent command,`from`, so it can describe its child commands.
pub fn app() -> Command {
    Command::new("json")
        .about(ABOUT)
        .arg(
            Arg::new("output")
                .long("output")
                .short('o')
                .help("Output file [default: STDOUT]"),
        )
        .arg(
            Arg::new("format")
                .long("format")
                .short('f')
                .default_value("pretty")
                .value_parser(["binary", "text", "pretty", "lines"])
                .help("Output format"),
        )
        .arg(
            // Any number of input files can be specified by repeating the "-i" or "--input" flags.
            // Unlabeled positional arguments will also be considered input file names.
            Arg::new("input")
                .index(1)
                .trailing_var_arg(true)
                .action(ArgAction::Append)
                .help("Input file"),
        )
}

// This function is invoked by the `from` command's parent, `beta`.
pub fn run(_command_name: &str, matches: &ArgMatches) -> Result<()> {
    // Because JSON data is valid Ion, the `dump` command may be reused for converting JSON.
    // TODO ideally, this would perform some smarter "up-conversion".
    dump::run("json", matches)
}
