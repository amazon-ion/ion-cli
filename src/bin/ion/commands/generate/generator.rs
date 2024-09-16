use crate::commands::generate::context::CodeGenContext;
use crate::commands::generate::model::{
    AbstractDataType, DataModelNode, FieldPresence, FieldReference, FullyQualifiedTypeReference,
    StructureBuilder,
};
use crate::commands::generate::result::{
    invalid_abstract_data_type_error, invalid_abstract_data_type_raw_error, CodeGenResult,
};
use crate::commands::generate::templates;
use crate::commands::generate::utils::{IonSchemaType, Template};
use crate::commands::generate::utils::{JavaLanguage, Language, RustLanguage};
use convert_case::{Case, Casing};
use ion_schema::isl::isl_constraint::{IslConstraint, IslConstraintValue};
use ion_schema::isl::isl_type::IslType;
use ion_schema::isl::isl_type_reference::IslTypeRef;
use ion_schema::isl::IslSchema;
use ion_schema::system::SchemaSystem;
use std::collections::HashMap;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::marker::PhantomData;
use std::path::Path;
use tera::{Context, Tera};

pub(crate) struct CodeGenerator<'a, L: Language> {
    // Represents the templating engine - tera
    // more information: https://docs.rs/tera/latest/tera/
    pub(crate) tera: Tera,
    output: &'a Path,
    // This field is used by Java code generation to get the namespace for generated code.
    current_type_fully_qualified_name: Vec<String>,
    // Represents a counter for naming nested type definitions
    pub(crate) nested_type_counter: usize,
    pub(crate) data_model_store: HashMap<FullyQualifiedTypeReference, DataModelNode>,
    phantom: PhantomData<L>,
}

impl<'a> CodeGenerator<'a, RustLanguage> {
    #[allow(dead_code)]
    pub fn new(output: &'a Path) -> CodeGenerator<RustLanguage> {
        let mut tera = Tera::default();
        // Add all templates using `rust_templates` module constants
        // This allows packaging binary without the need of template resources.
        tera.add_raw_templates(vec![
            ("struct.templ", templates::rust::STRUCT),
            ("scalar.templ", templates::rust::SCALAR),
            ("sequence.templ", templates::rust::SEQUENCE),
            ("util_macros.templ", templates::rust::UTIL_MACROS),
            ("import.templ", templates::rust::IMPORT),
            ("nested_type.templ", templates::rust::NESTED_TYPE),
            ("result.templ", templates::rust::RESULT),
        ])
        .unwrap();
        // Render the imports into output file
        let rendered_import = tera.render("import.templ", &Context::new()).unwrap();
        // Render the SerdeResult that is used in generated read-write APIs
        let rendered_result = tera.render("result.templ", &Context::new()).unwrap();

        let mut file = OpenOptions::new()
            // In order for the file to be created, OpenOptions::write or OpenOptions::append access must be used
            // reference: https://doc.rust-lang.org/std/fs/struct.OpenOptions.html#method.create
            .write(true)
            .truncate(true)
            .create(true)
            .open(output.join("ion_generated_code.rs"))
            .unwrap();
        file.write_all(rendered_import.as_bytes()).unwrap();
        file.write_all(rendered_result.as_bytes()).unwrap();

        Self {
            output,
            // Currently Rust code generation doesn't have a `--namespace` option available on the CLI, hence this is default set as an empty vector.
            current_type_fully_qualified_name: vec![],
            nested_type_counter: 0,
            tera,
            phantom: PhantomData,
            data_model_store: HashMap::new(),
        }
    }
}

impl<'a> CodeGenerator<'a, JavaLanguage> {
    pub fn new(output: &'a Path, namespace: Vec<String>) -> CodeGenerator<'a, JavaLanguage> {
        let mut tera = Tera::default();
        // Add all templates using `java_templates` module constants
        // This allows packaging binary without the need of template resources.
        tera.add_raw_templates(vec![
            ("class.templ", templates::java::CLASS),
            ("scalar.templ", templates::java::SCALAR),
            ("sequence.templ", templates::java::SEQUENCE),
            ("util_macros.templ", templates::java::UTIL_MACROS),
            ("nested_type.templ", templates::java::NESTED_TYPE),
        ])
        .unwrap();
        Self {
            output,
            current_type_fully_qualified_name: namespace,
            nested_type_counter: 0,
            tera,
            phantom: PhantomData,
            data_model_store: HashMap::new(),
        }
    }
}

impl<'a, L: Language + 'static> CodeGenerator<'a, L> {
    /// A [tera] filter that converts given tera string value to [upper camel case].
    /// Returns error if the given value is not a string.
    ///
    /// For more information: <https://docs.rs/tera/1.19.0/tera/struct.Tera.html#method.register_filter>
    ///
    /// [tera]: <https://docs.rs/tera/latest/tera/>
    /// [upper camel case]: <https://docs.rs/convert_case/latest/convert_case/enum.Case.html#variant.UpperCamel>
    pub fn upper_camel(
        value: &tera::Value,
        _map: &HashMap<String, tera::Value>,
    ) -> Result<tera::Value, tera::Error> {
        Ok(tera::Value::String(
            value
                .as_str()
                .ok_or(tera::Error::msg(
                    "the `upper_camel` filter only accepts strings",
                ))?
                .to_case(Case::UpperCamel),
        ))
    }

    /// Represents a [tera] filter that converts given tera string value to [camel case].
    /// Returns error if the given value is not a string.
    ///
    /// For more information: <https://docs.rs/tera/1.19.0/tera/struct.Tera.html#method.register_filter>
    ///
    /// [tera]: <https://docs.rs/tera/latest/tera/>
    /// [camel case]: <https://docs.rs/convert_case/latest/convert_case/enum.Case.html#variant.Camel>
    pub fn camel(
        value: &tera::Value,
        _map: &HashMap<String, tera::Value>,
    ) -> Result<tera::Value, tera::Error> {
        Ok(tera::Value::String(
            value
                .as_str()
                .ok_or(tera::Error::msg("Required string for this filter"))?
                .to_case(Case::Camel),
        ))
    }

    /// A [tera] filter that converts given tera string value to [snake case].
    /// Returns error if the given value is not a string.
    ///
    /// For more information: <https://docs.rs/tera/1.19.0/tera/struct.Tera.html#method.register_filter>
    ///
    /// [tera]: <https://docs.rs/tera/latest/tera/>
    /// [snake case]: <https://docs.rs/convert_case/latest/convert_case/enum.Case.html#variant.Camel>
    pub fn snake(
        value: &tera::Value,
        _map: &HashMap<String, tera::Value>,
    ) -> Result<tera::Value, tera::Error> {
        Ok(tera::Value::String(
            value
                .as_str()
                .ok_or(tera::Error::msg("Required string for this filter"))?
                .to_case(Case::Snake),
        ))
    }

    /// A [tera] filter that return true if the value is a built in type, otherwise returns false.
    ///
    /// For more information: <https://docs.rs/tera/1.19.0/tera/struct.Tera.html#method.register_filter>
    ///
    /// [tera]: <https://docs.rs/tera/latest/tera/>
    pub fn is_built_in_type(
        value: &tera::Value,
        _map: &HashMap<String, tera::Value>,
    ) -> Result<tera::Value, tera::Error> {
        Ok(tera::Value::Bool(L::is_built_in_type(
            value
                .as_str()
                .ok_or(tera::Error::msg(
                    "Required string for the `is_built_in_type` filter",
                ))?
                .to_string(),
        )))
    }

    /// A [tera] filter that return field names for the given object.
    ///
    /// For more information: <https://docs.rs/tera/1.19.0/tera/struct.Tera.html#method.register_filter>
    ///
    /// [tera]: <https://docs.rs/tera/latest/tera/>
    pub fn field_names(
        value: &tera::Value,
        _map: &HashMap<String, tera::Value>,
    ) -> Result<tera::Value, tera::Error> {
        Ok(tera::Value::Array(
            value
                .as_object()
                .ok_or(tera::Error::msg("Required object for `keys` filter"))?
                .keys()
                .map(|k| tera::Value::String(k.to_string()))
                .collect(),
        ))
    }

    /// A [tera] filter that returns a string representation of a tera object i.e. `FullyQualifiedTypeReference`.
    ///
    /// For more information: <https://docs.rs/tera/1.19.0/tera/struct.Tera.html#method.register_filter>
    ///
    /// [tera]: <https://docs.rs/tera/latest/tera/>
    pub fn fully_qualified_type_name(
        value: &tera::Value,
        _map: &HashMap<String, tera::Value>,
    ) -> Result<tera::Value, tera::Error> {
        let fully_qualified_type_ref: &FullyQualifiedTypeReference = &value.try_into()?;
        Ok(tera::Value::String(fully_qualified_type_ref.to_string()))
    }

    /// Generates code for all the schemas in given authorities
    pub fn generate_code_for_authorities(
        &mut self,
        authorities: &Vec<&String>,
        schema_system: &mut SchemaSystem,
    ) -> CodeGenResult<()> {
        for authority in authorities {
            // Sort the directory paths to ensure nested type names are always ordered based
            // on directory path. (nested type name uses a counter in its name to represent that type)
            let mut paths = fs::read_dir(authority)?.collect::<Result<Vec<_>, _>>()?;
            paths.sort_by_key(|dir| dir.path());
            for schema_file in paths {
                let schema_file_path = schema_file.path();
                let schema_id = schema_file_path.file_name().unwrap().to_str().unwrap();

                let schema = schema_system.load_isl_schema(schema_id).unwrap();

                self.generate(schema)?;
            }
        }
        Ok(())
    }

    /// Generates code for given Ion Schema
    pub fn generate_code_for_schema(
        &mut self,
        schema_system: &mut SchemaSystem,
        schema_id: &str,
    ) -> CodeGenResult<()> {
        let schema = schema_system.load_isl_schema(schema_id).unwrap();
        self.generate(schema)
    }

    fn generate(&mut self, schema: IslSchema) -> CodeGenResult<()> {
        // Register a tera filter that can be used to convert a string based on case
        self.tera.register_filter("upper_camel", Self::upper_camel);
        self.tera.register_filter("snake", Self::snake);
        self.tera.register_filter("camel", Self::camel);

        // Register a tera filter that can be used to see if a type is built in data type or not
        self.tera
            .register_filter("is_built_in_type", Self::is_built_in_type);
        self.tera.register_filter("field_names", Self::field_names);
        self.tera
            .register_filter("fully_qualified_type_name", Self::fully_qualified_type_name);

        // Iterate through the ISL types, generate an abstract data type for each
        for isl_type in schema.types() {
            // unwrap here is safe because all the top-level type definition always has a name
            let isl_type_name = isl_type.name().clone().unwrap();
            self.generate_abstract_data_type(&isl_type_name, isl_type)?;
            // Since the fully qualified name of this generator represents the current fully qualified name,
            // remove it before generating code for the next ISL type.
            self.current_type_fully_qualified_name.pop();
        }

        Ok(())
    }

    /// generates an nested type that can be part of another type definition.
    /// This will be used by the parent type to add this nested type in its namespace or module.
    fn generate_nested_type(
        &mut self,
        type_name: &String,
        isl_type: &IslType,
        parent_code_gen_context: &mut CodeGenContext,
    ) -> CodeGenResult<FullyQualifiedTypeReference> {
        let mut code_gen_context = CodeGenContext::new();
        let mut data_model_node = self.convert_isl_type_def_to_data_model_node(
            type_name,
            isl_type,
            &mut code_gen_context,
        )?;

        // add this nested type to parent code gene context's current list of nested types
        parent_code_gen_context
            .nested_types
            .push(data_model_node.to_owned());

        // pop out the nested type name from the fully qualified namespace as it has been already added to the type store and to nested types
        self.current_type_fully_qualified_name.pop();
        data_model_node
            .fully_qualified_type_ref()
            .ok_or(invalid_abstract_data_type_raw_error(
                "Can not determine fully qualified name for the data model",
            ))
    }

    fn generate_abstract_data_type(
        &mut self,
        isl_type_name: &String,
        isl_type: &IslType,
    ) -> CodeGenResult<()> {
        let mut context = Context::new();
        let mut code_gen_context = CodeGenContext::new();

        let data_model_node = self.convert_isl_type_def_to_data_model_node(
            isl_type_name,
            isl_type,
            &mut code_gen_context,
        )?;

        // add the entire type store and the data model node into tera's context to be used to render template
        context.insert(
            "type_store",
            &self
                .data_model_store
                .iter()
                .map(|(k, v)| (format!("{}", k), v))
                .collect::<HashMap<String, &DataModelNode>>(),
        );
        context.insert("model", &data_model_node);

        self.render_generated_code(isl_type_name, &mut context, &data_model_node)
    }

    fn convert_isl_type_def_to_data_model_node(
        &mut self,
        isl_type_name: &String,
        isl_type: &IslType,
        code_gen_context: &mut CodeGenContext,
    ) -> CodeGenResult<DataModelNode> {
        self.current_type_fully_qualified_name
            .push(isl_type_name.to_case(Case::UpperCamel));

        let constraints = isl_type.constraints();

        // Initialize `AbstractDataType` according to the first constraint in the list of constraints
        let abstract_data_type = if constraints
            .iter()
            .any(|it| matches!(it.constraint(), IslConstraintValue::Fields(_, _)))
        {
            self.build_structure_from_constraints(constraints, code_gen_context, isl_type)?
        } else {
            todo!("Support for sequences, maps, scalars, and tuples not implemented yet.")
        };

        let data_model_node = DataModelNode {
            name: isl_type_name.to_case(Case::UpperCamel),
            code_gen_type: Some(abstract_data_type.to_owned()),
            nested_types: code_gen_context.nested_types.to_owned(),
        };

        // TODO: verify the `occurs` value within a field, by default the fields are optional.
        // add current data model node into the data model store
        self.data_model_store.insert(
            abstract_data_type.fully_qualified_type_ref().ok_or(
                invalid_abstract_data_type_raw_error(
                    "Can not determine fully qualified name for the data model",
                ),
            )?,
            data_model_node.to_owned(),
        );
        Ok(data_model_node)
    }

    fn render_generated_code(
        &mut self,
        type_name: &str,
        context: &mut Context,
        data_model_node: &DataModelNode,
    ) -> CodeGenResult<()> {
        // Add namespace to tera context
        let mut import_context = Context::new();
        let namespace_ref = self.current_type_fully_qualified_name.as_slice();
        context.insert("namespace", &namespace_ref[0..namespace_ref.len() - 1]);
        import_context.insert("namespace", &namespace_ref[0..namespace_ref.len() - 1]);

        // Render or generate file for the template with the given context
        let template: &Template = &data_model_node.try_into()?;

        // This will be used by Java templates. Since `java` templates use recursion(i.e. use the same template for nested types) when rendering nested types,
        // We need to tune the `is_nested` flag to allow static classes being added inside a parent class
        context.insert("is_nested", &false);

        let rendered = self
            .tera
            .render(&format!("{}.templ", L::template_name(template)), context)
            .unwrap();
        let mut file_options = OpenOptions::new();
        if L::name() == "rust" {
            // since Rust code is generated into a single file, it needs append set to true.
            file_options.append(true);
        }
        let mut file = file_options
            // In order for the file to be created, OpenOptions::write or OpenOptions::append access must be used
            // reference: https://doc.rust-lang.org/std/fs/struct.OpenOptions.html#method.create
            .write(true)
            .create(true)
            .open(self.output.join(format!(
                "{}.{}",
                L::file_name_for_type(type_name),
                L::file_extension()
            )))?;
        file.write_all(rendered.as_bytes())?;
        Ok(())
    }

    /// Provides the `FullyQualifiedTypeReference` to be used for the `AbstractDataType` in the data model.
    /// Returns None when the given ISL type is `struct`, `list` or `sexp` as open-ended types are not supported currently.
    fn fully_qualified_type_ref_name(
        &mut self,
        isl_type_ref: &IslTypeRef,
        parent_code_gen_context: &mut CodeGenContext,
    ) -> CodeGenResult<Option<FullyQualifiedTypeReference>> {
        Ok(match isl_type_ref {
            IslTypeRef::Named(name, _) => {
                let schema_type: IonSchemaType = name.into();
                L::target_type(&schema_type)
                    .as_ref()
                    .map(|type_name| FullyQualifiedTypeReference {
                        type_name: vec![type_name.to_string()],
                        parameters: vec![],
                    })
            }
            IslTypeRef::TypeImport(_, _) => {
                unimplemented!("Imports in schema are not supported yet!");
            }
            IslTypeRef::Anonymous(type_def, _) => {
                let name = self.next_nested_type_name();
                Some(self.generate_nested_type(&name, type_def, parent_code_gen_context)?)
            }
        })
    }

    /// Provides the name of the next nested type
    fn next_nested_type_name(&mut self) -> String {
        self.nested_type_counter += 1;
        let name = format!("NestedType{}", self.nested_type_counter);
        name
    }

    /// Build structure from constraints
    fn build_structure_from_constraints(
        &mut self,
        constraints: &[IslConstraint],
        code_gen_context: &mut CodeGenContext,
        parent_isl_type: &IslType,
    ) -> CodeGenResult<AbstractDataType> {
        let mut structure_builder = StructureBuilder::default();
        for constraint in constraints {
            match constraint.constraint() {
                IslConstraintValue::Fields(struct_fields, is_closed) => {
                    // TODO: Check for `closed` annotation on fields and based on that return error while reading if there are extra fields.
                    let mut fields = HashMap::new();
                    for (name, value) in struct_fields.iter() {
                        let type_name = self
                            .fully_qualified_type_ref_name(
                                value.type_reference(),
                                code_gen_context,
                            )?
                            .ok_or(invalid_abstract_data_type_raw_error(
                                "Given type doesn't have a name",
                            ))?;

                        // TODO: change the field presence based on occurs constraint
                        // by default the field presence is optional
                        fields.insert(
                            name.to_string(),
                            FieldReference(type_name.to_owned(), FieldPresence::Optional),
                        );
                    }
                    // unwrap here is safe as the `current_abstract_data_type_builder` will either be initialized with default implementation
                    // or already initialized with a previous structure related constraint at this point.
                    structure_builder
                        .fields(fields)
                        .source(parent_isl_type.to_owned())
                        .is_closed(*is_closed)
                        .name(self.current_type_fully_qualified_name.to_owned());
                }
                IslConstraintValue::Type(_) => {
                    // by default fields aren't closed
                    structure_builder
                        .is_closed(false)
                        .source(parent_isl_type.to_owned());
                }
                _ => {
                    return invalid_abstract_data_type_error(
                        "Could not determine the abstract data type due to conflicting constraints",
                    )
                }
            }
        }

        Ok(AbstractDataType::Structure(structure_builder.build()?))
    }
}

#[cfg(test)]
mod isl_to_model_tests {
    use super::*;
    use crate::commands::generate::model::AbstractDataType;
    use ion_schema::isl;

    #[test]
    fn isl_to_model_test_for_struct() -> CodeGenResult<()> {
        let isl_type = isl::isl_type::v_2_0::load_isl_type(
            r#"
                // ISL type definition with `fields` constraint
                type:: {
                    name: my_struct,
                    type: struct,
                    fields: {
                        foo: string,
                        bar: int
                    },
                }
            "#
            .as_bytes(),
        )?;

        // Initialize code generator for Java
        let mut java_code_generator = CodeGenerator::<JavaLanguage>::new(
            Path::new("./"),
            vec!["org".to_string(), "example".to_string()],
        );
        let data_model_node = java_code_generator.convert_isl_type_def_to_data_model_node(
            &"my_struct".to_string(),
            &isl_type,
            &mut CodeGenContext::new(),
        )?;
        let abstract_data_type = data_model_node.code_gen_type.unwrap();
        assert_eq!(
            abstract_data_type.fully_qualified_type_ref().unwrap(),
            FullyQualifiedTypeReference {
                type_name: vec![
                    "org".to_string(),
                    "example".to_string(),
                    "MyStruct".to_string()
                ],
                parameters: vec![]
            }
        );
        assert!(matches!(abstract_data_type, AbstractDataType::Structure(_)));
        if let AbstractDataType::Structure(structure) = abstract_data_type {
            assert_eq!(
                structure.name,
                vec![
                    "org".to_string(),
                    "example".to_string(),
                    "MyStruct".to_string()
                ]
            );
            assert!(!structure.is_closed);
            assert_eq!(structure.source, isl_type);
            assert_eq!(
                structure.fields,
                HashMap::from_iter(vec![
                    (
                        "foo".to_string(),
                        FieldReference(
                            FullyQualifiedTypeReference {
                                type_name: vec!["String".to_string()],
                                parameters: vec![]
                            },
                            FieldPresence::Optional
                        )
                    ),
                    (
                        "bar".to_string(),
                        FieldReference(
                            FullyQualifiedTypeReference {
                                type_name: vec!["int".to_string()],
                                parameters: vec![]
                            },
                            FieldPresence::Optional
                        )
                    )
                ])
            )
        }
        Ok(())
    }

    #[test]
    fn isl_to_model_test_for_nested_struct() -> CodeGenResult<()> {
        let isl_type = isl::isl_type::v_2_0::load_isl_type(
            r#"
                // ISL type definition with nested `fields` constraint
                type:: {
                    name: my_nested_struct,
                    type: struct,
                    fields: {
                        foo: {
                            fields: {
                                baz: bool
                            },
                            type: struct
                        },
                        bar: int
                    },
                }
            "#
            .as_bytes(),
        )?;

        // Initialize code generator for Java
        let mut java_code_generator = CodeGenerator::<JavaLanguage>::new(
            Path::new("./"),
            vec!["org".to_string(), "example".to_string()],
        );
        let data_model_node = java_code_generator.convert_isl_type_def_to_data_model_node(
            &"my_nested_struct".to_string(),
            &isl_type,
            &mut CodeGenContext::new(),
        )?;
        let abstract_data_type = data_model_node.code_gen_type.unwrap();
        assert_eq!(
            abstract_data_type.fully_qualified_type_ref().unwrap(),
            FullyQualifiedTypeReference {
                type_name: vec![
                    "org".to_string(),
                    "example".to_string(),
                    "MyNestedStruct".to_string()
                ],
                parameters: vec![]
            }
        );
        assert!(matches!(abstract_data_type, AbstractDataType::Structure(_)));
        if let AbstractDataType::Structure(structure) = abstract_data_type {
            assert_eq!(
                structure.name,
                vec![
                    "org".to_string(),
                    "example".to_string(),
                    "MyNestedStruct".to_string()
                ]
            );
            assert!(!structure.is_closed);
            assert_eq!(structure.source, isl_type);
            assert_eq!(
                structure.fields,
                HashMap::from_iter(vec![
                    (
                        "foo".to_string(),
                        FieldReference(
                            FullyQualifiedTypeReference {
                                type_name: vec![
                                    "org".to_string(),
                                    "example".to_string(),
                                    "MyNestedStruct".to_string(),
                                    "NestedType1".to_string()
                                ],
                                parameters: vec![]
                            },
                            FieldPresence::Optional
                        )
                    ),
                    (
                        "bar".to_string(),
                        FieldReference(
                            FullyQualifiedTypeReference {
                                type_name: vec!["int".to_string()],
                                parameters: vec![]
                            },
                            FieldPresence::Optional
                        )
                    )
                ])
            );
            assert_eq!(data_model_node.nested_types.len(), 1);
            assert_eq!(
                data_model_node.nested_types[0]
                    .code_gen_type
                    .as_ref()
                    .unwrap()
                    .fully_qualified_type_ref(),
                Some(FullyQualifiedTypeReference {
                    type_name: vec![
                        "org".to_string(),
                        "example".to_string(),
                        "MyNestedStruct".to_string(),
                        "NestedType1".to_string()
                    ],
                    parameters: vec![]
                })
            );
        }
        Ok(())
    }
}
