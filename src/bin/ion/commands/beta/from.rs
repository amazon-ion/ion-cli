use anyhow::Result;
use clap::{Arg, ArgAction, ArgMatches, Command};
use crate::commands::dump;

const ABOUT: &str =
    "Converts data from a particular format into Ion.";

// Creates a `clap` (Command Line Arguments Parser) configuration for the `inspect` command.
// This function is invoked by the `inspect` command's parent, `beta`, so it can describe its
// child commands.
pub fn app() -> Command {
    Command::new("from")
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
            Arg::new("source_format")
                .index(1)
                .help("Format of the data to convert."),
        )
        .arg(
            // Any number of input files can be specified by repeating the "-i" or "--input" flags.
            // Unlabeled positional arguments will also be considered input file names.
            Arg::new("input")
                .long("input")
                .short('i')
                .index(2)
                .trailing_var_arg(true)
                .action(ArgAction::Append)
                .help("Input file"),
        )
}

// This function is invoked by the `from` command's parent, `beta`.
pub fn run(_command_name: &str, matches: &ArgMatches) -> Result<()> {
    match matches.get_one::<String>("source_format").expect("Format not found").as_str() {
        "json" => {
            // Because JSON data is valid Ion, the `dump` command may be reused for converting JSON.
            dump::run("from", matches)
        }
        _ => { unimplemented!("Unsupported format.") }
    }.expect("Cannot convert.");

    Ok(())
}
