use anyhow::{Context, Result};
use clap::{App, Arg, ArgMatches};
use ion_schema::authority::{DocumentAuthority, FileSystemDocumentAuthority};
use ion_schema::external::ion_rs::value::native_writer::NativeElementWriter;
use ion_schema::external::ion_rs::value::owned::OwnedElement;
use ion_schema::external::ion_rs::value::reader::{element_reader, ElementReader};
use ion_schema::external::ion_rs::value::writer::ElementWriter;
use ion_schema::external::ion_rs::IonType;
use ion_schema::external::ion_rs::{IonResult, TextWriterBuilder, Writer};
use ion_schema::system::SchemaSystem;
use std::fs;
use std::path::Path;
use std::str::from_utf8;

const ABOUT: &str = "validates an Ion Value based on given Ion Schema Type";

// Creates a `clap` (Command Line Arguments Parser) configuration for the `load` command.
// This function is invoked by the `load` command's parent `schema`, so it can describe its
// child commands.
pub fn app() -> App<'static, 'static> {
    App::new("validate")
        .about(ABOUT)
        .arg(
            // Schema file can be specified by the "-s" or "--schema" flags.
            Arg::with_name("schema")
                .long("schema")
                .short("s")
                .required(true)
                .takes_value(true)
                .value_name("SCHEMA")
                .help("The Ion Schema file to load"),
        )
        .arg(
            // Directory(s) that will be used as authority(s) for schema system
            Arg::with_name("directories")
                .long("directory")
                .short("d")
                .min_values(1)
                .takes_value(true)
                .multiple(true)
                .value_name("DIRECTORY")
                .required(true)
                .help("One or more directories that will be searched for the requested schema"),
        )
        .arg(
            // Input ion file can be specified by the "-i" or "--input" flags.
            Arg::with_name("input")
                .long("input")
                .short("i")
                .required(true)
                .takes_value(true)
                .value_name("INPUT_FILE")
                .help("Input file containing the Ion values to be validated"),
        )
        .arg(
            // Schema Type can be specified by the "-t" or "--type" flags.
            Arg::with_name("type")
                .long("type")
                .short("t")
                .required(true)
                .takes_value(true)
                .value_name("TYPE")
                .help("Name of schema type from given schema that needs to be used for validation"),
        )
}

// This function is invoked by the `load` command's parent `schema`.
pub fn run(_command_name: &str, matches: &ArgMatches<'static>) -> Result<()> {
    // Extract the user provided document authorities/ directories
    let authorities: Vec<_> = matches.values_of("directories").unwrap().collect();

    // Extract schema file provided by user
    let schema_id = matches.value_of("schema").unwrap();

    // Extract the schema type provided by user
    let schema_type = matches.value_of("type").unwrap();

    // Extract Ion value provided by user
    let input_file = matches.value_of("input").unwrap();
    let value = fs::read(input_file).with_context(|| format!("Could not open '{}'", schema_id))?;
    let owned_elements: Vec<OwnedElement> = element_reader()
        .read_all(&value)
        .with_context(|| format!("Could not parse Ion file: '{}'", schema_id))?;

    // Set up document authorities vector
    let mut document_authorities: Vec<Box<dyn DocumentAuthority>> = vec![];

    for authority in authorities {
        document_authorities.push(Box::new(FileSystemDocumentAuthority::new(Path::new(
            authority,
        ))))
    }

    // Create a new schema system from given document authorities
    let mut schema_system = SchemaSystem::new(document_authorities);

    // load schema
    let schema = schema_system.load_schema(schema_id);

    // get the type provided by user from the schema file
    let type_ref = schema?
        .get_type(schema_type)
        .with_context(|| format!("Schema {} does not have type {}", schema_id, schema_type))?;

    // create a text writer to make the output
    let mut output = vec![];
    let mut writer = TextWriterBuilder::new().build(&mut output)?;

    // validate owned_elements according to type_ref
    for owned_element in owned_elements {
        // create a validation report with validation result, value, schema and/or violation
        writer.step_in(IonType::Struct)?;
        let validation_result = type_ref.validate(&owned_element);
        writer.set_field_name("result");
        match validation_result {
            Ok(_) => {
                writer.write_string("Valid")?;
                writer.set_field_name("value");
                writer.write_string(element_to_string(&owned_element)?)?;
                writer.set_field_name("schema");
                writer.write_string(schema_id)?;
            }
            Err(error) => {
                writer.write_string("Invalid")?;
                writer.set_field_name("violation");
                writer.write_string(format!("{:#?}", error))?;
            }
        }
        writer.step_out()?;
    }
    drop(writer);
    println!("Validation report:");
    println!("{}", from_utf8(&output).unwrap());
    Ok(())
}

// TODO: this will be provided by OwnedElement's implementation of `Display` in a future
//       release of ion-rs.
fn element_to_string(element: &OwnedElement) -> IonResult<String> {
    let mut buffer = Vec::new();
    let text_writer = TextWriterBuilder::new().build(std::io::Cursor::new(&mut buffer))?;
    let mut element_writer = NativeElementWriter::new(text_writer);
    element_writer.write(element)?;
    let mut text_writer = element_writer.finish()?;
    text_writer.flush()?;
    drop(text_writer);
    Ok(from_utf8(buffer.as_slice()).unwrap().to_string())
}
