use crate::commands::beta::generate::context::DataModel;
use crate::commands::beta::generate::result::{invalid_data_model_error, CodeGenError};
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
/// This will be used by template engine to fill import statements of a type definition.
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

/// Represents all the supported templates for code generation.
/// These templates will be used by [tera] templating engine to render the generated code with appropriate context value.
/// _Note: These template variants are based on Rust programming language.
/// [Template::name] provides an appropriate template file name based on given programming language._
///
/// [tera]: <https://docs.rs/tera/latest/tera/>
pub enum Template {
    Struct, // Represents a template for a Rust struct or Java class
}

impl Template {
    /// Returns a string that represent the template file name based on given programming language.
    pub fn name(&self, language: &Language) -> &str {
        match language {
            Language::Rust => "struct",
            Language::Java => "class",
        }
    }
}

impl TryFrom<Option<&DataModel>> for Template {
    type Error = CodeGenError;

    fn try_from(value: Option<&DataModel>) -> Result<Self, Self::Error> {
        match value {
            Some(DataModel::Value) | Some(DataModel::Sequence) | Some(DataModel::Struct) => {
                Ok(Template::Struct)
            }
            None => invalid_data_model_error(
                "Can not get a template without determining data model first.",
            ),
        }
    }
}

/// Represents an Ion schema type which could either be one of the [built-int types] or a user defined type.
///
/// [built-in types]: `<https://amazon-ion.github.io/ion-schema/docs/isl-2-0/spec#built-in-types>`
// TODO: Add enum variants for missing built-in ISL types.
pub enum IonSchemaType {
    Int,
    String,
    Symbol,
    Float,
    Bool,
    Blob,
    Clob,
    SchemaDefined(String), // A user defined schema type
}

impl IonSchemaType {
    /// Maps the given ISL type name to a target type
    pub fn target_type(&self, language: &Language) -> &str {
        use IonSchemaType::*;
        use Language::*;
        match (self, language) {
            (Int, Rust) => "i64",
            (Int, Java) => "int",
            (String | Symbol, _) => "String",
            (Float, Rust) => "f64",
            (Float, Java) => "double",
            (Bool, Rust) => "bool",
            (Bool, Java) => "boolean",
            (Blob | Clob, Rust) => "Vec<u8>",
            (Blob | Clob, Java) => "byte[]",
            (SchemaDefined(name), _) => name,
        }
    }
}

impl From<&str> for IonSchemaType {
    fn from(value: &str) -> Self {
        use IonSchemaType::*;
        match value {
            "int" => Int,
            "string" => String,
            "symbol" => Symbol,
            "float" => Float,
            "bool" => Bool,
            "blob" => Blob,
            "clob" => Clob,
            _ if &value[..1] == "$" => {
                unimplemented!("Built in types with nulls are not supported yet!")
            }
            "number" | "text" | "lob" | "document" | "nothing" => {
                unimplemented!("Complex types are not supported yet!")
            }
            "decimal" | "timestamp" => {
                unimplemented!("Decimal, Number and Timestamp aren't support yet!")
            }
            "list" | "struct" | "sexp" => {
                unimplemented!("Generic containers aren't supported yet!")
            }
            _ => SchemaDefined(value.to_case(Case::UpperCamel)),
        }
    }
}

impl From<String> for IonSchemaType {
    fn from(value: String) -> Self {
        value.as_str().into()
    }
}

impl From<&String> for IonSchemaType {
    fn from(value: &String) -> Self {
        value.as_str().into()
    }
}
