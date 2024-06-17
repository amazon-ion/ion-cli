use ion_schema::isl::isl_type::IslType;
use std::collections::HashMap;
// This module contains a data model that the code generator can use to render a template based on the type of the model.
// Currently, this same data model is represented by `AbstractDataType` but it doesn't hold all the information for the template.
// e.g. currently there are different fields in the template that hold this information like fields, target_kind_name, abstract_data_type.
// Also, the current approach doesn't allow having nested sequences in the generated code. Because the `element_type` in `AbstractDataType::Sequence`
// doesn't have information on its nested types' `element_type`. This can be resolved with below defined new data model.
// _Note: This model will eventually use a map (FullQualifiedTypeReference, DataModel) to resolve some the references in container types(sequence or structure)._
// TODO: This is not yet used in the implementation, modify current implementation to use this data model.
use crate::commands::beta::generate::context::SequenceType;
use serde::Serialize;

/// Represent a node in the data model tree of the generated code.
/// Each node in this tree could either be a module/package or a concrete data structure(class, struct, enum etc.).
/// This tree structure will be used by code generator and templates to render the generated code as per given ISL type definition hierarchy.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct DataModelNode {
    // Represents the name of this data model
    // Note: It doesn't point to the fully qualified name. To get fully qualified name use `fully_qualified_name()` from `AbstractDataType`.
    name: String,
    // Represents the type of the data model
    // It can be `None` for modules or packages.
    code_gen_type: Option<AbstractDataType>,
    // Represents the nested types for this data model
    nested_types: Vec<DataModelNode>,
}

impl DataModelNode {
    #![allow(dead_code)]
    pub fn is_scalar(&self) -> bool {
        if let Some(code_gen_type) = &self.code_gen_type {
            return matches!(code_gen_type, AbstractDataType::Scalar(_));
        }
        false
    }

    pub fn is_sequence(&self) -> bool {
        if let Some(code_gen_type) = &self.code_gen_type {
            return matches!(code_gen_type, AbstractDataType::Sequence(_));
        }
        false
    }

    pub fn is_structure(&self) -> bool {
        if let Some(code_gen_type) = &self.code_gen_type {
            return matches!(code_gen_type, AbstractDataType::Structure(_));
        }
        false
    }
}

/// Represents a fully qualified type name for a type definition
/// e.g. For a `Foo` class in `org.example` namespace
///     In Java, `org.example.Foo`
///     In Rust, `org::example::Foo`
type FullyQualifiedTypeName = Vec<String>;

/// Represents a fully qualified type name for a type reference
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FullyQualifiedTypeReference {
    // Represents fully qualified name of the type
    // e.g. In Java, `org.example.Foo`
    //      In Rust, `crate::org::example::Foo`
    type_name: FullyQualifiedTypeName,
    // For types with parameters this will represent the nested parameter
    parameters: Vec<FullyQualifiedTypeReference>,
}

/// A target-language-agnostic data type that determines which template(s) to use for code generation.
#[allow(dead_code)]
// TODO: Add more code gent types like sum/discriminated union, enum and map.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum AbstractDataType {
    // Represents a top level scalar type definition which also has a name attached to it.
    WrappedScalar(WrappedScalar),
    // Represents a nested/inline scalar value (e.g. a string or integer or user defined type)
    Scalar(Scalar),
    // A series of zero or more values whose type is described by the nested `element_type`
    Sequence(Sequence),
    // A collection of field name/value pairs (e.g. a map)
    Structure(Structure),
}

impl AbstractDataType {
    #![allow(dead_code)]
    pub fn doc_comment(&self) -> Option<String> {
        match self {
            AbstractDataType::WrappedScalar(WrappedScalar { doc_comment, .. }) => {
                doc_comment.to_owned()
            }
            AbstractDataType::Scalar(Scalar { doc_comment, .. }) => doc_comment.to_owned(),
            AbstractDataType::Sequence(Sequence { doc_comment, .. }) => {
                Some(doc_comment.to_string())
            }
            AbstractDataType::Structure(Structure { doc_comment, .. }) => {
                Some(doc_comment.to_string())
            }
        }
    }

    pub fn fully_qualified_name(&self) -> FullyQualifiedTypeName {
        match self {
            AbstractDataType::WrappedScalar(w) => w.fully_qualified_type_name().to_owned(),
            AbstractDataType::Scalar(s) => s.name.to_owned(),
            AbstractDataType::Sequence(seq) => seq.name.to_owned(),
            AbstractDataType::Structure(structure) => structure.name.to_owned(),
        }
    }
}

/// Represents a nested/inline scalar type (e.g. a string or integer or user defined type)
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Scalar {
    // Represents the fully qualified name for this data model
    // For any nested/inline scalar type this would be the name of that type.
    // e.g.
    // ```
    // type::{
    //    name: foo,
    //    type: list,
    //    element: string // this is a nested scalar type
    // }
    // ```
    // Corresponding `FullyQualifiedName` would be `vec!["String"]` and `scalar_type` would be `None`.
    name: FullyQualifiedTypeName,
    // Represents doc comment for the generated code
    // If this is a top level scalar type definition then this will have `Some(doc_comment)`,
    // Otherwise this is `None`.
    doc_comment: Option<String>,
    // Represents the source ISL type which can be used to get other constraints useful for this type.
    // For example, getting the length of this sequence from `container_length` constraint or getting a `regex` value for string type.
    // This will also be useful for `text` type to verify if this is a `string` or `symbol`.
    // TODO: `IslType` does not implement `Serialize`, define a custom implementation or define methods on this field that returns values which could be serialized.
    #[serde(skip_serializing)]
    source: IslType,
}

/// Represents a top level scalar type definition
/// e.g. Given below ISL,
/// ```
/// type::{
///   name: scalar_type,
///   type: int
/// }
/// ```
/// Corresponding generated code in Rust would look like following:
/// ```
/// struct ScalarType {
///    value: i64
/// }
/// ```
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WrappedScalar {
    // Represents the fully qualified name of this top level scalar type
    // e.g. Given below ISL,
    // ```
    // type::{
    //   name: foo,
    //   type: string
    // }
    // ```
    // Corresponding `FullyQualifiedTypeReference` would be as following:
    // ```
    // FullyQualifiedTypeReference {
    //    type_name: vec!["Foo"], // name of the top level scalar type
    //    parameters: vec![FullyQualifiedTypeReference {type_name: vec!["String"] }] // nested type name for the scalar value
    // }
    // ```
    name: FullyQualifiedTypeReference,
    // Represents the scalar type
    // Represents doc comment for the generated code
    // If this is a top level scalar type definition then this will have `Some(doc_comment)`,
    // Otherwise this is `None`.
    doc_comment: Option<String>,
    // Represents the source ISL type which can be used to get other constraints useful for this type.
    // For example, getting the length of this sequence from `container_length` constraint or getting a `regex` value for string type.
    // This will also be useful for `text` type to verify if this is a `string` or `symbol`.
    // TODO: `IslType` does not implement `Serialize`, define a custom implementation or define methods on this field that returns values which could be serialized.
    #[serde(skip_serializing)]
    source: IslType,
}

impl WrappedScalar {
    pub fn fully_qualified_type_name(&self) -> FullyQualifiedTypeName {
        self.name.type_name.to_owned()
    }
}

/// Represents series of zero or more values whose type is described by the nested `element_type`
/// and sequence type is described by nested `sequence_type` (e.g. List or SExp).
/// If there is no `element` constraint present in schema type then `element_type` will be None.
/// If there is no `type` constraint present in schema type then `sequence_type` will be None.
/// e.g. Given below ISL,
/// ```
/// type::{
///   name: sequence_type,
///   element: int
/// }
/// ```
/// Corresponding generated code in Rust would look like following:
/// ```
/// struct SequenceType {
///    value: Vec<i64>
/// }
/// ```
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Sequence {
    // Represents the fully qualified name for this data model
    name: FullyQualifiedTypeName,
    // Represents doc comment for the generated code
    doc_comment: String,
    // Represents the fully qualified name with namespace where each element of vector stores a module name or class/struct name.
    // _Note: that a hashmap with (FullQualifiedTypeReference, DataModel) pairs will be stored in code generator to get information on the element_type name used here._
    element_type: FullyQualifiedTypeReference,
    // Represents the type of the sequence which is either `sexp` or `list`.
    sequence_type: SequenceType,
    // Represents the source ISL type which can be used to get other constraints useful for this type.
    // For example, getting the length of this sequence from `container_length` constraint or getting a `regex` value for string type.
    // This will also be useful for `text` type to verify if this is a `string` or `symbol`.
    // TODO: `IslType` does not implement `Serialize`, define a custom implementation or define methods on this field that returns values which could be serialized.
    #[serde(skip_serializing)]
    source: IslType,
}

/// Represents a collection of field name/value pairs (e.g. a map)
/// e.g. Given below ISL,
/// ```
/// type::{
///   name: struct_type,
///   fields: {
///      a: int,
///      b: string,
///   }
/// }
/// ```
/// Corresponding generated code in Rust would look like following:
/// ```
/// struct StructType {
///    a: i64,
///    b: String,
/// }
/// ```
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Structure {
    // Represents the fully qualified name for this data model
    name: FullyQualifiedTypeName,
    // Represents doc comment for the generated code
    doc_comment: String,
    // Represents whether the struct has closed fields or not
    is_closed: bool,
    // Represents the fields of the struct i.e. (field_name, field_value) pairs
    // field_value represents the type of the value field as fully qualified name
    // _Note: that a hashmap with (FullQualifiedTypeReference, DataModel) pairs will be stored in code generator to get information on the field_value name used here._
    fields: HashMap<String, FullyQualifiedTypeReference>,
    // Represents the source ISL type which can be used to get other constraints useful for this type.
    // For example, getting the length of this sequence from `container_length` constraint or getting a `regex` value for string type.
    // This will also be useful for `text` type to verify if this is a `string` or `symbol`.
    // TODO: `IslType` does not implement `Serialize`, define a custom implementation or define methods on this field that returns values which could be serialized.
    #[serde(skip_serializing)]
    source: IslType,
}
