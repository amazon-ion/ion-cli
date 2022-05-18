use clap::{App, Arg, ArgMatches, Values};
use anyhow::{Result};
use ion_rs::{SystemReader, IonDataSource, RawReader};
use ion_rs::text::raw_text_reader::RawTextReader;
use std::{fs, io};
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
                .multiple(false)//TODO: handle multiple files, explicitly specified stdin
                .help("Specify the input Ion file for querying."),
        )
}

// This function is invoked by the `jq` command's parent, `beta`.
pub fn run(_command_name: &str, matches: &ArgMatches<'static>) -> Result<()> {
    let is_jq = matches.occurrences_of("jq") == 1;
    println!("Is jq syntax expected: {}", is_jq);

    let query = matches.value_of("QUERY").unwrap();
    println!("Query: {}", query);

    let mut rdr: Box<dyn io::Read> = match matches.value_of("INPUT") {
        None => Box::new(io::stdin()), // no files provided, read from stdin
        Some(input_file) => Box::new(fs::File::open(input_file).unwrap()),
    };

    // send input to stdout
    io::copy(&mut rdr, &mut io::stdout()).unwrap();
    // let reader = RawTextReader::new(ion_content);

    todo!()
}
