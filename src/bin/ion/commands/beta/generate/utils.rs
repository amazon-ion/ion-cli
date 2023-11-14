use convert_case::{Case, Casing};
use serde::Serialize;
use std::fmt::{Display, Formatter};

/// Represents a field that will be added to generated data model.
/// This will be used by the template engine to fill properties of a struct/classs.
#[derive(Serialize)]
pub struct Field {
    pub(crate) name: String,
    pub(crate) value: String,
}

/// Represents an import statement in a module file.
/// this will be used by template engine to fill import statements of a type.
#[derive(Serialize)]
pub struct Import {
    pub(crate) module_name: String,
    pub(crate) type_name: String,
}

/// Represent the programming language for code generation.
#[derive(Debug, Clone, PartialEq)]
pub enum Language {
    Rust,
    Java,
}

impl Language {
    pub fn file_extension(&self) -> &str {
        match self {
            Language::Rust => "rs",
            Language::Java => "java",
        }
    }

    pub fn file_name(&self, name: &str) -> String {
        match self {
            Language::Rust => name.to_case(Case::Snake),
            Language::Java => name.to_case(Case::UpperCamel),
        }
    }
}

impl From<&str> for Language {
    fn from(value: &str) -> Self {
        match value {
            "java" => Language::Java,
            "rust" => Language::Rust,
            _ => unreachable!("Unsupported programming language: {}, this tool only supports Java and Rust code generation.", value)
        }
    }
}

impl Display for Language {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::Rust => {
                write!(f, "rust")
            }
            Language::Java => {
                write!(f, "java")
            }
        }
    }
}
