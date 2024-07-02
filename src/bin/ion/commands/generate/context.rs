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
    // e.g. Given below ISL,
    // ```
    // type::{
    //   name: value_type,
    //   type: int
    // }
    // ```
    // Corresponding abstract type in Rust would look like following:
    // ```
    // struct ValueType {
    //    value: i64
    // }
    // ```
    Value,
    // A series of zero or more values whose type is described by the nested `element_type`
    // and sequence type is described by nested `sequence_type` (e.g. List or SExp).
    // If there is no `element` constraint present in schema type then `element_type` will be None.
    // If there is no `type` constraint present in schema type then `sequence_type` will be None.
    // e.g. Given below ISL,
    // ```
    // type::{
    //   name: sequence_type,
    //   element: int
    // }
    // ```
    // Corresponding abstract type in Rust would look like following:
    // ```
    // struct SequenceType {
    //    value: Vec<i64>
    // }
    // ```
    Sequence {
        element_type: Option<String>,
        sequence_type: Option<SequenceType>,
    },
    // A collection of field name/value pairs (e.g. a map)
    // the nested boolean represents whether the struct has closed fields or not
    // e.g. Given below ISL,
    // ```
    // type::{
    //   name: struct_type,
    //   fields: {
    //      a: int,
    //      b: string,
    //   }
    // }
    // ```
    // Corresponding abstract type in Rust would look like following:
    // ```
    // struct StructType {
    //    a: i64,
    //    b: String,
    // }
    // ```
    Structure(bool),
}

impl AbstractDataType {
    pub fn element_type(&self) -> Option<String> {
        match self {
            AbstractDataType::Sequence { element_type, .. } => element_type.to_owned(),
            _ => None,
        }
    }

    pub fn sequence_type(&self) -> Option<SequenceType> {
        match self {
            AbstractDataType::Sequence { sequence_type, .. } => sequence_type.to_owned(),
            _ => None,
        }
    }

    pub fn is_content_closed(&self) -> Option<bool> {
        match self {
            AbstractDataType::Structure(content_closed) => Some(*content_closed),
            _ => None,
        }
    }
}

impl Display for AbstractDataType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                AbstractDataType::Value => "single value struct",
                AbstractDataType::Sequence { .. } => "sequence value struct",
                AbstractDataType::Structure(_) => "struct",
            }
        )
    }
}

/// Represents a sequenced type which could either be a list or s-expression.
/// This is used by `AbstractDataType` to represent sequence type for `Sequence` variant.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum SequenceType {
    List,
    SExp,
}
