use convert_case::{Case, Casing};
use ion_schema::result::IonSchemaError;
use serde::Serialize;
use std::fmt::{Display, Formatter};
use tera::Tera;
use thiserror::Error;

/// Represents code generation result
pub type CodeGenResult<T> = Result<T, CodeGenError>;

/// Represents an error found during code generation
#[derive(Debug, Error)]
pub enum CodeGenError {
    #[error("{source:?}")]
    IonError {
        #[from]
        source: IonSchemaError,
    },
    #[error("{source:?}")]
    IoError {
        #[from]
        source: std::io::Error,
    },
    #[error("{source:?}")]
    TeraError {
        #[from]
        source: tera::Error,
    },
    #[error("{description}")]
    InvalidDataModel { description: String },
}

/// A convenience method for creating an CodeGen containing an CodeGenError::InvalidDataModel
/// with the provided description text.
pub fn invalid_data_model_error<T, S: AsRef<str>>(description: S) -> CodeGenResult<T> {
    Err(CodeGenError::InvalidDataModel {
        description: description.as_ref().to_string(),
    })
}

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

/// Represents a context that will be used for code generation
pub struct CodeGenContext {
    // Represents the templating engine - tera
    pub(crate) tera: Tera,
    pub(crate) language: Language,
    // Initially the data_model field is set to None.
    // Once an ISL type definition is mapped to a data model this will have Some value.
    pub(crate) data_model: Option<DataModel>,
    // Represents a counter for naming anonymous type definitions
    pub(crate) anonymous_type_counter: usize,
}

impl CodeGenContext {
    pub fn new(language: Language) -> Self {
        Self {
            language,
            data_model: None,
            anonymous_type_counter: 0,
            tera: Tera::new("src/bin/ion/commands/beta/generate/templates/**/*.templ").unwrap(),
        }
    }

    pub fn with_data_model(&mut self, data_model: DataModel) {
        self.data_model = Some(data_model);
    }

    pub fn with_initial_data_model(&mut self) {
        // Initially the data model is set to None, this will be set with Some(_) value when data model is determined in code generation process
        self.data_model = None;
    }

    /// Returns a string that represent the template name based on data model type.
    pub fn template_name(&self) -> &str {
        if let Some(data_model) = &self.data_model {
            return match (&self.language, data_model) {
                (
                    Language::Rust,
                    DataModel::Struct | DataModel::UnitStruct | DataModel::SequenceStruct,
                ) => "struct",
                (
                    Language::Java,
                    DataModel::Struct | DataModel::UnitStruct | DataModel::SequenceStruct,
                ) => "class",
            };
        }
        "" // Default value is an empty string
    }
}

/// Represents a data model type that can be used to determine which templates can be used for code generation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum DataModel {
    UnitStruct,     // a struct with a scalar value (used for `type` constraint)
    SequenceStruct, // a struct with a sequence value (used for `element` constraint)
    Struct,
}
