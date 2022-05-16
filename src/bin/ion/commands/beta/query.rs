use clap::{App, Arg, ArgMatches};
use anyhow::{Result};
use ion_rs::{SystemReader, IonDataSource, RawReader};
use ion_rs::text::raw_text_reader::RawTextReader;
use std::fs;
use std::path::Path;
use ion_rs::result::IonResult;
use ion_rs::raw_reader::RawStreamItem::{VersionMarker, Value};

const ABOUT: &str =
    "A command-line processor for Ion.";

// Creates a `clap` (Command Line Arguments Parser) configuration for the `jq` command.
// This function is invoked by the `jq` command's parent, `beta`, so it can describe its
// child commands.
pub fn app() -> App<'static, 'static> {
    App::new("query")
        .about(ABOUT)
        .arg(
            Arg::with_name("jq")
                .long("jq")
                .short("j")
                .help("Uses jq query syntax."),
        )
        .arg(
            Arg::with_name("QUERY")
                .index(1)
                .help("Specify the query to run."),
        )
        .arg(
            Arg::with_name("INPUT")
                .index(2)
                .required(false)
                .multiple(true)
                .help("Specify the input Ion file for querying."),
        )
}

// This function is invoked by the `jq` command's parent, `beta`.
pub fn run(_command_name: &str, matches: &ArgMatches<'static>) -> Result<()> {
    let is_jq = matches.occurrences_of("jq") == 1;
    println!("Is jq syntax expected: {}", is_jq);

    let query = matches.value_of("QUERY").unwrap();
    println!("Query: {}", query);

    let input: Vec<&str> = matches.values_of("INPUT").unwrap().collect();
    println!("Input: {:?}", input);

    let ion_content = fs::read(Path::new(input[0]))?;
    let reader = RawTextReader::new(ion_content);

    todo!()
}
