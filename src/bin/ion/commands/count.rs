use anyhow::Result;
use clap::{ArgMatches, Command};
use ion_rs::*;

use crate::commands::{CommandIo, IonCliCommand, WithIonCliArgument};

pub struct CountCommand;

impl IonCliCommand for CountCommand {
    fn name(&self) -> &'static str {
        "count"
    }

    fn about(&self) -> &'static str {
        "Prints the number of top-level values found in the input stream."
    }

    fn is_stable(&self) -> bool {
        false
    }

    fn configure_args(&self, command: Command) -> Command {
        command.with_input()
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        CommandIo::new(args).for_each_input(|_output, input| {
            let mut reader = Reader::new(AnyEncoding, input.into_source())?;
            print_top_level_value_count(&mut reader)
        })
    }
}

fn print_top_level_value_count<I: IonInput>(reader: &mut Reader<AnyEncoding, I>) -> Result<()> {
    let mut count: usize = 0;
    while let Some(_) = reader.next()? {
        count += 1;
    }
    println!("{}", count);
    Ok(())
}
