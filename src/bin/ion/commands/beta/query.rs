use clap::{App, Arg, ArgMatches};
use anyhow::{Result, Context};
use std::{fs, io};
use nom::*;
use std::io::{Read};
use ion_rs::value::reader::{ElementReader, element_reader};
use ion_rs::value::owned::{OwnedElement, OwnedStruct};
use ion_rs::value::{Element, Struct};
use ion_rs::value::writer::{ElementWriter, Format, TextKind, SliceElementWriter};
use nom::combinator::*;
use nom::sequence::*;
use nom::branch::*;
use nom::multi::*;
use nom::character::complete::*;

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
        //TODO: Implement -r
        .arg(
            Arg::with_name("raw")
                .long("raw")
                .short("r")
                .help("Print bare strings (useful for consuming with other tools)")
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
    let (_remaining, jqExpression) = expression(query).unwrap();

    println!("jqExpression: {:?}", jqExpression);

    println!("Reading in Ion data...");
    // read the given input Ion file with element reader
    //TODO: Stream, don't load whole file to memory
    let mut ion_buffer = Vec::new();
    rdr.read_to_end(&mut ion_buffer)?;

    //TODO: Use native reader and writer
    let mut ion_iter = element_reader()
        .iterate_over(&ion_buffer)
        .with_context(|| "Could not parse Ion file")?;

    let mut foobar = Box::new(ion_iter.map(|oe| &oe.unwrap()));

    println!("Output: ");


    match jqExpression {
        JqExpression::Dot => {
            // identity, do nothing
        }
        JqExpression::Field(name) => {
            foobar = Box::new(foobar.flat_map(|oe| select_field(name, oe)));
        }
    }

    ion_iter.map(|oe| print(oe));

    Ok(())
}

//TODO: this should be a function that we can use with flat_map on an iter
fn select_field<'a>(field_name: String, owned_element: &'a OwnedElement) -> Box<dyn Iterator<Item=&OwnedElement> + 'a> {
    if let Some(ion_struct) = owned_element.as_struct() {
        return ion_struct.get_all(field_name);
    };
    Box::new(std::iter::empty())
}

fn print(ion_element: &OwnedElement) -> Result<()> {
    //TODO: Handle arbitrarily-sized output objects, or at least larger ones
    let mut buf = vec![0u8; 4096];
    let mut writer = Format::Text(TextKind::Compact).element_writer_for_slice(&mut buf)?;
    writer.write(ion_element)?;
    let result = writer.finish()?;

    println!("{}", String::from_utf8_lossy(result).to_string());
    Ok(())
}


// Recognize something like `.foo`
// Yields Ok("", "foo")
// .foo.bar
// .foo | .bar
fn field(input: &str) -> IResult<&str, JqExpression> {
    map(preceded(dot, identifier),
        |name| JqExpression::Field(name.to_string()))(input)
}

fn expression(input: &str) -> IResult<&str, JqExpression> {
    alt((
        dot,
        field
    ))(input)
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

fn dot(input:&str) -> IResult<&str, JqExpression> {
    map(char('.'), |_| JqExpression::Dot)(input)
}


#[derive(Debug, Clone, PartialEq)]
pub(crate) enum JqExpression {
    Dot,
    Field(String),
}