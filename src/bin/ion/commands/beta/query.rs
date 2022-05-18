use clap::{App, Arg, ArgMatches};
use anyhow::{Result, Context};
use std::{fs, io};
use nom::IResult;
use nom::combinator::{recognize, not};
use nom::sequence::{pair, terminated};
use nom::branch::alt;
use nom::multi::many0_count;
use nom::character::complete::{one_of, satisfy};
use std::io::Read;
use ion_rs::value::reader::{element_reader, ElementReader};
use ion_rs::value::owned::{OwnedElement};
use ion_rs::value::{Element, Struct};
use ion_rs::value::writer::{ElementWriter, Format, TextKind};

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

    // get the identifier from given query
    let identifier = identifier(query).unwrap();
    // println!("{}", identifier.1);

    // send input to stdout
    // io::copy(&mut rdr, &mut io::stdout()).unwrap();

    // read the given input Ion file with element reader
    let mut ion_buffer = Vec::new();
    rdr.read_to_end(&mut ion_buffer)?;

    let owned_elements: Vec<OwnedElement> = element_reader()
        .read_all(&ion_buffer)
        .with_context(|| "Could not parse Ion file")?;

    // filter owned elements that match with given query
    let mut result: Vec<&OwnedElement> = vec![];
    for owned_element in owned_elements.iter() {
        match owned_element.as_struct() {
            None => {
            }
            Some(ion_struct) => {
                result.extend(ion_struct.get_all(identifier.1));
            }
        }
    }

    // write out the filtered result
    let mut element_writer = Format::Text(TextKind::Pretty).element_writer_for_slice(&mut ion_buffer)?;
    result.iter().for_each(|v| element_writer.write(*v).unwrap());
    let slice = element_writer.finish()?;
    let output = String::from_utf8_lossy(slice);

    println!("Output:\n{}", output);
    Ok(())
}

fn identifier(input: &str) -> IResult<&str, &str> {
    let (remaining, identifier_text) = recognize(terminated(
        pair(identifier_initial_character, identifier_trailing_characters),
        not(identifier_trailing_character),
    ))(input)?;

    Ok((remaining, identifier_text))
}

/// Matches any character that can appear at the start of an identifier.
fn identifier_initial_character(input: &str) -> IResult<&str, char> {
    alt((one_of("$_"), satisfy(|c| c.is_ascii_alphabetic())))(input)
}

/// Matches any character that is legal in an identifier, though not necessarily at the beginning.
fn identifier_trailing_character(input: &str) -> IResult<&str, char> {
    alt((one_of("$_"), satisfy(|c| c.is_ascii_alphanumeric())))(input)
}

/// Matches characters that are legal in an identifier, though not necessarily at the beginning.
fn identifier_trailing_characters(input: &str) -> IResult<&str, &str> {
    recognize(many0_count(identifier_trailing_character))(input)
}