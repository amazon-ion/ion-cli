use crate::commands::generate::model::{AbstractDataTypeBuilder, DataModelNode};
use serde::Serialize;

/// Represents a context that will be used for code generation
pub struct CodeGenContext {
    // Represents the current abstract data type builder
    // Initially this will be set to None, once a constraint is found in the type definition this will be updated accordingly.
    pub(crate) current_abstract_data_type_builder: Option<AbstractDataTypeBuilder>,
    // Represents the nested types for the current abstract data type
    pub(crate) nested_types: Vec<DataModelNode>,
}

impl CodeGenContext {
    pub fn new() -> Self {
        Self {
            current_abstract_data_type_builder: None,
            nested_types: vec![],
        }
    }

    pub fn with_abstract_data_type_builder(&mut self, builder: AbstractDataTypeBuilder) {
        self.current_abstract_data_type_builder = Some(builder);
    }
}

/// Represents a sequenced type which could either be a list or s-expression.
/// This is used by `AbstractDataType` to represent sequence type for `Sequence` variant.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum SequenceType {
    List,
    SExp,
}
