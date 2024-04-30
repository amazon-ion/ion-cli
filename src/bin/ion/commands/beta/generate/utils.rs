use crate::commands::beta::generate::context::AbstractDataType;
use crate::commands::beta::generate::result::{invalid_abstract_data_type_error, CodeGenError};
use convert_case::{Case, Casing};
use serde::Serialize;
use std::fmt::{Display, Formatter};

/// Represents a field that will be added to generated data model.
/// This will be used by the template engine to fill properties of a struct/classs.
#[derive(Serialize)]
pub struct Field {
    pub(crate) name: String,
    pub(crate) value_type: String,
    pub(crate) isl_type_name: String,
}

pub trait Language {
    /// Provides a file extension based on programming language
    fn file_extension() -> String;

    /// Returns string representation of programming language name
    fn name() -> String;

    /// Provides generated code's file name for given type `name` based on programming language standards
    /// e.g.
    ///     In Rust, this will return a string casing `name` to [Case::Snake].
    ///     In Java, this will return a string casing `name` to  [Case::UpperCamel]
    fn file_name_for_type(name: &str) -> String;

    /// Maps the given ISL type to a target type name
    fn target_type(ion_schema_type: &IonSchemaType) -> String;

    /// Provides given target type as sequence
    /// e.g.
    ///     target_type = "Foo" returns "ArrayList<Foo>"
    ///     target_type = "Foo" returns "Vec<Foo>"
    fn target_type_as_sequence(target_type: &str) -> String;

    /// Returns the [Case] based on programming languages
    /// e.g.  
    ///     Rust field name case -> [Case::Snake]
    ///     Java field name case -> [Case::Camel]
    fn field_name_case() -> Case;

    /// Returns true if the type name specified is provided by the target language implementation
    fn is_built_in_type(name: &str) -> bool;

    /// Returns the template as string based on programming language
    /// e.g.
    ///     In Rust, Template::Struct -> "struct"
    ///     In Java, Template::Struct -> "class"
    fn template_name(template: &Template) -> String;
}

pub struct JavaLanguage;

impl Language for JavaLanguage {
    fn file_extension() -> String {
        "java".to_string()
    }

    fn name() -> String {
        "java".to_string()
    }

    fn file_name_for_type(name: &str) -> String {
        name.to_case(Case::UpperCamel)
    }

    fn target_type(ion_schema_type: &IonSchemaType) -> String {
        use IonSchemaType::*;
        match ion_schema_type {
            Int => "int",
            String | Symbol => "String",
            Float => "double",
            Bool => "boolean",
            Blob | Clob => "byte[]",
            List | SExp => "Object",
            SchemaDefined(name) => name,
        }
        .to_string()
    }

    fn target_type_as_sequence(target_type: &str) -> String {
        match JavaLanguage::wrapper_class(target_type) {
            Some(wrapper_class_name) => {
                format!("ArrayList<{}>", wrapper_class_name)
            }
            None => {
                format!("ArrayList<{}>", target_type)
            }
        }
    }

    fn field_name_case() -> Case {
        Case::Camel
    }

    fn is_built_in_type(name: &str) -> bool {
        matches!(name, "int" | "String" | "boolean" | "byte[]" | "double")
    }

    fn template_name(template: &Template) -> String {
        match template {
            Template::Struct => "class".to_string(),
        }
    }
}

impl JavaLanguage {
    fn wrapper_class(primitive_data_type_name: &str) -> Option<&str> {
        match primitive_data_type_name {
            "int" => Some("Integer"),
            "bool" => Some("Boolean"),
            "double" => Some("Double"),
            "long" => Some("Long"),
            _ => {
                // for any other non-primitive types return None
                None
            }
        }
    }
}

impl Display for JavaLanguage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "java")
    }
}

pub struct RustLanguage;

impl Language for RustLanguage {
    fn file_extension() -> String {
        "rs".to_string()
    }

    fn name() -> String {
        "rust".to_string()
    }

    fn file_name_for_type(_name: &str) -> String {
        "ion_generated_code".to_string()
    }

    fn target_type(ion_schema_type: &IonSchemaType) -> String {
        use IonSchemaType::*;
        match ion_schema_type {
            Int => "i64",
            String | Symbol => "String",
            Float => "f64",
            Bool => "bool",
            Blob | Clob => "Vec<u8>",
            List | SExp => "T",
            SchemaDefined(name) => name,
        }
        .to_string()
    }

    fn target_type_as_sequence(target_type: &str) -> String {
        format!("Vec<{}>", target_type)
    }

    fn field_name_case() -> Case {
        Case::Snake
    }

    fn is_built_in_type(name: &str) -> bool {
        matches!(name, "i64" | "String" | "bool" | "Vec<u8>" | "f64")
    }

    fn template_name(template: &Template) -> String {
        match template {
            Template::Struct => "struct".to_string(),
        }
    }
}

impl Display for RustLanguage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "rust")
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

impl TryFrom<Option<&AbstractDataType>> for Template {
    type Error = CodeGenError;

    fn try_from(value: Option<&AbstractDataType>) -> Result<Self, Self::Error> {
        match value {
            Some(_) => Ok(Template::Struct),
            None => invalid_abstract_data_type_error(
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
    SExp,
    List,
    SchemaDefined(String), // A user defined schema type
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
            "struct" => {
                unimplemented!("Generic containers aren't supported yet!")
            }
            "list" => List,
            "sexp" => SExp,
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
