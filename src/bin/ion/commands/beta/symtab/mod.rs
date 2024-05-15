use crate::commands::beta::symtab::filter::SymtabFilterCommand;
use crate::commands::IonCliCommand;

pub mod filter;

pub struct SymtabNamespace;

impl IonCliCommand for SymtabNamespace {
    fn name(&self) -> &'static str {
        "symtab"
    }

    fn about(&self) -> &'static str {
        "'symtab' is a namespace for commands that operate on symbol tables"
    }

    fn subcommands(&self) -> Vec<Box<dyn IonCliCommand>> {
        vec![Box::new(SymtabFilterCommand)]
    }
}
