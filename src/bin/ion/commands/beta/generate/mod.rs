mod context;
mod generator;
mod result;
mod utils;

use crate::commands::beta::generate::generator::CodeGenerator;
use crate::commands::beta::generate::utils::Language;
use crate::commands::IonCliCommand;
use anyhow::Result;
use clap::{Arg, ArgAction, ArgMatches, Command};
use ion_schema::authority::{DocumentAuthority, FileSystemDocumentAuthority};
use ion_schema::system::SchemaSystem;
use std::fs;
use std::path::{Path, PathBuf};

pub struct GenerateCommand;

impl IonCliCommand for GenerateCommand {
    fn name(&self) -> &'static str {
        "generate"
    }

    fn about(&self) -> &'static str {
        "Generates code using given schema file."
    }

    fn configure_args(&self, command: Command) -> Command {
        command
            .arg(
                Arg::new("output")
                    .long("output")
                    .short('o')
                    .help("Output directory [default: current directory]"),
            )
            .arg(
                Arg::new("schema")
                    .long("schema")
                    .required(true)
                    .short('s')
                    .help("Schema file"),
            )
            .arg(
                Arg::new("language")
                    .long("language")
                    .short('l')
                    .required(true)
                    .value_parser(["java", "rust"])
                    .help("Programming language for the generated code"),
            )
            .arg(
                // Directory(s) that will be used as authority(s) for schema system
                Arg::new("directory")
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
        // Extract programming language for code generation
        let language: Language = args.get_one::<String>("language").unwrap().as_str().into();

        // Extract output path information where the generated code will be saved
        // Create a module `ion_data_model` for storing all the generated code in the output directory
        let binding = match args.get_one::<String>("output") {
            Some(output_path) => PathBuf::from(output_path).join("ion_data_model"),
            None => PathBuf::from("./ion_data_model"),
        };

        let output = binding.as_path();

        // Extract the user provided document authorities/ directories
        let authorities: Vec<&String> = args.get_many("directory").unwrap().collect();

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

        let schema = schema_system.load_isl_schema(schema_id).unwrap();

        // clean the target output directory if it already exists, before generating new code
        if output.exists() {
            fs::remove_dir_all(output).unwrap();
        }
        fs::create_dir_all(output).unwrap();

        println!("Started generating code...");

        // generate code based on schema and programming language
        CodeGenerator::new(language, output).generate(schema)?;

        println!("Code generation complete successfully!");
        println!("Path to generated code: {}", output.display());
        Ok(())
    }
}
