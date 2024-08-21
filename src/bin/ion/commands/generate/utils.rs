use crate::commands::generate::model::{
    AbstractDataType, DataModelNode, FullyQualifiedTypeReference,
};
use crate::commands::generate::result::{invalid_abstract_data_type_error, CodeGenError};
use convert_case::{Case, Casing};
use std::fmt::{Display, Formatter};

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
    ///     target_type = "Foo" returns "java.util.ArrayList<Foo>"
    ///     target_type = "Foo" returns "Vec<Foo>"
    #[allow(dead_code)]
    fn target_type_as_sequence(
        target_type: FullyQualifiedTypeReference,
    ) -> FullyQualifiedTypeReference;

    /// Returns true if the type `String` specified is provided by the target language implementation
    fn is_built_in_type(type_name: String) -> bool;

    /// Returns a fully qualified type reference name as per the programming language
    /// e.g. For a fully qualified type reference as below:
    ///   FullyQualifiedTypeReference {
    ///     type_name: vec!["org", "example", "Foo"],
    ///     parameters: vec![] // type ref with no parameters
    ///   }
    ///   In Java, `org.example.Foo`
    ///   In Rust, `org::example::Foo`
    #[allow(dead_code)]
    fn fully_qualified_type_ref(name: &FullyQualifiedTypeReference) -> String;

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

    fn target_type_as_sequence(
        target_type: FullyQualifiedTypeReference,
    ) -> FullyQualifiedTypeReference {
        match JavaLanguage::wrapper_class(&format!("{}", target_type)) {
            Some(wrapper_name) => FullyQualifiedTypeReference {
                type_name: vec![
                    "java".to_string(),
                    "util".to_string(),
                    "ArrayList".to_string(),
                ],
                parameters: vec![FullyQualifiedTypeReference {
                    type_name: vec![wrapper_name],
                    parameters: vec![],
                }],
            },
            None => FullyQualifiedTypeReference {
                type_name: vec![
                    "java".to_string(),
                    "util".to_string(),
                    "ArrayList".to_string(),
                ],
                parameters: vec![target_type],
            },
        }
    }

    fn is_built_in_type(type_name: String) -> bool {
        matches!(
            type_name.as_str(),
            "int"
                | "String"
                | "boolean"
                | "byte[]"
                | "double"
                | "java.util.ArrayList<String>"
                | "java.util.ArrayList<Integer>"
                | "java.util.ArrayList<Boolean>"
                | "java.util.ArrayList<Double>"
        )
    }

    fn fully_qualified_type_ref(name: &FullyQualifiedTypeReference) -> String {
        name.type_name.join(".")
    }

    fn template_name(template: &Template) -> String {
        match template {
            Template::Struct => "class".to_string(),
            Template::Scalar => "scalar".to_string(),
            Template::Sequence => "sequence".to_string(),
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

    fn target_type_as_sequence(
        target_type: FullyQualifiedTypeReference,
    ) -> FullyQualifiedTypeReference {
        FullyQualifiedTypeReference {
            type_name: vec!["Vec".to_string()],
            parameters: vec![target_type],
        }
    }

    fn is_built_in_type(type_name: String) -> bool {
        matches!(
            type_name.as_str(),
            "i64"
                | "String"
                | "bool"
                | "Vec<u8>"
                | "f64"
                | "Vec<String>"
                | "Vec<i64>"
                | "Vec<bool>"
                | "Vec<f64>"
        )
    }

    fn fully_qualified_type_ref(name: &FullyQualifiedTypeReference) -> String {
        name.type_name.join("::")
    }

    fn template_name(template: &Template) -> String {
        match template {
            Template::Struct => "struct".to_string(),
            Template::Scalar => "scalar".to_string(),
            Template::Sequence => "sequence".to_string(),
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
    Struct,   // Represents a template for a Rust struct or Java class with Ion struct value
    Sequence, // Represents a template for a Rust struct or Java class with Ion sequence value
    Scalar,   // Represents a template for a Rust struct or Java class with Ion scalar value
}

impl TryFrom<&DataModelNode> for Template {
    type Error = CodeGenError;

    fn try_from(value: &DataModelNode) -> Result<Self, Self::Error> {
        if let Some(abstract_data_type) = &value.code_gen_type {
            match abstract_data_type {
                AbstractDataType::Scalar(_) | AbstractDataType::WrappedScalar(_) => {
                    Ok(Template::Scalar)
                }
                AbstractDataType::Sequence(_) => Ok(Template::Sequence),
                AbstractDataType::Structure(_) => Ok(Template::Struct),
            }
        } else {
            invalid_abstract_data_type_error(
                "Can not get a template without determining data model first.",
            )
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
