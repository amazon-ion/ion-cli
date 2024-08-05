use ion_schema::isl::isl_type::IslType;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
// This module contains a data model that the code generator can use to render a template based on the type of the model.
// Currently, this same data model is represented by `AbstractDataType` but it doesn't hold all the information for the template.
// e.g. currently there are different fields in the template that hold this information like fields, target_kind_name, abstract_data_type.
// Also, the current approach doesn't allow having nested sequences in the generated code. Because the `element_type` in `AbstractDataType::Sequence`
// doesn't have information on its nested types' `element_type`. This can be resolved with below defined new data model.
// _Note: This model will eventually use a map (FullQualifiedTypeReference, DataModel) to resolve some the references in container types(sequence or structure)._
// TODO: This is not yet used in the implementation, modify current implementation to use this data model.
use crate::commands::generate::context::SequenceType;
use serde::Serialize;
use serde_json::Value;

/// Represent a node in the data model tree of the generated code.
/// Each node in this tree could either be a module/package or a concrete data structure(class, struct, enum etc.).
/// This tree structure will be used by code generator and templates to render the generated code as per given ISL type definition hierarchy.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DataModelNode {
    // Represents the name of this data model
    // Note: It doesn't point to the fully qualified name. To get fully qualified name use `fully_qualified_name()` from `AbstractDataType`.
    pub(crate) name: String,
    // Represents the type of the data model
    // It can be `None` for modules or packages.
    pub(crate) code_gen_type: Option<AbstractDataType>,
    // Represents the nested types for this data model
    pub(crate) nested_types: Vec<DataModelNode>,
}

impl DataModelNode {
    #[allow(dead_code)]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[allow(dead_code)]
    pub fn is_scalar(&self) -> bool {
        if let Some(code_gen_type) = &self.code_gen_type {
            return matches!(code_gen_type, AbstractDataType::Scalar(_));
        }
        false
    }

    #[allow(dead_code)]
    pub fn is_sequence(&self) -> bool {
        if let Some(code_gen_type) = &self.code_gen_type {
            return matches!(code_gen_type, AbstractDataType::Sequence(_));
        }
        false
    }

    #[allow(dead_code)]
    pub fn is_structure(&self) -> bool {
        if let Some(code_gen_type) = &self.code_gen_type {
            return matches!(code_gen_type, AbstractDataType::Structure(_));
        }
        false
    }

    pub fn with_abstract_data_type(&mut self, code_gen_type: AbstractDataType) {
        self.code_gen_type = Some(code_gen_type)
    }

    pub fn with_nested_type(&mut self, nested_type: DataModelNode) {
        self.nested_types.push(nested_type)
    }

    pub fn with_element_type(&mut self, element_type: Option<FullyQualifiedTypeReference>) {
        if let Some(ref mut code_gen_type) = self.code_gen_type {
            code_gen_type.with_element_type(element_type);
        }
    }
}

/// Represents a fully qualified type name for a type definition
/// e.g. For a `Foo` class in `org.example` namespace
///     In Java, `org.example.Foo`
///     In Rust, `org::example::Foo`
type FullyQualifiedTypeName = Vec<String>;

/// Represents a fully qualified type name for a type reference
#[derive(Debug, Clone, PartialEq, Serialize, Hash, Eq)]
pub struct FullyQualifiedTypeReference {
    // Represents fully qualified name of the type
    // e.g. In Java, `org.example.Foo`
    //      In Rust, `crate::org::example::Foo`
    pub(crate) type_name: FullyQualifiedTypeName,
    // For types with parameters this will represent the nested parameters
    pub(crate) parameters: Vec<FullyQualifiedTypeReference>,
}

impl From<FullyQualifiedTypeName> for FullyQualifiedTypeReference {
    fn from(value: FullyQualifiedTypeName) -> Self {
        Self {
            type_name: value,
            parameters: vec![],
        }
    }
}

impl Display for FullyQualifiedTypeReference {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.parameters.is_empty() {
            return write!(f, "{}", self.type_name.join("."));
        }
        write!(f, "{}<", self.type_name.join("."))?;

        for (i, parameter) in self.parameters.iter().enumerate() {
            if i == self.parameters.len() - 1 {
                write!(f, "{}", parameter)?;
            } else {
                write!(f, "{},", parameter)?;
            }
        }
        write!(f, ">")
    }
}

// This is useful for code generator to convert input `serde_json::Value` coming from tera(template engine) into `FullyQualifiedTypeReference`
impl TryFrom<&Value> for FullyQualifiedTypeReference {
    type Error = tera::Error;

    fn try_from(v: &Value) -> Result<Self, Self::Error> {
        let obj = v.as_object().ok_or(tera::Error::msg(
            "Tera value can not be converted to an object",
        ))?;
        let mut type_name = vec![];
        let mut parameters: Vec<FullyQualifiedTypeReference> = vec![];
        for (key, value) in obj {
            if key == "type_name" {
                type_name = value
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|s| s.as_str().unwrap().to_string())
                    .collect();
            } else {
                let parameters_result: Result<Vec<FullyQualifiedTypeReference>, tera::Error> =
                    value
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.try_into())
                        .collect();
                parameters = parameters_result?;
            }
        }
        Ok(FullyQualifiedTypeReference {
            type_name,
            parameters,
        })
    }
}

impl FullyQualifiedTypeReference {
    pub fn with_parameters(&mut self, parameters: Vec<FullyQualifiedTypeReference>) {
        self.parameters = parameters;
    }
}

/// A target-language-agnostic data type that determines which template(s) to use for code generation.
// TODO: Add more code gen types like sum/discriminated union, enum and map.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum AbstractDataType {
    // Represents a scalar type which also has a name attached to it and is nominally distinct from its base type.
    WrappedScalar(WrappedScalar),
    // Represents a scalar value (e.g. a string or integer or user defined type)
    Scalar(Scalar),
    // Represents a sequence type which also has a name attached to it and is nominally distinct from its base type.
    WrappedSequence(WrappedSequence),
    // A series of zero or more values whose type is described by the nested `element_type`
    Sequence(Sequence),
    // A collection of field name/value pairs (e.g. a map)
    Structure(Structure),
}

impl AbstractDataType {
    #[allow(dead_code)]
    pub fn doc_comment(&self) -> Option<&str> {
        match self {
            AbstractDataType::WrappedScalar(WrappedScalar { doc_comment, .. }) => {
                doc_comment.as_ref().map(|s| s.as_str())
            }
            AbstractDataType::Scalar(Scalar { doc_comment, .. }) => {
                doc_comment.as_ref().map(|s| s.as_str())
            }
            AbstractDataType::Sequence(Sequence { doc_comment, .. }) => {
                doc_comment.as_ref().map(|s| s.as_str())
            }
            AbstractDataType::WrappedSequence(WrappedSequence { doc_comment, .. }) => {
                doc_comment.as_ref().map(|s| s.as_str())
            }
            AbstractDataType::Structure(Structure { doc_comment, .. }) => {
                doc_comment.as_ref().map(|s| s.as_str())
            }
        }
    }

    pub fn fully_qualified_type_ref(&self) -> Option<FullyQualifiedTypeReference> {
        match self {
            AbstractDataType::WrappedScalar(w) => {
                Some(w.fully_qualified_type_name().to_owned().into())
            }
            AbstractDataType::Scalar(s) => Some(s.name.to_owned().into()),
            AbstractDataType::Sequence(seq) => seq.element_type.to_owned(),
            AbstractDataType::WrappedSequence(wrapped_seq) => {
                Some(wrapped_seq.fully_qualified_type_name().to_owned().into())
            }
            AbstractDataType::Structure(structure) => Some(structure.name.to_owned().into()),
        }
    }

    pub fn with_element_type(&mut self, element_type: Option<FullyQualifiedTypeReference>) {
        match self {
            AbstractDataType::WrappedSequence(ref mut wrapped_seq) => {
                wrapped_seq.with_element_type(element_type);
            }
            AbstractDataType::Sequence(ref mut seq) => {
                seq.with_element_type(element_type);
            }
            _ => {}
        }
    }
}

/// Represents a scalar type (e.g. a string or integer or user defined type)
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
    // Corresponding `FullyQualifiedName` would be `vec!["String"]`.
    pub(crate) name: FullyQualifiedTypeName,
    // Represents doc comment for the generated code
    // If the doc comment is provided for this scalar type then this is `Some(doc_comment)`, other it is None.
    pub(crate) doc_comment: Option<String>,
    // Represents the source ISL type which can be used to get other constraints useful for this type.
    // For example, getting the length of this sequence from `container_length` constraint or getting a `regex` value for string type.
    // This will also be useful for `text` type to verify if this is a `string` or `symbol`.
    // TODO: `IslType` does not implement `Serialize`, define a custom implementation or define methods on this field that returns values which could be serialized.
    #[serde(skip_serializing)]
    pub(crate) source: IslType,
}

impl Scalar {
    pub fn with_type(&mut self, type_name: FullyQualifiedTypeReference) {
        self.name = type_name.type_name;
    }
}

/// Represents a scalar type which also has a name attached to it and is nominally distinct from its base type.
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
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WrappedScalar {
    // Represents the fully qualified name of this wrapped scalar type
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
    //    type_name: vec!["Foo"], // name of the wrapped scalar type
    //    parameters: vec![FullyQualifiedTypeReference {type_name: vec!["String"] }] // base type name for the scalar value
    // }
    // ```
    pub(crate) name: FullyQualifiedTypeReference,
    // Represents doc comment for the generated code
    // If the doc comment is provided for this scalar type then this is `Some(doc_comment)`, other it is None.
    pub(crate) doc_comment: Option<String>,
    // Represents the source ISL type which can be used to get other constraints useful for this type.
    // For example, getting the length of this sequence from `container_length` constraint or getting a `regex` value for string type.
    // This will also be useful for `text` type to verify if this is a `string` or `symbol`.
    // TODO: `IslType` does not implement `Serialize`, define a custom implementation or define methods on this field that returns values which could be serialized.
    #[serde(skip_serializing)]
    pub(crate) source: IslType,
}

impl WrappedScalar {
    pub fn fully_qualified_type_name(&self) -> &FullyQualifiedTypeName {
        &self.name.type_name
    }

    pub fn with_type(&mut self, type_name: FullyQualifiedTypeReference) {
        self.name.with_parameters(vec![type_name])
    }

    #[allow(dead_code)]
    pub fn scalar_type(&self) -> &str {
        &self.name.parameters[0].type_name[0]
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
///   element: int,
///   type: list
/// }
/// ```
/// Corresponding generated code in Rust would look like following:
/// ```
/// struct SequenceType {
///    value: Vec<i64>
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Sequence {
    // Represents the fully qualified name for this data model
    pub(crate) name: FullyQualifiedTypeName,
    // Represents doc comment for the generated code if it is provided by user
    pub(crate) doc_comment: Option<String>,
    // Represents the fully qualified name with namespace where each element of vector stores a module name or class/struct name.
    // _Note: that a hashmap with (FullQualifiedTypeReference, DataModel) pairs will be stored in code generator to get information on the element_type name used here._
    pub(crate) element_type: Option<FullyQualifiedTypeReference>,
    // Represents the type of the sequence which is either `sexp` or `list`.
    pub(crate) sequence_type: Option<SequenceType>,
    // Represents the source ISL type which can be used to get other constraints useful for this type.
    // For example, getting the length of this sequence from `container_length` constraint or getting a `regex` value for string type.
    // This will also be useful for `text` type to verify if this is a `string` or `symbol`.
    // TODO: `IslType` does not implement `Serialize`, define a custom implementation or define methods on this field that returns values which could be serialized.
    #[serde(skip_serializing)]
    pub(crate) source: IslType,
}

impl Sequence {
    pub fn with_element_type(&mut self, element_type: Option<FullyQualifiedTypeReference>) {
        self.element_type = element_type;
    }

    pub fn with_sequence_type(&mut self, sequence_type: SequenceType) {
        self.sequence_type = Some(sequence_type);
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
///   element: int,
///   type: list
/// }
/// ```
/// Corresponding generated code in Rust would look like following:
/// ```
/// struct SequenceType {
///    value: Vec<i64>
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WrappedSequence {
    // Represents the fully qualified name for this data model
    pub(crate) name: FullyQualifiedTypeReference,
    // Represents doc comment for the generated code
    pub(crate) doc_comment: Option<String>,
    // Represents the fully qualified name with namespace where each element of vector stores a module name or class/struct name.
    // _Note: that a hashmap with (FullQualifiedTypeReference, DataModel) pairs will be stored in code generator to get information on the element_type name used here._
    pub(crate) element_type: Option<FullyQualifiedTypeReference>,
    // Represents the type of the sequence which is either `sexp` or `list`.
    pub(crate) sequence_type: Option<SequenceType>,
    // Represents the source ISL type which can be used to get other constraints useful for this type.
    // For example, getting the length of this sequence from `container_length` constraint or getting a `regex` value for string type.
    // This will also be useful for `text` type to verify if this is a `string` or `symbol`.
    // TODO: `IslType` does not implement `Serialize`, define a custom implementation or define methods on this field that returns values which could be serialized.
    #[serde(skip_serializing)]
    pub(crate) source: IslType,
}

impl WrappedSequence {
    pub fn fully_qualified_type_name(&self) -> &FullyQualifiedTypeName {
        &self.name.type_name
    }

    pub fn with_element_type(&mut self, element_type: Option<FullyQualifiedTypeReference>) {
        self.element_type = element_type;
    }

    pub fn with_sequence_type(&mut self, sequence_type: SequenceType) {
        self.sequence_type = Some(sequence_type);
    }
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
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Structure {
    // Represents the fully qualified name for this data model
    pub(crate) name: FullyQualifiedTypeName,
    // Represents doc comment for the generated code
    pub(crate) doc_comment: Option<String>,
    // Represents whether the struct has closed fields or not
    pub(crate) is_closed: bool,
    // Represents the fields of the struct i.e. (field_name, field_value) pairs
    // field_value represents `FieldReference` i.e. the type of the value field as fully qualified name and the presence for this field.
    // _Note: that a hashmap with (FullQualifiedTypeReference, DataModel) pairs will be stored in code generator to get information on the field_value name used here._
    // Currently code gen does not support open-ended types, hence when no fields are specified for a struct this will be set to None.
    // Generator will use this information to throw an error when no fields are specified.
    pub(crate) fields: Option<HashMap<String, FieldReference>>,
    // Represents the source ISL type which can be used to get other constraints useful for this type.
    // For example, getting the length of this sequence from `container_length` constraint or getting a `regex` value for string type.
    // This will also be useful for `text` type to verify if this is a `string` or `symbol`.
    // TODO: `IslType` does not implement `Serialize`, define a custom implementation or define methods on this field that returns values which could be serialized.
    #[serde(skip_serializing)]
    pub(crate) source: IslType,
}

impl Structure {
    pub fn with_fields(&mut self, fields: HashMap<String, FieldReference>) {
        self.fields = Some(fields);
    }

    pub fn with_open_fields(&mut self) {
        self.is_closed = false;
    }
}

/// Represents whether the field is required or not
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum FieldPresence {
    #[allow(dead_code)]
    Required,
    Optional,
}

/// Represents a reference to the field with its fully qualified name and its presence (i.e. required or optional)
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FieldReference(
    pub(crate) FullyQualifiedTypeReference,
    pub(crate) FieldPresence,
);
