use clap::{App, Arg, ArgMatches};
use anyhow::{Result, Context};
use std::{fs, io};
use nom::*;
use std::io::{Read};
use ion_rs::value::reader::{ElementReader, element_reader};
use ion_rs::value::owned::{OwnedElement};
use ion_rs::value::{Element, Struct};
use ion_rs::value::writer::{ElementWriter, Format, TextKind};
use nom::combinator::*;
use nom::sequence::*;
use nom::branch::*;
use nom::multi::*;
use nom::character::complete::*;
use std::rc::Rc;
use futures::stream::{self, StreamExt};

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
    let (_remaining, jq_expression) = expression(query).unwrap();

    println!("jq_expression: {:?}", jq_expression);

    println!("Reading in Ion data...");
    // read the given input Ion file with element reader
    //TODO: Stream, don't load whole file to memory
    let mut ion_buffer = Vec::new();
    rdr.read_to_end(&mut ion_buffer)?;

    //TODO: Use native reader and writer
    let ion_elements = element_reader()
        .read_all(&ion_buffer)
        .with_context(|| "Could not parse Ion file")?;

    let mut stream = stream::iter(ion_elements.iter());
    let mut ion_iter: Box<dyn Iterator<Item = Option<&OwnedElement>>> = Box::new(ion_elements.iter().map(|oe| Some(oe)).into_iter());

    println!("Output: ");

    match jq_expression {
        JqTerm::Dot => {
            // identity, do nothing
        }
        JqTerm::Field(ref name) => {
            // select field for the given field name
            ion_iter = Box::new(ion_iter.map(|oe| select_field(&name.value, oe)).into_iter());
        }
        _ => {}
    }

    // print query results
    ion_iter.for_each(|oe| print(oe).unwrap());

    Ok(())
}

//TODO: add a switch to get_all/get OwnedElements
fn select_field<'a>(field_name: &'a String, owned_element: Option<&'a OwnedElement>) -> Option<&'a OwnedElement> {
    if let Some(ion_struct) = owned_element.unwrap().as_struct() {
        return ion_struct.get(field_name);
    }
    panic!("Cannot index {} with string {}", owned_element.unwrap().ion_type(), field_name)
}

fn print(ion_element: Option<&OwnedElement>) -> Result<()> {
    match ion_element {
        None => {
            println!("null");
        }
        Some(element) => {
            //TODO: Handle arbitrarily-sized output objects, or at least larger ones
            let mut buf = vec![0u8; 4096];
            let mut writer = Format::Text(TextKind::Pretty).element_writer_for_slice(&mut buf)?;

            writer.write(element)?;
            let result = writer.finish()?;
            println!("{}", String::from_utf8_lossy(result).to_string());
        }
    }
    Ok(())
}

// Recognize something like `.foo`
// Yields Ok("", "foo")
// .foo.bar
// .foo | .bar
fn field(input: &str) -> IResult<&str, JqTerm> {
    map(preceded(dot, identifier),
        |name| JqTerm::Field(FieldToken { value: name.to_string() }))(input)
}

fn expression(input: &str) -> IResult<&str, JqTerm> {
    alt((
        field,
        dot
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

fn dot(input:&str) -> IResult<&str, JqTerm> {
    map(char('.'), |_| JqTerm::Dot)(input)
}


#[derive(Debug, Clone, PartialEq)]
pub(crate) enum JqTerm {
    Dot,
    Field(FieldToken),
    TermField(Rc<JqTerm>, FieldToken)
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldToken {
    value: String
}

// impl FieldToken {
//     fn new(value: String) -> Self {
//         Self {
//            value
//         }
//     }
//
//     fn value(&self) -> &String {
//         &self.value
//     }
// }