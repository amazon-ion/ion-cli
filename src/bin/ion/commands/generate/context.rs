use crate::commands::generate::model::DataModelNode;
use serde::Serialize;

/// Represents a context that will be used for code generation
pub struct CodeGenContext {
    // Represents the nested types for the current abstract data type
    pub(crate) nested_types: Vec<DataModelNode>,
}

impl CodeGenContext {
    pub fn new() -> Self {
        Self {
            nested_types: vec![],
        }
    }
}

/// Represents a sequenced type which could either be a list or s-expression.
/// This is used by `AbstractDataType` to represent sequence type for `Sequence` variant.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[allow(dead_code)]
pub enum SequenceType {
    List,
    SExp,
}
