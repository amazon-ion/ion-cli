use crate::ansi_codes::*;
use crate::commands::eq::InputType::{Auto, File, Hex, Ion};
use crate::commands::IonCliCommand;
use crate::hex_reader::HexReader;
use crate::input::CommandInput;
use anyhow::Result;
use clap::error::ErrorKind;
use clap::{Arg, ArgAction, ArgMatches, Command, Error};
use ion_rs::*;
use std::fs;
use std::io;
use std::io::{Cursor, Read};
use std::str::from_utf8;
use std::sync::LazyLock;

pub struct EqCommand;

static HELP_EPILOGUE: LazyLock<String> = LazyLock::new(|| {
    format!(
        // '\' at the end of the line indicates that CLAP will handle the line wrapping.
        "\
Exactly two Ion streams must be provided, and may be provided as a file name, a string of \
hexadecimal pairs, or a string of Ion text or binary. If only one stream is provided as an \
argument, then stdin is implied to be the second Ion stream.

Input mode can be specified up to two times. If input modes are provided, the first one applies \
to the first input and the second one (if present) applies to the second input. No more than two \
input mode flags may be provided. The input mode is not required to be specified. The eq command \
will attempt to infer the type of each input using the following heuristic:
    1. If the input is not valid UTF-8, it is assumed to be Ion binary
    2. If the input is a path to an existing file, the input is assumed to be a file name
    3. Then, eq attempts to parse the input as Ion
    4. Then, eq attempts to parse the input as a stream of hex digit pairs

{BOLD}{UNDERLINE}Example Usage:{NO_STYLE}

~$ ion eq -B '0' 'E0 01 01 EA 60'
true
"
    )
});

const FILE_INPUT_ARG_ID: &str = "file-input";
const ION_INPUT_ARG_ID: &str = "ion-input";
const HEX_INPUT_ARG_ID: &str = "hex-input";

impl IonCliCommand for EqCommand {
    fn name(&self) -> &'static str {
        "eq"
    }

    fn about(&self) -> &'static str {
        "Compares two Ion streams"
    }

    fn is_stable(&self) -> bool {
        false
    }

    fn is_porcelain(&self) -> bool {
        false
    }

    fn configure_args(&self, command: Command) -> Command {
        command.after_help(HELP_EPILOGUE.as_str()).args([
            Arg::new(FILE_INPUT_ARG_ID)
                .help("Indicates that an input is the name of a file containing an Ion stream")
                .short('f')
                .help_heading("Input mode")
                .action(ArgAction::Count),
            Arg::new(ION_INPUT_ARG_ID)
                .help("Indicates that an input is an Ion stream")
                .short('i')
                .help_heading("Input mode")
                .action(ArgAction::Count),
            Arg::new(HEX_INPUT_ARG_ID)
                .help("Interprets an input as a string of hex digit pairs")
                .short('x')
                .help_heading("Input mode")
                .action(ArgAction::Count),
            Arg::new("bool-output")
                .short('B')
                .visible_short_alias('=')
                .help("Prints true or false to stdout.")
                .required(false)
                .action(ArgAction::SetTrue),
            Arg::new("exit-code-output")
                .short('E')
                .visible_short_alias('!')
                .help("Exits with exitcode=1 if they are not equivalent")
                .required(false)
                .action(ArgAction::SetTrue),
            Arg::new("input-a")
                .action(ArgAction::Set)
                .required(true)
                .help("First input to compare"),
            Arg::new("input-b")
                .action(ArgAction::Set)
                .required(false)
                .help("Second input to compare; defaults to stdin if no value provided"),
        ])
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        let no_auto_decompression = args.get_flag("no-auto-decompress");
        let (in_mode_a, in_mode_b) = read_input_modes(args)?;

        let input_a = args.get_one::<String>("input-a");
        let input_b = args.get_one::<String>("input-b");

        // TODO: See if we can lazily parse, load, and compare the data instead of using the
        //       [Element] API so that we don't have to hold all of the data into memory at once.

        let data_a = match in_mode_a {
            File => read_file_input(input_a.unwrap(), no_auto_decompression),
            Ion => read_ion_input(input_a.unwrap()),
            Hex => read_hex_input(input_a.unwrap()),
            Auto => read_input_auto_mode(input_a.unwrap(), no_auto_decompression),
        }?;

        let data_b = if let Some(s) = input_b {
            match in_mode_b {
                File => read_file_input(s, no_auto_decompression),
                Ion => read_ion_input(s),
                Hex => read_hex_input(s),
                Auto => read_input_auto_mode(s.as_bytes(), no_auto_decompression),
            }
        } else {
            match in_mode_b {
                File => {
                    let file_name =
                        String::from_utf8(read_stdin_input_bytes(no_auto_decompression)?)?;
                    read_file_input(&file_name, no_auto_decompression)
                }
                Ion => read_ion_input(read_stdin_input_bytes(no_auto_decompression)?),
                Hex => {
                    let hex_string =
                        String::from_utf8(read_stdin_input_bytes(no_auto_decompression)?)?;
                    read_hex_input(&hex_string)
                }
                Auto => {
                    let input_bytes = read_stdin_input_bytes(no_auto_decompression)?;
                    read_input_auto_mode(input_bytes, no_auto_decompression)
                }
            }
        }?;

        let is_equivalent = IonData::eq(&data_a, &data_b);

        if args.get_flag("bool-output") {
            println!("{}", is_equivalent);
        }
        if !is_equivalent && args.get_flag("exit-code-output") {
            std::process::exit(1);
        }
        Ok(())
    }
}

/// Reads the two [InputType]s for the Ion streams from the [ArgMatches].
fn read_input_modes(args: &ArgMatches) -> Result<(InputType, InputType)> {
    let f_index = args.index_of(FILE_INPUT_ARG_ID);
    let i_index = args.index_of(ION_INPUT_ARG_ID);
    let x_index = args.index_of(HEX_INPUT_ARG_ID);
    let f_count = args.get_count(FILE_INPUT_ARG_ID);
    let i_count = args.get_count(ION_INPUT_ARG_ID);
    let x_count = args.get_count(HEX_INPUT_ARG_ID);

    match (f_count, i_count, x_count) {
        (0, 0, 0) => Ok((Auto, Auto)),
        (1, 0, 0) => Ok((File, Auto)),
        (0, 1, 0) => Ok((Ion, Auto)),
        (0, 0, 1) => Ok((Hex, Auto)),
        (2, 0, 0) => Ok((File, File)),
        (0, 2, 0) => Ok((Ion, Ion)),
        (0, 0, 2) => Ok((Hex, Hex)),
        (1, 1, 0) if f_index.unwrap() < i_index.unwrap() => Ok((File, Ion)),
        (1, 1, 0) => Ok((Ion, File)),
        (1, 0, 1) if f_index.unwrap() < x_index.unwrap() => Ok((File, Hex)),
        (1, 0, 1) => Ok((Hex, File)),
        (0, 1, 1) if i_index.unwrap() < x_index.unwrap() => Ok((Ion, Hex)),
        (0, 1, 1) => Ok((Hex, Ion)),
        _ => Err(Error::raw(
            ErrorKind::ArgumentConflict,
            "A maximum of two input modes can be specified.",
        ))?,
    }
}

/// Indicates the type of input for an argument.
enum InputType {
    Auto,
    File,
    Hex,
    Ion,
}

/// Attempts to infer the type of input and read it appropriately.
///
/// Heuristic:
/// 1. If the input is not valid UTF-8, then it is assumed to be Ion binary
/// 2. If the input happens to be the name of an existing file, we assume that it points to a file
///    that contains an Ion stream.
/// 3. Attempt to parse the input as an Ion stream
/// 4. Attempt to parse the input as hex digit pairs, convert to bytes, and then parse the bytes as Ion.
///
/// Mistaken input types are rare for non-trivial inputs.
/// * Ion binary must start with the IVM bytes, which includes bytes that are not valid Ascii or
///   UTF-8. There is no possibility of conflict with any of the other input types.
/// * Any file name that contains `\`, `/`, or `.` is not a valid top-level Ion value. Only trivial
///   Ion text such as `null.string` or `README` could possibly be mistake for a file, and additionally,
///   a file by that name must exist for it to be interpreted that way. More typical files, such as
///   `foo.ion` cannot be mistaken for any other input type.
/// * Any Ion binary encoded as hex digits starts with the characters `E0`, which is not valid Ion
///   text or binary.
/// * Any _non-trivial_ Ion text encoded as hex digits is likely to contain one of `2C` (`,`),
///   `3A` (`:`), `5B` (`[`), or `7B` (`{`), all of which are not valid Ion text values. In addition,
///   the hex-digits for all of `.JKLMNOZ_jklmnoz` are also invalid as top-level Ion text values.
///
/// TODO: Do we want to support detecting hex digit pairs in files?
fn read_input_auto_mode<A: AsRef<[u8]>>(input: A, no_auto_decompression: bool) -> Result<Sequence> {
    if let Ok(input_string) = from_utf8(input.as_ref()) {
        // If fs::exists returns Err, that signals that the existence check is inconclusive. That
        // could be caused by insufficient permissions, for example, but we'll try other methods anyway.
        let is_existing_file_path = fs::exists(input_string);
        // TODO: Check to make sure it's a file, not a directory.
        if is_existing_file_path.is_ok() && is_existing_file_path.unwrap() {
            read_file_input(input_string, no_auto_decompression)
        } else {
            let input_as_ion = read_ion_input(input_string);
            if input_as_ion.is_err() {
                read_hex_input(input_string).or(input_as_ion)
            } else {
                input_as_ion
            }
        }
    } else {
        read_ion_input(input)
    }
}

fn read_file_input(file_name: &str, no_auto_decompression: bool) -> Result<Sequence> {
    let command_input = if no_auto_decompression {
        CommandInput::without_decompression(file_name, fs::File::open(file_name)?)?
    } else {
        CommandInput::decompress(file_name, fs::File::open(file_name)?)?
    };
    let data = command_input
        .into_source()
        .bytes()
        .collect::<Result<Vec<_>, io::Error>>()?;
    Ok(Element::read_all(data)?)
}

fn read_hex_input(hex_string: &str) -> Result<Sequence> {
    let hex_reader = HexReader::from(Cursor::new(hex_string));
    let bytes = hex_reader.bytes().collect::<Result<Vec<u8>, io::Error>>()?;
    Ok(Element::read_all(bytes)?)
}

fn read_ion_input<A: AsRef<[u8]>>(text: A) -> Result<Sequence> {
    Ok(Element::read_all(text)?)
}

fn read_stdin_input_bytes(no_auto_decompression: bool) -> Result<Vec<u8>> {
    const STDIN_NAME: &str = "-";
    let stdin = io::stdin().lock();
    let command_input = if !no_auto_decompression {
        CommandInput::decompress(STDIN_NAME, stdin)
    } else {
        CommandInput::without_decompression(STDIN_NAME, stdin)
    }?;
    let data = command_input
        .into_source()
        .bytes()
        .collect::<Result<Vec<_>, io::Error>>()?;
    Ok(data)
}
