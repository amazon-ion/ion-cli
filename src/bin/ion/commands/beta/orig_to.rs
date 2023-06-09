use anyhow::{Context, Result};
use clap::{Arg, ArgAction, ArgMatches, Command};
use ion_rs::element::reader::ElementReader;
use ion_rs::element::Element;
use ion_rs::ReaderBuilder;
use memmap::MmapOptions;
use serde_json::{Map, Number, Value as JsonValue};
use std::fs::File;
use std::io;
use std::io::{stdout, BufWriter, Write};
use std::str::FromStr;

const ABOUT: &str = "Converts data from Ion into a requested format. Currently supports json.";

// Creates a `clap` (Command Line Arguments Parser) configuration for the `to` command.
// This function is invoked by the `to` command's parent, `beta`, so it can describe its
// child commands.
pub fn app() -> Command {
    Command::new("to")
        .about(ABOUT)
        .arg(
            Arg::new("output")
                .long("output")
                .short('o')
                .help("Output file [default: STDOUT]"),
        )
        .arg(Arg::new("format").index(1).help("Output format"))
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
    // NOTE: it may be necessary to add format-specific options. For example, a "pretty" option
    // would make sense for JSON, but not binary formats like CBOR.
}

pub fn run(_command_name: &str, matches: &ArgMatches) -> Result<()> {
    // NOTE: the following logic is copied from inspect.run(), and should be refactored for reuse.

    let format = matches
        .get_one::<String>("format")
        .with_context(|| "No `format` was specified.")?
        .as_str();

    // -o filename
    let mut output: Box<dyn Write> = if let Some(output_file) = matches.get_one::<String>("output")
    {
        let file = File::create(output_file).with_context(|| {
            format!(
                "could not open file output file '{}' for writing",
                output_file
            )
        })?;
        Box::new(file)
    } else {
        Box::new(stdout().lock())
    };

    if let Some(input_file_iter) = matches.get_many::<String>("input") {
        for input_file in input_file_iter {
            let mut file = File::open(input_file)
                .with_context(|| format!("Could not open file '{}'", input_file))?;
            convert(&mut file, &mut output, format)?;
        }
    } else {
        // If no input file was specified, run the inspector on STDIN.

        // The inspector expects its input to be a byte array or mmap()ed file acting as a byte
        // array. If the user wishes to provide data on STDIN, we'll need to copy those bytes to
        // a temporary file and then read from that.

        // Create a temporary file that will delete itself when the program ends.
        let mut input_file = tempfile::tempfile().with_context(|| {
            concat!(
                "Failed to create a temporary file to store STDIN.",
                "Try passing an --input flag instead."
            )
        })?;

        // Pipe the data from STDIN to the temporary file.
        let mut writer = BufWriter::new(input_file);
        io::copy(&mut io::stdin(), &mut writer)
            .with_context(|| "Failed to copy STDIN to a temp file.")?;
        // Get our file handle back from the BufWriter
        input_file = writer
            .into_inner()
            .with_context(|| "Failed to read from temp file containing STDIN data.")?;
        convert(&mut input_file, &mut output, format)?;
    }

    output.flush()?;
    Ok(())
}

pub fn convert(file: &mut File, output: &mut Box<dyn Write>, format: &str) -> Result<()> {
    // NOTE: mmap logic is copied from inspect.inspect_file().

    // mmap involves operating system interactions that inherently place its usage outside of Rust's
    // safety guarantees. If the file is unexpectedly truncated while it's being read, for example,
    // problems could arise.
    let mmap = unsafe {
        MmapOptions::new()
            .map(file)
            .with_context(|| "Could not mmap ")?
    };

    // Treat the mmap as a byte array.
    let ion_data: &[u8] = &mmap[..];
    let mut reader = ReaderBuilder::new()
        .build(ion_data)
        .with_context(|| "No `source_format` was specified.")?;
    match format {
        "json" => {
            for result in reader.elements() {
                let element = result.with_context(|| "invalid input")?;
                writeln!(output, "{}", to_json_value(&element)?)?
            }
        }
        _ => {
            unimplemented!("Unsupported format.")
        }
    };
    Ok(())
}

fn to_json_value(element: &Element) -> Result<JsonValue> {
    if element.is_null() {
        Ok(JsonValue::Null)
    } else {
        use ion_rs::element::Value::*;
        let value = match element.value() {
            Null(_ion_type) => JsonValue::Null,
            Bool(b) => JsonValue::Bool(*b),
            Int(i) => JsonValue::Number(
                Number::from_str(&(*i).to_string())
                    .with_context(|| format!("{element} could not be turned into a Number"))?,
            ),
            Float(f) => {
                let value = *f;
                if value.is_finite() {
                    JsonValue::Number(
                        Number::from_f64(value).with_context(|| {
                            format!("{element} could not be turned into a Number")
                        })?,
                    )
                } else {
                    // +inf, -inf, and nan are not JSON numbers, and are written as null in
                    // accordance with Ion's JSON down-conversion guidelines.
                    JsonValue::Null
                }
            }
            Decimal(d) => JsonValue::Number(
                Number::from_str(d.to_string().replace('d', "e").as_str())
                    .with_context(|| format!("{element} could not be turned into a Number"))?,
            ),
            Timestamp(t) => JsonValue::String(t.to_string()),
            Symbol(s) => s
                .text()
                .map(|text| JsonValue::String(text.to_owned()))
                .unwrap_or_else(|| JsonValue::Null),
            String(s) => JsonValue::String(s.text().to_owned()),
            Blob(b) | Clob(b) => {
                use base64::{engine::general_purpose as base64_encoder, Engine as _};
                let base64_text = base64_encoder::STANDARD.encode(b.as_ref());
                JsonValue::String(base64_text)
            }
            List(s) | SExp(s) => {
                let result: Result<Vec<JsonValue>> = s.elements().map(to_json_value).collect();
                JsonValue::Array(result?)
            }
            Struct(s) => {
                let result: Result<Map<std::string::String, JsonValue>> = s
                    .fields()
                    .map(|(k, v)| to_json_value(v).map(|value| (k.text().unwrap().into(), value)))
                    .collect();
                JsonValue::Object(result?)
            }
        };
        Ok(value)
    }
}
