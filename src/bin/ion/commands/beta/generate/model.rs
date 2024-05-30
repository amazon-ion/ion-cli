// This module contains a data model that code generator cna use to render a template based on the type of the model.
// Currently, this same data model represented is by `AbstractDataType` but it doesn't hold all the information for the template.
// e.g. currently there are different fields in template that hold this information like fields, target_kind_name, abstract_data_type.
// Also, current approach doesn't allow having nested sequences in the generated code. Because the `element_type` in `AbstractDataType::Sequence`
// doesn't have information on its nested types' `element_type`. This can be resolved with below defined new data model.
// TODO: This is not yet used in the implementation, modify current implementation to use this data model.
use crate::commands::beta::generate::context::SequenceType;
use serde::Serialize;

/// Represents the data model that will be used by code generator to render templates
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DataModel {
    // Represents the name of the target data model
    // e.g. For an ISL as below:
    // ```
    // type::{
    //      name: struct_type,
    //      fields:{
    //          foo: String,
    //          bar: int
    //      }
    // }
    // ```
    // The corresponding target_kind_name in Java and Rust would be `StructType`
    // This property can be `None` if:
    // - It is a nested sequence type (e.g. `{ type: list, element: int }`)
    // - It is a nested scalar type (e.g. `{ type: int }`)
    // For all other cases including nested struct, there will always be a name associated for the target data model.
    // _Note: For nested struct, currently code generator creates a name based on a counter `NestedX`._
    target_kind_name: Option<String>,
    // Represents the code gene type which could be value, structure or sequence.
    // It holds the information for the model based on the type.
    code_gen_type: Box<CodeGenType>,
    // Represents the nested types for this data model
    nested_types: Vec<DataModel>,
}

/// A target-language-agnostic data type that determines which template(s) to use for code generation.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum CodeGenType {
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
    Value {
        isl_type_name: String,
        value_type: String,
    },
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
        element_type: Option<Box<DataModel>>,
        sequence_type: Option<SequenceType>,
    },
    // A collection of field name/value pairs (e.g. a map)
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
    Structure {
        // Represents whether the struct has closed fields or not
        is_closed: bool,
        // Represents the fields of the struct
        fields: Vec<Field>,
    },
}

/// Represents a field in struct type
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Field {
    name: String,          // field name
    value_type: DataModel, // the target data model
    isl_type_name: String, // ISL type name for field's data model
}
