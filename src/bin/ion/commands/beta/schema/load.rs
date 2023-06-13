use crate::commands::IonCliCommand;
use anyhow::Result;
use clap::{Arg, ArgAction, ArgMatches, Command};
use ion_schema::authority::{DocumentAuthority, FileSystemDocumentAuthority};
use ion_schema::system::SchemaSystem;
use std::path::Path;

pub struct LoadCommand;

impl IonCliCommand for LoadCommand {
    fn name(&self) -> &'static str {
        "load"
    }

    fn about(&self) -> &'static str {
        r#"Loads an Ion Schema file using user provided schema id and returns a result message.\
        Shows an error message if there were any invalid schema syntax found during the load process"#
    }

    fn clap_command(&self) -> Command {
        Command::new(self.name())
            .about(self.about())
            .arg(
                // Input file can be specified by the "-s" or "--schema" flags.
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
                    // If this appears more than once, collect all values
                    .action(ArgAction::Append)
                    .value_name("DIRECTORY")
                    .required(true)
                    .help("One or more directories that will be searched for the requested schema"),
            )
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        // Extract the user provided document authorities/ directories
        let authorities: Vec<&String> = args.get_many("directories").unwrap().collect();

        // Extract schema file provided by user
        let schema_id = args.get_one::<String>("schema").unwrap();

        // Set up document authorities vector
        let mut document_authorities: Vec<Box<dyn DocumentAuthority>> = vec![];

        for authority in authorities {
            document_authorities.push(Box::new(FileSystemDocumentAuthority::new(Path::new(
                authority,
            ))))
        }

        // Create a new schema system from given document authorities
        let mut schema_system = SchemaSystem::new(document_authorities);

        // load given schema
        println!("Schema: {:#?}", schema_system.load_schema(schema_id)?);

        Ok(())
    }
}
