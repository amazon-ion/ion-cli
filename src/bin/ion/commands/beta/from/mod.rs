use crate::commands::IonCliCommand;

use crate::commands::beta::from::json::FromJsonCommand;

pub mod json;

pub struct FromNamespace;

impl IonCliCommand for FromNamespace {
    fn name(&self) -> &'static str {
        "from"
    }

    fn about(&self) -> &'static str {
        "'from' is a namespace for commands that convert other data formats to Ion."
    }

    fn subcommands(&self) -> Vec<Box<dyn IonCliCommand>> {
        vec![Box::new(FromJsonCommand)]
    }
}
