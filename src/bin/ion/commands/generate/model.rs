use derive_builder::Builder;
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
    // e.g. For a given schema as below:
    // ```
    //  type::{
    //    name: foo,
    //    type: struct,
    //    fields: {
    //      a: int,
    //      b: string
    //    }
    //  }
    // ```
    // The name of the abstract data type would be `Foo` where `Foo` will represent a Java class or Rust struct.
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

    pub fn fully_qualified_type_ref(&mut self) -> Option<FullyQualifiedTypeReference> {
        self.code_gen_type
            .as_ref()
            .and_then(|t| t.fully_qualified_type_ref())
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    WrappedScalar(WrappedScalar),
    // Represents a scalar value (e.g. a string or integer or user defined type)
    #[allow(dead_code)]
    Scalar(Scalar),
    // A series of zero or more values whose type is described by the nested `element_type`
    #[allow(dead_code)]
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
            AbstractDataType::Sequence(Sequence { doc_comment, .. }) => Some(doc_comment.as_str()),
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
            AbstractDataType::Scalar(s) => Some(s.base_type.to_owned()),
            AbstractDataType::Sequence(seq) => Some(seq.element_type.to_owned()),
            AbstractDataType::Structure(structure) => Some(structure.name.to_owned().into()),
        }
    }
}

/// Represents a scalar type (e.g. a string or integer or user defined type)
#[allow(dead_code)]
#[derive(Debug, Clone, Builder, PartialEq, Serialize)]
#[builder(setter(into))]
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
    // Corresponding `FullyQualifiedReference` would be `FullyQualifiedTypeReference { type_name: vec!["String"], parameters: vec![] }`.
    base_type: FullyQualifiedTypeReference,
    // Represents doc comment for the generated code
    // If the doc comment is provided for this scalar type then this is `Some(doc_comment)`, other it is None.
    #[builder(default)]
    doc_comment: Option<String>,
    // Represents the source ISL type which can be used to get other constraints useful for this type.
    // For example, getting the length of this sequence from `container_length` constraint or getting a `regex` value for string type.
    // This will also be useful for `text` type to verify if this is a `string` or `symbol`.
    // TODO: `IslType` does not implement `Serialize`, define a custom implementation or define methods on this field that returns values which could be serialized.
    #[serde(skip_serializing)]
    source: IslType,
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
#[allow(dead_code)]
#[derive(Debug, Clone, Builder, PartialEq, Serialize)]
#[builder(setter(into))]
pub struct WrappedScalar {
    // Represents the fully qualified name of this wrapped scalar type
    // e.g. Given below ISL,
    // ```
    // type::{
    //   name: foo,
    //   type: string
    // }
    // ```
    // Corresponding `name` would be `vec!["Foo"]` and `base_type` would be `FullyQualifiedTypeReference { type_name: vec!["String"], parameters: vec![] }`.
    name: FullyQualifiedTypeName,
    base_type: FullyQualifiedTypeReference,
    // Represents doc comment for the generated code
    // If the doc comment is provided for this scalar type then this is `Some(doc_comment)`, other it is None.
    #[builder(default)]
    doc_comment: Option<String>,
    // Represents the source ISL type which can be used to get other constraints useful for this type.
    // For example, getting the length of this sequence from `container_length` constraint or getting a `regex` value for string type.
    // This will also be useful for `text` type to verify if this is a `string` or `symbol`.
    // TODO: `IslType` does not implement `Serialize`, define a custom implementation or define methods on this field that returns values which could be serialized.
    #[serde(skip_serializing)]
    source: IslType,
}

impl WrappedScalar {
    pub fn fully_qualified_type_name(&self) -> &FullyQualifiedTypeName {
        &self.name
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
#[allow(dead_code)]
#[derive(Debug, Clone, Builder, PartialEq, Serialize)]
#[builder(setter(into))]
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
#[derive(Debug, Clone, Builder, PartialEq, Serialize)]
#[builder(setter(into))]
pub struct Structure {
    // Represents the fully qualified name for this data model
    pub(crate) name: FullyQualifiedTypeName,
    // Represents doc comment for the generated code
    #[builder(default)]
    pub(crate) doc_comment: Option<String>,
    // Represents whether the struct has closed fields or not
    pub(crate) is_closed: bool,
    // Represents the fields of the struct i.e. (field_name, field_value) pairs
    // field_value represents `FieldReference` i.e. the type of the value field as fully qualified name and the presence for this field.
    // _Note: that a hashmap with (FullQualifiedTypeReference, DataModel) pairs will be stored in code generator to get information on the field_value name used here._
    pub(crate) fields: HashMap<String, FieldReference>,
    // Represents the source ISL type which can be used to get other constraints useful for this type.
    // For example, getting the length of this sequence from `container_length` constraint or getting a `regex` value for string type.
    // This will also be useful for `text` type to verify if this is a `string` or `symbol`.
    // TODO: `IslType` does not implement `Serialize`, define a custom implementation or define methods on this field that returns values which could be serialized.
    #[serde(skip_serializing)]
    pub(crate) source: IslType,
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

#[cfg(test)]
mod model_tests {
    use super::*;
    use ion_schema::isl::isl_constraint::v_2_0::*;
    use ion_schema::isl::isl_type::v_2_0::anonymous_type;
    use ion_schema::isl::isl_type_reference::v_2_0::*;
    use ion_schema::isl::ranges::UsizeRange;

    #[test]
    fn scalar_builder_test() {
        let expected_scalar = Scalar {
            base_type: FullyQualifiedTypeReference {
                type_name: vec!["String".to_string()],
                parameters: vec![],
            },
            doc_comment: Some("This is scalar type".to_string()),
            source: anonymous_type(vec![type_constraint(named_type_ref("string"))]),
        };

        let mut scalar_builder = ScalarBuilder::default();

        // sets all the information about the scalar type
        scalar_builder
            .base_type(vec!["String".to_string()])
            .doc_comment(Some("This is scalar type".to_string()))
            .source(anonymous_type(vec![type_constraint(named_type_ref(
                "string",
            ))]));

        // Verify the excepted_scalar is same as the one built by scalar_builder
        assert_eq!(expected_scalar, scalar_builder.build().unwrap());
    }

    #[test]
    fn wrapped_scalar_builder_test() {
        let expected_scalar = WrappedScalar {
            name: vec!["Foo".to_string()],
            base_type: FullyQualifiedTypeReference {
                type_name: vec!["String".to_string()],
                parameters: vec![],
            },
            doc_comment: Some("This is scalar type".to_string()),
            source: anonymous_type(vec![type_constraint(named_type_ref("string"))]),
        };

        let mut scalar_builder = WrappedScalarBuilder::default();

        // sets all the information about the scalar type
        scalar_builder
            .name(vec!["Foo".to_string()])
            .base_type(FullyQualifiedTypeReference {
                type_name: vec!["String".to_string()],
                parameters: vec![],
            })
            .doc_comment(Some("This is scalar type".to_string()))
            .source(anonymous_type(vec![type_constraint(named_type_ref(
                "string",
            ))]));

        // Verify the excepted_scalar is same as the one built by scalar_builder
        assert_eq!(expected_scalar, scalar_builder.build().unwrap());
    }

    #[test]
    fn sequence_builder_test() {
        let expected_seq = Sequence {
            name: vec![],
            doc_comment: "This is sequence type of strings".to_string(),
            element_type: FullyQualifiedTypeReference {
                type_name: vec!["String".to_string()],
                parameters: vec![],
            },
            sequence_type: SequenceType::List,
            source: anonymous_type(vec![
                type_constraint(named_type_ref("list")),
                element(named_type_ref("string"), false),
            ]),
        };

        let mut seq_builder = SequenceBuilder::default();

        // sets all the information about the sequence except the `element_type`
        seq_builder
            .name(vec![])
            .doc_comment("This is sequence type of strings")
            .sequence_type(SequenceType::List)
            .source(anonymous_type(vec![
                type_constraint(named_type_ref("list")),
                element(named_type_ref("string"), false),
            ]));

        // Verify that not setting `element_type` returns an error while building the sequence
        assert!(seq_builder.build().is_err());

        // sets the `element_type` for the sequence
        seq_builder.element_type(FullyQualifiedTypeReference {
            type_name: vec!["String".to_string()],
            parameters: vec![],
        });

        // Verify the excepted_seq is same as the one built by seq_builder
        assert_eq!(expected_seq, seq_builder.build().unwrap());
    }

    #[test]
    fn struct_builder_test() {
        let expected_struct = Structure {
            name: vec!["org".to_string(), "example".to_string(), "Foo".to_string()],
            doc_comment: Some("This is a structure".to_string()),
            is_closed: false,
            fields: HashMap::from_iter(vec![
                (
                    "foo".to_string(),
                    FieldReference(
                        FullyQualifiedTypeReference {
                            type_name: vec!["String".to_string()],
                            parameters: vec![],
                        },
                        FieldPresence::Required,
                    ),
                ),
                (
                    "bar".to_string(),
                    FieldReference(
                        FullyQualifiedTypeReference {
                            type_name: vec!["int".to_string()],
                            parameters: vec![],
                        },
                        FieldPresence::Required,
                    ),
                ),
            ]),
            source: anonymous_type(vec![
                type_constraint(named_type_ref("struct")),
                fields(
                    vec![
                        (
                            "foo".to_string(),
                            variably_occurring_type_ref(
                                named_type_ref("string"),
                                UsizeRange::zero_or_one(),
                            ),
                        ),
                        (
                            "bar".to_string(),
                            variably_occurring_type_ref(
                                named_type_ref("int"),
                                UsizeRange::zero_or_one(),
                            ),
                        ),
                    ]
                    .into_iter(),
                    true,
                ),
            ]),
        };

        let mut struct_builder = StructureBuilder::default();

        // sets all the information about the structure
        struct_builder
            .name(vec![
                "org".to_string(),
                "example".to_string(),
                "Foo".to_string(),
            ])
            .doc_comment(Some("This is a structure".to_string()))
            .is_closed(false)
            .fields(HashMap::from_iter(vec![
                (
                    "foo".to_string(),
                    FieldReference(
                        FullyQualifiedTypeReference {
                            type_name: vec!["String".to_string()],
                            parameters: vec![],
                        },
                        FieldPresence::Required,
                    ),
                ),
                (
                    "bar".to_string(),
                    FieldReference(
                        FullyQualifiedTypeReference {
                            type_name: vec!["int".to_string()],
                            parameters: vec![],
                        },
                        FieldPresence::Required,
                    ),
                ),
            ]))
            .source(anonymous_type(vec![
                type_constraint(named_type_ref("struct")),
                fields(
                    vec![
                        (
                            "foo".to_string(),
                            variably_occurring_type_ref(
                                named_type_ref("string"),
                                UsizeRange::zero_or_one(),
                            ),
                        ),
                        (
                            "bar".to_string(),
                            variably_occurring_type_ref(
                                named_type_ref("int"),
                                UsizeRange::zero_or_one(),
                            ),
                        ),
                    ]
                    .into_iter(),
                    true,
                ),
            ]));

        // Verify the expected_struct is same as the one built by struct_builder
        assert_eq!(expected_struct, struct_builder.build().unwrap());
    }
}
