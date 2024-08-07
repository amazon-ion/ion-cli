use crate::commands::generate::model::{
    AbstractDataType, DataModelNode, FullyQualifiedTypeReference,
};
use serde::Serialize;

/// Represents a context that will be used for code generation
pub struct CodeGenContext {
    // Initially the abstract_data_type field is set to None.
    // Once an ISL type definition is mapped to an abstract data type this will have Some value.
    pub(crate) data_model_node: Option<DataModelNode>,
}

impl CodeGenContext {
    pub fn new() -> Self {
        Self {
            data_model_node: None,
        }
    }

    pub fn with_data_model_node(&mut self, data_model_node: DataModelNode) {
        self.data_model_node = Some(data_model_node);
    }

    pub fn with_abstract_data_type(&mut self, abstract_data_type: AbstractDataType) {
        if let Some(ref mut data_model_node) = self.data_model_node {
            data_model_node.with_abstract_data_type(abstract_data_type);
        }
    }

    pub fn with_nested_type(&mut self, nested_type: DataModelNode) {
        if let Some(ref mut data_model_node) = self.data_model_node {
            data_model_node.with_nested_type(nested_type);
        }
    }

    pub fn with_element_type(&mut self, element_type: Option<FullyQualifiedTypeReference>) {
        if let Some(ref mut data_model_node) = self.data_model_node {
            data_model_node.with_element_type(element_type);
        }
    }
}

/// Represents a sequenced type which could either be a list or s-expression.
/// This is used by `AbstractDataType` to represent sequence type for `Sequence` variant.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum SequenceType {
    List,
    SExp,
}
