use crate::commands::IonCliCommand;
use anyhow::{Context, Result};
use clap::{Arg, ArgAction, ArgMatches, Command};
use ion_schema::authority::{DocumentAuthority, FileSystemDocumentAuthority};
use ion_schema::external::ion_rs::element::reader::ElementReader;
use ion_schema::external::ion_rs::element::writer::ElementWriter;
use ion_schema::external::ion_rs::element::writer::TextKind;
use ion_schema::external::ion_rs::element::Element;
use ion_schema::external::ion_rs::{IonResult, TextWriterBuilder};
use ion_schema::external::ion_rs::{IonType, IonWriter, ReaderBuilder};
use ion_schema::system::SchemaSystem;
use std::fs;
use std::path::Path;
use std::str::from_utf8;

pub struct ValidateCommand;

impl IonCliCommand for ValidateCommand {
    fn name(&self) -> &'static str {
        "validate"
    }

    fn about(&self) -> &'static str {
        "Validates an Ion value based on a given Ion Schema type."
    }

    fn configure_args(&self, command: Command) -> Command {
        command
            .arg(
                // Input ion file can be specified by the "-i" or "--input" flags.
                Arg::new("input")
                    .long("input")
                    .short('i')
                    .required(true)
                    .value_name("INPUT_FILE")
                    .help("Input file containing the Ion values to be validated"),
            )
            .arg(
                // Schema file can be specified by the "-s" or "--schema" flags.
                Arg::new("schema")
                    .long("schema")
                    .short('s')
                    .required(true)
                    .value_name("SCHEMA")
                    .help("The Ion Schema file to load"),
            )
            .arg(
                // Directory(s) that will be used as authority(s) for schema system
                Arg::new("directories")
                    .long("directory")
                    .short('d')
                    .action(ArgAction::Append)
                    .value_name("DIRECTORY")
                    .required(true)
                    .help("One or more directories that will be searched for the requested schema"),
            )
            .arg(
                // Schema Type can be specified by the "-t" or "--type" flags.
                Arg::new("type")
                    .long("type")
                    .short('t')
                    .required(true)
                    .value_name("TYPE")
                    .help("Name of schema type from given schema that needs to be used for validation"),
            )
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        // Extract the user provided document authorities/ directories
        let authorities: Vec<&String> = args.get_many("directories").unwrap().collect();

        // Extract schema file provided by user
        let schema_id = args.get_one::<String>("schema").unwrap();

        // Extract the schema type provided by user
        let schema_type = args.get_one::<String>("type").unwrap();

        // Extract Ion value provided by user
        let input_file = args.get_one::<String>("input").unwrap();
        let value =
            fs::read(input_file).with_context(|| format!("Could not open '{}'", schema_id))?;
        let owned_elements: Vec<Element> = ReaderBuilder::new()
            .build(value.as_slice())?
            .read_all_elements()
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
        let mut writer = TextWriterBuilder::new(TextKind::Pretty).build(&mut output)?;

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
}

// TODO: this will be provided by Element's implementation of `Display` in a future
//       release of ion-rs.
fn element_to_string(element: &Element) -> IonResult<String> {
    let mut buffer = Vec::new();
    let mut text_writer = TextWriterBuilder::new(TextKind::Pretty).build(&mut buffer)?;
    text_writer.write_element(element)?;
    text_writer.flush()?;
    Ok(from_utf8(text_writer.output().as_slice())
        .expect("Invalid UTF-8 output")
        .to_string())
}
