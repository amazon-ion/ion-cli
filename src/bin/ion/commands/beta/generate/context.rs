use serde::Serialize;
use std::fmt::{Display, Formatter};

/// Represents a context that will be used for code generation
pub struct CodeGenContext {
    // Initially the abstract_data_type field is set to None.
    // Once an ISL type definition is mapped to an abstract data type this will have Some value.
    pub(crate) abstract_data_type: Option<AbstractDataType>,
}

impl CodeGenContext {
    pub fn new() -> Self {
        Self {
            abstract_data_type: None,
        }
    }

    pub fn with_abstract_data_type(&mut self, abstract_data_type: AbstractDataType) {
        self.abstract_data_type = Some(abstract_data_type);
    }
}

/// A target-language-agnostic data type that determines which template(s) to use for code generation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum AbstractDataType {
    // A scalar value (e.g. a string or integer or user defined type)
    Value,
    // A series of zero or more values whose type is described by the nested `String` (e.g. a list)
    Sequence(String),
    // A collection of field name/value pairs (e.g. a map)
    Struct,
}

impl Display for AbstractDataType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                AbstractDataType::Value => "scalar value struct",
                AbstractDataType::Sequence(_) => "sequence",
                AbstractDataType::Struct => "struct",
            }
        )
    }
}
