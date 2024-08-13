mod context;
mod generator;
mod result;
mod templates;
mod utils;

mod model;

use crate::commands::generate::generator::CodeGenerator;
use crate::commands::generate::utils::JavaLanguage;
use crate::commands::IonCliCommand;
use anyhow::{bail, Result};
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

    fn is_stable(&self) -> bool {
        false
    }

    fn is_porcelain(&self) -> bool {
        false
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
                    .short('s')
                    .help("Schema file name or schema id"),
            )
            // `--namespace` is required when Java language is specified for code generation
            .arg(
                Arg::new("namespace")
                    .long("namespace")
                    .short('n')
                    .required_if_eq("language", "java")
                    .help("Provide namespace for generated Java code (e.g. `org.example`)"),
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
        let language: &str = args.get_one::<String>("language").unwrap().as_str();

        // Extract namespace for code generation
        let namespace = args.get_one::<String>("namespace");

        // Extract output path information where the generated code will be saved
        // Create a module `ion_data_model` for storing all the generated code in the output directory
        let binding = match args.get_one::<String>("output") {
            Some(output_path) => PathBuf::from(output_path),
            None => PathBuf::from("./"),
        };

        let output = binding.as_path();

        // Extract the user provided document authorities/ directories
        let authorities: Vec<&String> = args.get_many("directory").unwrap().collect();

        // Set up document authorities vector
        let mut document_authorities: Vec<Box<dyn DocumentAuthority>> = vec![];

        for authority in &authorities {
            document_authorities.push(Box::new(FileSystemDocumentAuthority::new(Path::new(
                authority,
            ))))
        }

        // Create a new schema system from given document authorities
        let mut schema_system = SchemaSystem::new(document_authorities);

        // Generate directories in the output path if the path doesn't exist
        if !output.exists() {
            fs::create_dir_all(output).unwrap();
        }

        println!("Started generating code...");

        // Extract schema file provided by user
        match args.get_one::<String>("schema") {
            None => {
                // generate code based on schema and programming language
                match language {
                    "java" =>
                        CodeGenerator::<JavaLanguage>::new(output, namespace.unwrap().split('.').map(|s| s.to_string()).collect())
                            .generate_code_for_authorities(&authorities, &mut schema_system)?,
                    "rust" => {
                        // TODO: Initialize and run code generator for `rust`, once the rust templates are modified based on new code generation model
                        todo!("Rust support is disabled until this is resolved: https://github.com/amazon-ion/ion-cli/issues/136")
                    }
                    _ => bail!(
                        "Programming language '{}' is not yet supported. Currently supported targets: 'java', 'rust'",
                        language
                    )
                }
            }
            Some(schema_id) => {
                // generate code based on schema and programming language
                match language {
                    "java" => CodeGenerator::<JavaLanguage>::new(output, namespace.unwrap().split('.').map(|s| s.to_string()).collect()).generate_code_for_schema(&mut schema_system, schema_id)?,
                    "rust" => {
                        // TODO: Initialize and run code generator for `rust`, once the rust templates are modified based on new code generation model
                        todo!()
                    }
                    _ => bail!(
                        "Programming language '{}' is not yet supported. Currently supported targets: 'java', 'rust'",
                        language
                    )
                }
            }
        }

        println!("Code generation complete successfully!");
        println!("Path to generated code: {}", output.display());
        Ok(())
    }
}
