use ion_schema::isl::isl_type::IslType;
use std::collections::HashMap;
use std::ops::Deref;
// This module contains a data model that the code generator can use to render a template based on the type of the model.
// Currently, this same data model is represented by `AbstractDataType` but it doesn't hold all the information for the template.
// e.g. currently there are different fields in the template that hold this information like fields, target_kind_name, abstract_data_type.
// Also, the current approach doesn't allow having nested sequences in the generated code. Because the `element_type` in `AbstractDataType::Sequence`
// doesn't have information on its nested types' `element_type`. This can be resolved with below defined new data model.
// _Note: This model will eventually use a map (FullQualifiedTypeReference, DataModel) to resolve some the references in container types(sequence or structure)._
// TODO: This is not yet used in the implementation, modify current implementation to use this data model.
use crate::commands::beta::generate::context::SequenceType;
use serde::Serialize;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct AbstractDataType {
    // Represents the fully qualified name for this data model
    name: FullyQualifiedTypeName,
    // Represents doc comment for the generated code
    doc_comment: String,
    // Represents the type of the data model
    // It can be `None` for modules or packages.
    code_gen_type: Option<AbstractDataKind>,
    // Represents the nested types for this data model
    nested_types: Vec<AbstractDataType>,
    // Represents the source ISL type which can be used to get other constraints useful for this type.
    // For example, getting the length of this sequence from `container_length` constraint or getting a `regex` value for string type.
    // This will also be useful for `text` type to verify if this is a `string` or `symbol`.
    source: Option<IslType>,
}

impl AbstractDataType {
    #![allow(dead_code)]
    pub fn is_scalar(&self) -> bool {
        if let Some(code_gen_type) = &self.code_gen_type {
            return matches!(code_gen_type, AbstractDataKind::Scalar);
        }
        false
    }

    pub fn is_sequence(&self) -> bool {
        if let Some(code_gen_type) = &self.code_gen_type {
            return matches!(code_gen_type, AbstractDataKind::Sequence(_));
        }
        false
    }

    pub fn is_structure(&self) -> bool {
        if let Some(code_gen_type) = &self.code_gen_type {
            return matches!(code_gen_type, AbstractDataKind::Structure(_));
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
pub struct FullQualifiedTypeReference {
    // Represents fully qualified name of the type
    // e.g. In Java, `org.example.Foo`
    //      In Rust, `crate::org::example::Foo`
    type_name: FullyQualifiedTypeName,
    // For types with parameters this will represent the nested parameter
    parameters: Option<Box<FullQualifiedTypeReference>>,
}

/// A target-language-agnostic data type that determines which template(s) to use for code generation.
#[allow(dead_code)]
// TODO: Add more code gent types like sum/discriminated union, enum and map.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum AbstractDataKind {
    // Represents a scalar value (e.g. a string or integer or user defined type)
    // e.g. Given below ISL,
    // ```
    // type::{
    //   name: scalar_type,
    //   type: int
    // }
    // ```
    // Corresponding abstract type in Rust would look like following:
    // ```
    // struct ScalarType {
    //    value: i64
    // }
    // ```
    Scalar,
    // A series of zero or more values whose type is described by the nested `element_type`
    Sequence(Sequence),
    // A collection of field name/value pairs (e.g. a map)
    Structure(Structure),
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
/// Corresponding abstract type in Rust would look like following:
/// ```
/// struct SequenceType {
///    value: Vec<i64>
/// }
/// ```
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Sequence {
    // Represents the fully qualified name with namespace where each element of vector stores a module name or class/struct name.
    // _Note: that a hashmap with (FullQualifiedTypeReference, DataModel) pairs will be stored in code generator to get information on the element_type name used here._
    element_type: FullQualifiedTypeReference,
    // Represents the type of the sequence which is either `sexp` or `list`.
    sequence_type: SequenceType,
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
/// Corresponding abstract type in Rust would look like following:
/// ```
/// struct StructType {
///    a: i64,
///    b: String,
/// }
/// ```
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Structure {
    // Represents whether the struct has closed fields or not
    is_closed: bool,
    // Represents the fields of the struct i.e. (field_name, field_value) pairs
    // field_value represents the type of the value field as fully qualified name
    // _Note: that a hashmap with (FullQualifiedTypeReference, DataModel) pairs will be stored in code generator to get information on the field_value name used here._
    fields: HashMap<String, FullQualifiedTypeReference>,
}
