use crate::commands::{IonCliCommand, WithIonCliArgument};
use anyhow::{bail, Context, Result};
use clap::{ArgMatches, Command};
use ion_rs::Element;
use ion_rs::ElementReader;
use ion_rs::{IonType, Reader, ReaderBuilder};
use std::fs::File;

pub struct DepthCommand;

impl IonCliCommand for DepthCommand {
    fn name(&self) -> &'static str {
        "depth"
    }

    fn about(&self) -> &'static str {
        "Prints the maximum depth of the input ion stream."
    }

    fn configure_args(&self, command: Command) -> Command {
        command.with_input()
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        if let Some(input_file_iter) = args.get_many::<String>("input") {
            for input_file in input_file_iter {
                let file = File::open(input_file)
                    .with_context(|| format!("Could not open file '{}'", input_file))?;
                let mut reader = ReaderBuilder::new().build(file)?;
                get_depth(&mut reader)?;
            }
        } else {
            bail!("this command does not yet support reading from STDIN")
        };
        Ok(())
    }
}

fn get_depth(reader: &mut Reader) -> Result<()> {
    let mut max_depth = 0;
    for element in reader.elements() {
        let unwrap_element = element.unwrap();
        max_depth = calculate_depth(&unwrap_element, 0);
    }
    println!("The maximum depth is {}", max_depth);
    Ok(())
}

fn calculate_depth(element: &Element, depth: usize) -> usize {
    return if element.ion_type().is_container() {
        if element.ion_type() == IonType::Struct {
            element
                .as_struct()
                .unwrap()
                .iter()
                .map(|(_field_name, e)| calculate_depth(e, depth + 1))
                .max()
                .unwrap_or(depth)
        } else {
            element
                .as_sequence()
                .unwrap()
                .into_iter()
                .map(|e| calculate_depth(e, depth + 1))
                .max()
                .unwrap_or(depth)
        }
    } else {
        depth
    };
}
