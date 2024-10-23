use crate::commands::schema::IonSchemaCommandInput;
use crate::commands::IonCliCommand;
use anyhow::Result;
use clap::{Arg, ArgAction, ArgMatches, Command};

pub struct CheckCommand;

impl IonCliCommand for CheckCommand {
    fn name(&self) -> &'static str {
        "check"
    }

    fn about(&self) -> &'static str {
        "Loads a schema and checks it for problems."
    }

    fn is_stable(&self) -> bool {
        false
    }

    fn configure_args(&self, command: Command) -> Command {
        command.args(IonSchemaCommandInput::schema_args()).arg(
            Arg::new("show-debug")
                .short('D')
                .long("show-debug")
                .action(ArgAction::SetTrue),
        )
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        let ion_schema_input = IonSchemaCommandInput::read_from_args(args)?;
        let schema = ion_schema_input.get_schema();
        if args.get_flag("show-debug") {
            println!("Schema: {:#?}", schema);
        }
        Ok(())
    }
}
