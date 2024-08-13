use crate::commands::generate::model::{
    AbstractDataType, DataModelNode, FullyQualifiedTypeReference,
};
use serde::Serialize;

/// Represents a context that will be used for code generation
pub struct CodeGenContext {
    // Initially the data_model_node field is set to None.
    // Once an ISL type definition is mapped to an abstract data type this will have `Some` value.
    // This data_model_node represents the determined data model for current ISL type definition.
    pub(crate) data_model_node: Option<DataModelNode>,
}

impl CodeGenContext {
    pub fn new() -> Self {
        Self {
            data_model_node: None,
        }
    }

    /// Sets given data model node as current determined data model node for given ISL type definition.
    pub fn with_data_model_node(&mut self, data_model_node: DataModelNode) {
        self.data_model_node = Some(data_model_node);
    }

    /// Sets given data model node's abstract data type for given ISL type definition, if the current data model node has `Some` value;
    /// otherwise does nothing.
    pub fn with_abstract_data_type(&mut self, abstract_data_type: AbstractDataType) {
        if let Some(ref mut data_model_node) = self.data_model_node {
            data_model_node.with_abstract_data_type(abstract_data_type);
        }
    }

    /// Adds given nested type to the current data model node, if the current data model node has `Some` value;
    /// otherwise does nothing.
    pub fn with_nested_type(&mut self, nested_type: DataModelNode) {
        if let Some(ref mut data_model_node) = self.data_model_node {
            data_model_node.with_nested_type(nested_type);
        }
    }

    /// Sets element type the abstract data type if it is of `Sequence` type and if the current data model node has `Some` value;
    /// otherwise does nothing.
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
