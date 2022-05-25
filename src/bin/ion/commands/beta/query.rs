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
    let (_remaining, jq_term) = term(query).unwrap();

    println!("jq_expression: {:?}", jq_term);

    println!("Reading in Ion data...");
    // read the given input Ion file with element reader
    //TODO: Stream, don't load whole file to memory
    let mut ion_buffer = Vec::new();
    rdr.read_to_end(&mut ion_buffer)?;

    //TODO: Use native reader and writer
    let ion_elements = element_reader()
        .read_all(&ion_buffer)
        .with_context(|| "Could not parse Ion file")?;

    let mut ion_iter: Box<dyn Iterator<Item = Option<&OwnedElement>>> = Box::new(ion_elements.iter().map(|oe| Some(oe)).into_iter());

    println!("Output: ");

    ion_iter = select_term(Box::new(jq_term), ion_iter);

    // print query results
    ion_iter.for_each(|oe| print(oe).unwrap());

    Ok(())
}

fn select_term<'a>(jq_term: Box<JqTerm>, ion_iter: Box<dyn Iterator<Item=Option<&'a OwnedElement>> + 'a>) -> Box<dyn Iterator<Item=Option<&'a OwnedElement>> + 'a> {
    // TODO: remove usage of move here
    match *jq_term {
        JqTerm::Dot => {
            // identity, do nothing
            ion_iter
        }
        JqTerm::Field(name) => {
            // select field for the given field name
            Box::new(ion_iter.map(move |oe| select_field(name.value.to_owned(), oe)).into_iter())
        }
        JqTerm::TermField(jq_recursive_term, jq_field) => {
            // Recursive call to select term
            select_term(jq_recursive_term, select_term(Box::new(JqTerm::Field(jq_field.to_owned())), ion_iter))
        }
        // TODO: Understand ownership and lifetimes
        // JqTerm::Literal(literal) => {
        //     Box::new(ion_iter.map(move |oe| Some(&OwnedElement::new_i64(literal))).into_iter())
        // }
        _ => {
            panic!("Don't know how to handle jq term")
        }
    }
}

//TODO: add a switch to get_all/get OwnedElements
fn select_field(field_name: String, owned_element: Option<&OwnedElement>) -> Option<&OwnedElement> {
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
// .foo
fn term(input: &str) -> IResult<&str, JqTerm> {
    alt ((
        field_term,
        field,
        number,
        dot
    ))(input)
}

fn expression<T>(input: &str) -> IResult<&str, T> {
    alt((number, string))(input)
}

fn field_term(input: &str) -> IResult<&str, JqTerm> {
    map(tuple((field, term)),
        |(jq_field, jq_term)| JqTerm::TermField(Box::new(jq_term), jq_field.field().unwrap()))(input)
}

fn path<T>(input: &str) -> IResult<&str, Path<T>> {
    map(
        tuple((field, path_trail)),
        |(head, tail)| {
            let mut path = Vec::new();
            //TODO: expected `&mut Vec<<unknown>, Global>`, found `Path<<unknown>>`
            path.append(tail);
            path
        }
    )
}

// .identifier(.part|[range])*
fn path_trail<T>(input: &str) -> IResult<&str, Path<T>> {
    fold_many0(alt((field, range)),
                        Vec::new,
                            |mut acc, part| {
                                acc.push(part);
                                acc
                            }
    )(input)
}
// a[0]
// a[1+2]
// a["foo"]
// a[1:]
// a[1:10]
// ["foo"]["bar"]?[baz]
// TODO: Handle non-index ranges, e.g. [a:b] instead of [a]
fn range<T>(input: &str) -> IResult<&str, Part<T>> {
    map(delimited(char('['), expression, char(']')),
        |name| Part::Index(name.to_string()))(input)
}

fn field<T>(input: &str) -> IResult<&str, Part<T>> {
    map(preceded(dot, identifier),
        |name| Part::Index(name.to_string()))(input)
}

// "foo bar"
fn string(input: &str) -> IResult<&str, &str> {
    recognize(delimited(char('"'),
                        many0(not(char('"'))),
                        char('"')))(input)
}

#[cfg(test)]
mod string_tests {
    use super::*;

    #[test]
    fn test_string() {
        assert_eq!(string(r#""foo bar""#), Ok(("", "foo bar")));
    }
}


// foo
fn identifier(input: &str) -> IResult<&str, &str> {
    let (remaining, identifier_text) = recognize(terminated(
        pair(identifier_initial_character, identifier_trailing_characters),
        not(identifier_trailing_character),
    ))(input)?;

    Ok((remaining, identifier_text))
}

#[cfg(test)]
mod identifier_tests {

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

// "123b" => 123
//TODO: Fixme - we're discarding/truncating
fn number(input: &str) -> IResult<&str, (&str, JqTerm)> {
    map(
        map_res(digit1, str::parse::<i64>), // Result<(&str, i64)>
        |(remainder, i)| (remainder, JqTerm::Literal(i))
    )(input)
}

// map_res(digit1, str::parse)(input)

fn dot(input:&str) -> IResult<&str, JqTerm> {
    map(char('.'), |_| JqTerm::Dot)(input)
}

// Term:
//         '.'   |
//         ".."  |
//         "break" '$' IDENT    |
//         Term FIELD '?'       |
//         FIELD '?'            |
//         Term '.' String '?'  |
//         '.' String '?'       |
//         Term FIELD           |
//         FIELD                |
//         Term '.' String      |
//         '.' String           |
//         Term '[' Exp ']' '?'          |
//         Term '[' Exp ']'              |
//         Term '[' ']' '?'              |
//         Term '[' ']'                  |
//         Term '[' Exp ':' Exp ']' '?'  |
//         Term '[' Exp ':' ']' '?'      |
//         Term '[' ':' Exp ']' '?'      |
//         Term '[' Exp ':' Exp ']'      |
//         Term '[' Exp ':' ']'          |
//         Term '[' ':' Exp ']'          |
//         LITERAL  |
//         String   |
//         FORMAT   |
//         '(' Exp ')'     |
//         '[' Exp ']'     |
//         '[' ']'         |
//         '{' MkDict '}'  |
//         '$' "__loc__"   |
//         '$' IDENT       |
//         IDENT           |
//         IDENT '(' Args ')'
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum JqTerm {
    Literal(i64),
    Dot,
    Field(FieldToken),
    //TODO: change to FieldTerm
    TermField(Box<JqTerm>, FieldToken)
}

impl JqTerm {
    pub fn field(&self) -> Option<FieldToken> {
        match self {
            JqTerm::Field(field) => {Some(field.to_owned())}
            _ => None
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct FieldToken {
    value: String
}

type Path<T> = Vec<Part<T>>;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Part<I> {
    Index(I),
    Range(Option<I>, Option<I>)
}