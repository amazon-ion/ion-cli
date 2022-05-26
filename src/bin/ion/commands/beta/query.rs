use clap::{App, Arg, ArgMatches};
use anyhow::{Result, Context};
use thiserror::Error;
use std::{fs, io};
use nom::*;
use std::io::{Read};
use ion_rs::value::reader::{ElementReader, element_reader};
use ion_rs::value::owned::{OwnedElement, OwnedSequence};
use ion_rs::value::{Element, Struct, Sequence, Builder};
use ion_rs::value::writer::{ElementWriter, Format, TextKind};
use nom::combinator::*;
use nom::sequence::*;
use nom::branch::*;
use nom::multi::*;
use nom::character::complete::*;
use nom::bytes::complete::take_until;
use ion_rs::IonType;

type IonIterator = Box<dyn Iterator<Item = IonQueryResult>>;
type IonQueryResult = Result<Option<OwnedElement>, IonQueryError>;

#[derive(Debug, Clone, Error)]
enum IonQueryError {
    #[error(
    "Illegal query operation: {operation}"
    )]
    IllegalOperation { operation: String },
}

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
    let (remaining, path) = path(query).unwrap();

    if !remaining.is_empty() {
        panic!("Could not parse entire expression, remaining: {:?}", remaining);
    }

    println!("path: {:?}", path);

    println!("Reading in Ion data...");
    // read the given input Ion file with element reader
    //TODO: Stream, don't load whole file to memory
    let mut ion_buffer = Vec::new();
    rdr.read_to_end(&mut ion_buffer)?;

    //TODO: Use native reader and writer
    let ion_elements = element_reader()
        .read_all(&ion_buffer)
        .with_context(|| "Could not parse Ion file")?;

    let mut ion_iter: IonIterator = Box::new(ion_elements.into_iter().map(|oe| Ok(Some(oe))).into_iter());

    println!("Output: ");

    ion_iter = filter(path, ion_iter);

    // print query results
    ion_iter.for_each(|oe| print(oe).unwrap());

    Ok(())
}

// .["foo"][]
// .["foo"]
// .[0]
// [1, 2 ,3]
// Path: [Part::Index(Token::Dot), Part::Index(Token::String("foo"))]
fn filter(path: Path<Token>, ion_iter: IonIterator) -> IonIterator {
    path.into_iter().fold(ion_iter, |acc, part| match part {
        Part::Index(index) => {
            match index {
                Token::Number(number) => {
                    Box::new(acc.map(move |oe| select_element(number, oe)).into_iter())
                }
                Token::String(field_name) => {
                    Box::new(acc.map(move |oe| select_field(field_name.to_owned(), oe)).into_iter())
                }
                Token::Dot => {
                    acc.into_iter()
                }
            }
        }
        Part::Range(from, to) => {
            let fr = from.as_ref().map(|f| f.as_int()).transpose();
            let tr = to.as_ref().map(|t| t.as_int()).transpose();
            match (fr, tr) {
                (Ok(None), Ok(None)) => todo!("implement explode"), // explode!
                (Ok(fo), Ok(to)) => { // Any other permutation of indexing where both sides are None or Some(i64)
                            Box::new(acc.map(move |oe| select_range(fo, to, oe)).into_iter())
                },
                _ => panic!("Cannot handle this range {:?}:{:?}", from, to),
            }
        }
    })
}

fn select_element(index: i64, result: IonQueryResult) -> IonQueryResult {
    result.map(|opt| match opt {
        None => Ok(None),
        Some(oe) => {
            if let Some(ion_sequence) = oe.as_sequence() {
                let idx = as_index(&ion_sequence, index);
                return Ok(ion_sequence.get(idx).map(|i| i.to_owned()));
            }
            return Err(IonQueryError::IllegalOperation { operation: format!("Cannot index {} with index {}", oe.ion_type(), index)});
        }
    })?
}

fn select_range(from: Option<i64>, to: Option<i64>, result: IonQueryResult) -> IonQueryResult {
    result.map(|opt| {
        match opt {
            None => Ok(None),
            Some(oe) => {
                if let Some(ion_sequence) = oe.as_sequence() {
                    let from_idx: usize = from.map(|i| as_index(ion_sequence, i)).unwrap_or(0);
                    let to_idx: usize = to.map(|i| as_index(ion_sequence, i)).unwrap_or(ion_sequence.len());

                    let run = if from_idx > to_idx {
                        0
                    } else {
                        to_idx - from_idx
                    };

                    let generator = match oe.ion_type() {
                        IonType::List => OwnedElement::new_list,
                        IonType::SExpression => OwnedElement::new_sexp,
                        _ => unreachable!("We know that owned_element is a sequence")
                    };

                    let partial: Box<dyn Iterator<Item=OwnedElement>> = Box::new(ion_sequence.iter()
                        .skip(from_idx)
                        .take(run)
                        .map(|e| e.to_owned()).into_iter());
                    return Ok(Some(generator(partial)));
                }
                return Err(IonQueryError::IllegalOperation { operation: format!("Cannot index {} with index range [{:?}: {:?}]", oe.ion_type(), from, to)});
            }
        }
        }
    )?
}

fn as_index(ion_sequence: &OwnedSequence, index: i64) -> usize {
    if index < 0 {
        ion_sequence.len() - i64::abs(index) as usize
    } else {
        index as usize
    }
}

//TODO: add a switch to get_all/get OwnedElements
fn select_field(field_name: String, result: IonQueryResult) -> IonQueryResult {
    result.map(|opt| {
        match opt {
            None => Ok(None),
            Some(oe) => {
                if let Some(ion_struct) = oe.as_struct() {
                    return Ok(ion_struct.get(field_name).map(|i| i.to_owned()));
                }
                return Err(IonQueryError::IllegalOperation { operation: format!("Cannot index {} with string {}", oe.ion_type(), field_name) });
            }
        }
    }
    )?
}

fn print(ion_element: IonQueryResult) -> Result<()> {
    match ion_element {
        Err(err) => {
            println!("ERROR: {}", err);
        }
        Ok(None) => {
            println!("null");
        }
        Ok(Some(element)) => {
            //TODO: Handle arbitrarily-sized output objects, or at least larger ones
            let mut buf = vec![0u8; 4096];
            let mut writer = Format::Text(TextKind::Pretty).element_writer_for_slice(&mut buf)?;

            writer.write(&element)?;
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
// fn term(input: &str) -> IResult<&str, JqTerm> {
//     alt ((
//         field_term,
//         field,
//         number,
//         dot
//     ))(input)
// }

fn expression(input: &str) -> IResult<&str, Token> {
    alt((number, string))(input)
}

// fn field_term(input: &str) -> IResult<&str, JqTerm> {
//     map(tuple((field, term)),
//         |(jq_field, jq_term)| JqTerm::TermField(Box::new(jq_term), jq_field.field().unwrap()))(input)
// }

// .["foo"]
// .identifier(.part|[range])*
fn path(input: &str) -> IResult<&str, Path<Token>> {
    map(
        tuple((field_or_dot, path_trail)),
        |(head, tail)| {
            let mut path = Vec::new();
            path.push(head);
            path.extend(tail);
            path
        }
    )(input)
}

fn path_trail(input: &str) -> IResult<&str, Path<Token>> {
    fold_many0(alt((field, path_part)),
               Vec::new,
               |mut acc, part| {
                                acc.push(part);
                                acc
                            }
    )(input)
}

fn path_part(input: &str) -> IResult<&str, Part<Token>> {
    // Token (Token, Token)
    delimited(char('['), alt((range, index)), char(']'))(input)
}

fn index(input: &str) -> IResult<&str, Part<Token>> {
    map(expression, |t| Part::Index(t))(input)
}

fn range(input: &str) -> IResult<&str, Part<Token>> {
    // (Token, Token)
    map(separated_pair(opt(expression), char(':'), opt(expression)),
        |(from, to)| Part::Range(from, to)
    )(input)
}

fn field_or_dot(input: &str) -> IResult<&str, Part<Token>> {
   alt((field, dot))(input)
}

fn field(input: &str) -> IResult<&str, Part<Token>> {
    map(preceded(dot, identifier),
        |name| Part::Index(Token::String(name.to_string())))(input)
}

// "123b" => Ok("b", 123)
fn number(input: &str) -> IResult<&str, Token> {
    map(
        map_res(recognize(tuple((opt(one_of("+-")), digit1))), str::parse::<i64>), // Result<(&str, i64)>
        |i: i64| Token::Number(i))(input)
}

fn dot(input:&str) -> IResult<&str, Part<Token>> {
    map(char('.'), |_| Part::Index(Token::Dot))(input)
}

// "foo bar"
fn string(input: &str) -> IResult<&str, Token> {
    map(delimited(char('"'),
recognize(take_until("\"")),
              char('"')), |s: &str| Token::String(s.to_string()))(input)
}

#[cfg(test)]
mod string_tests {
    use super::*;

    #[test]
    fn test_string() {
        assert_eq!(string(r#""foo bar""#), Ok(("", Token::String("foo bar".to_owned()))));
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
type Path<T> = Vec<Part<T>>;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Part<I> {
    Index(I),
    Range(Option<I>, Option<I>)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(i64),
    String(String),
    Dot
}

impl Token {
    /// If the value is integer, return it, else fail.
    pub fn as_int(&self) -> Result<i64, Token> {
        match self {
            Self::Number(i) => Ok(*i),
            _ => Err(self.clone()),
        }
    }
}