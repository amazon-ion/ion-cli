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

/// Represents an abstract data type type that can be used to determine which templates can be used for code generation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum AbstractDataType {
    // a struct with a scalar value (used for `type` constraint)
    Value,
    // a struct with a sequence/collection value (used for `element` constraint)
    // the parameter string represents the data type of the sequence
    Sequence(String),
    Struct,
}

impl Display for AbstractDataType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                AbstractDataType::Value => "single value struct",
                AbstractDataType::Sequence(_) => "sequence value struct",
                AbstractDataType::Struct => "struct",
            }
        )
    }
}
