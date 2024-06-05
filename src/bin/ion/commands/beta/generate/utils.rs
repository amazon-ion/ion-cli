use crate::commands::beta::generate::context::AbstractDataType;
use crate::commands::beta::generate::result::{invalid_abstract_data_type_error, CodeGenError};
use convert_case::{Case, Casing};
use serde::Serialize;
use std::fmt::{Display, Formatter};

/// Represents a field that will be added to generated data model.
/// This will be used by the template engine to fill properties of a struct/class.
#[derive(Serialize)]
pub struct Field {
    pub(crate) name: String,
    // The value_type represents the AbstractDatType for given field. When given ISL has constraints, that lead to open ended types,
    // this will be ste to None, Otherwise set to Some(ABSTRACT_DATA_TYPE_NAME).
    // e.g For below ISL type:
    // ```
    // type::{
    //   name: list_type,
    //   type: list // since this doesn't have `element` constraint defined it will be set `value_type` to None
    // }
    // ```
    // Following will be the `Field` value for this ISL type:
    // Field {
    //     name: value,
    //     value_type: None,
    //     isl_type_name: "list"
    // }
    // Code generation process results into an Error when `value_type` is set to `None`
    pub(crate) value_type: Option<String>,
    pub(crate) isl_type_name: String,
}

/// Represents an nested type that can be a part of another type definition.
/// This will be used by the template engine to add these intermediate data models for nested types
/// in to the parent type definition's module/namespace.
#[derive(Serialize)]
pub struct NestedType {
    pub(crate) target_kind_name: String,
    pub(crate) fields: Vec<Field>,
    pub(crate) abstract_data_type: AbstractDataType,
    pub(crate) nested_types: Vec<NestedType>,
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
    fn target_type(ion_schema_type: &IonSchemaType) -> Option<String>;

    /// Provides given target type as sequence
    /// e.g.
    ///     target_type = "Foo" returns "ArrayList<Foo>"
    ///     target_type = "Foo" returns "Vec<Foo>"
    fn target_type_as_sequence(target_type: &Option<String>) -> Option<String>;

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

    fn target_type(ion_schema_type: &IonSchemaType) -> Option<String> {
        use IonSchemaType::*;
        Some(
            match ion_schema_type {
                Int => "int",
                String | Symbol => "String",
                Float => "double",
                Bool => "boolean",
                Blob | Clob => "byte[]",
                List | SExp | Struct => return None,
                SchemaDefined(name) => name,
            }
            .to_string(),
        )
    }

    fn target_type_as_sequence(target_type: &Option<String>) -> Option<String> {
        target_type.as_ref().map(|target_type_name| {
            match JavaLanguage::wrapper_class(target_type_name) {
                Some(wrapper_name) => format!("ArrayList<{}>", wrapper_name),
                None => format!("ArrayList<{}>", target_type_name),
            }
        })
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
    fn wrapper_class(primitive_data_type: &str) -> Option<String> {
        match primitive_data_type {
            "int" => Some("Integer".to_string()),
            "bool" => Some("Boolean".to_string()),
            "double" => Some("Double".to_string()),
            "long" => Some("Long".to_string()),
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

    fn target_type(ion_schema_type: &IonSchemaType) -> Option<String> {
        use IonSchemaType::*;
        Some(
            match ion_schema_type {
                Int => "i64",
                String | Symbol => "String",
                Float => "f64",
                Bool => "bool",
                Blob | Clob => "Vec<u8>",
                List | SExp | Struct => return None,
                SchemaDefined(name) => name,
            }
            .to_string(),
        )
    }

    fn target_type_as_sequence(target_type: &Option<String>) -> Option<String> {
        target_type
            .as_ref()
            .map(|target_type_name| format!("Vec<{}>", target_type_name))
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
    Struct,
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
            "struct" => Struct,
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
