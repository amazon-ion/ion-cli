use crate::commands::beta::symtab::count::SymbolTableCommand;
use crate::commands::beta::symtab::filter::SymtabFilterCommand;
use crate::commands::beta::symtab::symbol_count::SymbolNumberCommand;
use crate::commands::IonCliCommand;

pub mod count;
pub mod filter;
pub mod symbol_count;

pub struct SymtabNamespace;

impl IonCliCommand for SymtabNamespace {
    fn name(&self) -> &'static str {
        "symtab"
    }

    fn about(&self) -> &'static str {
        "'symtab' is a namespace for commands that operate on symbol tables"
    }

    fn subcommands(&self) -> Vec<Box<dyn IonCliCommand>> {
        vec![
            Box::new(SymtabFilterCommand),
            Box::new(SymbolTableCommand),
            Box::new(SymbolNumberCommand),
        ]
    }
}
