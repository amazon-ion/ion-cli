use crate::commands::generate::context::{CodeGenContext, SequenceType};
use crate::commands::generate::model::{
    AbstractDataType, DataModelNode, FieldPresence, FieldReference, FullyQualifiedTypeReference,
    Scalar, Sequence, Structure, WrappedScalar, WrappedSequence,
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
    // For Rust code generation, this will be set to None.
    current_type_fully_qualified_name: Option<Vec<String>>,
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
            current_type_fully_qualified_name: None,
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
            current_type_fully_qualified_name: Some(namespace),
            nested_type_counter: 0,
            tera,
            phantom: PhantomData,
            data_model_store: HashMap::new(),
        }
    }
}

impl<'a, L: Language + 'static> CodeGenerator<'a, L> {
    /// Represents a [tera] filter that converts given tera string value to [upper camel case].
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

    /// Represents a [tera] filter that converts given tera string value to [snake case].
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

    /// Represents a [tera] filter that return true if the value is a built in type, otherwise returns false.
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
                .ok_or(tera::Error::msg("Required string for this filter"))?
                .to_string(),
        )))
    }

    /// Represents a [tera] filter that return keys for the given object.
    ///
    /// For more information: <https://docs.rs/tera/1.19.0/tera/struct.Tera.html#method.register_filter>
    ///
    /// [tera]: <https://docs.rs/tera/latest/tera/>
    pub fn keys(
        value: &tera::Value,
        _map: &HashMap<String, tera::Value>,
    ) -> Result<tera::Value, tera::Error> {
        Ok(tera::Value::Array(
            value
                .as_object()
                .ok_or(tera::Error::msg("Required object for this filter"))?
                .keys()
                .map(|k| tera::Value::String(k.to_string()))
                .collect(),
        ))
    }

    /// Represents a [tera] filter that returns a string representation of a tera object i.e. `FullyQualifiedTypeReference`.
    ///
    /// For more information: <https://docs.rs/tera/1.19.0/tera/struct.Tera.html#method.register_filter>
    ///
    /// [tera]: <https://docs.rs/tera/latest/tera/>
    pub fn to_string(
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
        self.tera.register_filter("keys", Self::keys);
        self.tera.register_filter("to_string", Self::to_string);

        // Iterate through the ISL types, generate an abstract data type for each
        for isl_type in schema.types() {
            // unwrap here is safe because all the top-level type definition always has a name
            let isl_type_name = isl_type.name().clone().unwrap();
            self.generate_abstract_data_type(&isl_type_name, isl_type)?;
            // Since the fully qualified name of this generator represents the current fully qualified name,
            // remove it before generating code for the next ISL type.
            if let Some(ref mut fully_qualified_name) = self.current_type_fully_qualified_name {
                fully_qualified_name.pop();
            }
        }

        Ok(())
    }

    /// generates an nested type that can be part of another type definition.
    /// This will be used by the parent type to add this nested type in its namespace or module.
    fn generate_nested_type(
        &mut self,
        type_name: &String,
        isl_type: &IslType,
    ) -> CodeGenResult<FullyQualifiedTypeReference> {
        // Add an object called `nested_types` in tera context
        // This will have a list of `nested_type` where each will include fields, a target_kind_name and abstract_data_type
        let mut code_gen_context = CodeGenContext::new();
        self.traverse_isl_type_definition(type_name, isl_type, &mut code_gen_context)?;

        // TODO: verify the `occurs` value within a field, by default the fields are optional.
        if let Some(data_model_node) = &code_gen_context.data_model_node {
            if let Some(abstract_data_type) = &data_model_node.code_gen_type {
                let fully_qualified_type_ref =
                    Self::verify_abstract_data_type_and_get_fully_qualified_type_ref(
                        abstract_data_type,
                    )?;
                self.data_model_store.insert(
                    fully_qualified_type_ref.to_owned(),
                    data_model_node.to_owned(),
                );
                if let Some(ref mut fully_qualified_type_name) =
                    self.current_type_fully_qualified_name
                {
                    fully_qualified_type_name.pop();
                }
                Ok(fully_qualified_type_ref)
            } else {
                invalid_abstract_data_type_error(
                    "Can not determine abstract data type, specified constraints do not map to an abstract data type.",
                )
            }
        } else {
            invalid_abstract_data_type_error(
                "Can not determine abstract data type, specified constraints do not map to an abstract data type.",
            )
        }
    }

    fn verify_abstract_data_type_and_get_fully_qualified_type_ref(
        abstract_data_type: &AbstractDataType,
    ) -> CodeGenResult<FullyQualifiedTypeReference> {
        Ok(match abstract_data_type {
            AbstractDataType::WrappedSequence(seq) => {
                if seq.sequence_type.is_none() || seq.element_type.is_none() {
                    return invalid_abstract_data_type_error(
                        "Currently code generation does not support open ended types. \
            Error can be due to a missing `type` or `element` constraint in the type definition.",
                    );
                }
                L::target_type_as_sequence(abstract_data_type.fully_qualified_type_ref().ok_or(
                    invalid_abstract_data_type_raw_error(
                        "Can not determine fully qualified name for the data model",
                    ),
                )?)
            }
            AbstractDataType::Sequence(seq) => {
                if seq.sequence_type.is_none() || seq.element_type.is_none() {
                    return invalid_abstract_data_type_error(
                        "Currently code generation does not support open ended types. \
            Error can be due to a missing `type` or `element` constraint in the type definition.",
                    );
                }
                L::target_type_as_sequence(abstract_data_type.fully_qualified_type_ref().ok_or(
                    invalid_abstract_data_type_raw_error(
                        "Can not determine fully qualified name for the data model",
                    ),
                )?)
            }
            AbstractDataType::Structure(structure) => {
                if structure.fields.is_none() {
                    return invalid_abstract_data_type_error(
                        "Currently code generation does not support open ended types. \
            Error can be due to a missing `fields` constraint in the type definition.",
                    );
                }
                abstract_data_type.fully_qualified_type_ref().ok_or(
                    invalid_abstract_data_type_raw_error(
                        "Can not determine fully qualified name for the data model",
                    ),
                )?
            }
            _ => abstract_data_type.fully_qualified_type_ref().ok_or(
                invalid_abstract_data_type_raw_error(
                    "Can not determine fully qualified name for the data model",
                ),
            )?,
        })
    }

    fn generate_abstract_data_type(
        &mut self,
        isl_type_name: &String,
        isl_type: &IslType,
    ) -> CodeGenResult<()> {
        let mut context = Context::new();
        let mut code_gen_context = CodeGenContext::new();

        self.traverse_isl_type_definition(isl_type_name, isl_type, &mut code_gen_context)?;

        // add data model for template
        // TODO: verify the `occurs` value within a field, by default the fields are optional.
        if let Some(data_model_node) = &code_gen_context.data_model_node {
            if let Some(abstract_data_type) = &data_model_node.code_gen_type {
                let fully_qualified_type_ref =
                    Self::verify_abstract_data_type_and_get_fully_qualified_type_ref(
                        abstract_data_type,
                    )?;

                // add current data model node into the data model store
                self.data_model_store.insert(
                    fully_qualified_type_ref.to_owned(),
                    data_model_node.to_owned(),
                );
            }
            // add the entire type store into tera's context to be sued to render template
            context.insert(
                "type_store",
                &self
                    .data_model_store
                    .iter()
                    .map(|(k, v)| (format!("{}", k), v))
                    .collect::<HashMap<String, &DataModelNode>>(),
            );
            context.insert("model", &data_model_node);
        } else {
            return invalid_abstract_data_type_error(
                    "Can not determine abstract data type, specified constraints do not map to an abstract data type.",
                );
        }

        self.render_generated_code(isl_type_name, &mut context, &mut code_gen_context)
    }

    fn traverse_isl_type_definition(
        &mut self,
        isl_type_name: &String,
        isl_type: &IslType,
        mut code_gen_context: &mut CodeGenContext,
    ) -> CodeGenResult<()> {
        code_gen_context.with_data_model_node(DataModelNode {
            name: isl_type_name.to_case(Case::UpperCamel),
            code_gen_type: None,
            nested_types: vec![],
        });

        if let Some(ref mut fully_qualified_type_name) = self.current_type_fully_qualified_name {
            fully_qualified_type_name.push(isl_type_name.to_case(Case::UpperCamel));
        }

        let constraints = isl_type.constraints();
        for constraint in constraints {
            self.map_constraint_to_abstract_data_type(constraint, &mut code_gen_context, isl_type)?;
        }
        Ok(())
    }

    fn render_generated_code(
        &mut self,
        type_name: &str,
        context: &mut Context,
        code_gen_context: &mut CodeGenContext,
    ) -> CodeGenResult<()> {
        // Add namespace to tera context
        let mut import_context = Context::new();
        if let Some(ref mut namespace) = self.current_type_fully_qualified_name {
            let namespace_ref = namespace.as_slice();
            context.insert("namespace", &namespace_ref[0..namespace_ref.len() - 1]);
            import_context.insert("namespace", &namespace_ref[0..namespace_ref.len() - 1]);
        }
        // Render or generate file for the template with the given context
        let template: &Template = &code_gen_context.data_model_node.as_ref().try_into()?;

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

    /// Provides name of the type reference that will be used for generated abstract data type.
    /// Returns the fully qualified type reference of given ISL type. Returns None when the type can not be converted to a fully qualified name.
    fn fully_qualified_type_ref_name(
        &mut self,
        isl_type_ref: &IslTypeRef,
        code_gen_context: &mut CodeGenContext,
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
                let fully_qualified_type_ref = self.generate_nested_type(&name, type_def)?;
                code_gen_context.with_nested_type(
                    self.data_model_store
                        .get(&fully_qualified_type_ref)
                        .unwrap()
                        .to_owned(),
                );
                Some(fully_qualified_type_ref)
            }
        })
    }

    /// Provides the name of the next nested type
    fn next_nested_type_name(&mut self) -> String {
        self.nested_type_counter += 1;
        let name = format!("NestedType{}", self.nested_type_counter);
        name
    }

    /// Maps the given constraint value to an abstract data type
    fn map_constraint_to_abstract_data_type(
        &mut self,
        constraint: &IslConstraint,
        code_gen_context: &mut CodeGenContext,
        parent_isl_type: &IslType,
    ) -> CodeGenResult<()> {
        match constraint.constraint() {
            IslConstraintValue::Element(isl_type, _) => {
                let type_name = self.fully_qualified_type_ref_name(isl_type, code_gen_context)?;

                if let Some(ref mut data_model_node) = code_gen_context.data_model_node {
                    if data_model_node.code_gen_type.is_some() {
                        code_gen_context.with_element_type(type_name);
                    } else {
                        // TODO: Pass the data model name here and namespace
                        let mut name = vec![];
                        if let Some(namespace) = &self.current_type_fully_qualified_name {
                            name = namespace.to_vec();
                        }
                        if data_model_node.name.contains("NestedType") {
                            code_gen_context.with_abstract_data_type(AbstractDataType::Sequence(
                                Sequence {
                                    name,
                                    doc_comment: None,
                                    element_type: type_name,
                                    sequence_type: None,
                                    source: parent_isl_type.to_owned(),
                                },
                            ))
                        } else {
                            code_gen_context.with_abstract_data_type(
                                AbstractDataType::WrappedSequence(WrappedSequence {
                                    name: FullyQualifiedTypeReference {
                                        type_name: name,
                                        parameters: vec![],
                                    },
                                    doc_comment: None,
                                    element_type: type_name,
                                    sequence_type: None,
                                    source: parent_isl_type.to_owned(),
                                }),
                            )
                        }
                    }
                } else {
                    unreachable!(
                        "The data model node will always be initialized with atleast a name"
                    )
                }
            }
            IslConstraintValue::Fields(struct_fields, _) => {
                // TODO: Check for `closed` annotation on fields and based on that return error while reading if there are extra fields.
                let mut fields = HashMap::new();
                for (name, value) in struct_fields.iter() {
                    let type_name = self
                        .fully_qualified_type_ref_name(value.type_reference(), code_gen_context)?
                        .ok_or(invalid_abstract_data_type_raw_error(
                            "Given type doesn't have a name",
                        ))?;

                    // TODO: change the field presence field based on occurs constraint
                    // by default the field presence is optional
                    fields.insert(
                        name.to_string(),
                        FieldReference(type_name.to_owned(), FieldPresence::Optional),
                    );
                }
                if let Some(ref mut data_model_node) = code_gen_context.data_model_node {
                    if let Some(ref mut code_gen_type) = data_model_node.code_gen_type {
                        match code_gen_type {
                            AbstractDataType::Structure(ref mut structure) => {
                                structure.with_fields(fields);
                            }
                            _ => {
                                return invalid_abstract_data_type_error("Could not determine the abstract data type due to conflicting constraints")
                            }
                        }
                    } else {
                        let mut name = vec![];
                        if let Some(namespace) = &self.current_type_fully_qualified_name {
                            name = namespace.to_vec();
                        }
                        code_gen_context.with_abstract_data_type(AbstractDataType::Structure(
                            Structure {
                                name,
                                doc_comment: None,
                                is_closed: false,
                                fields: Some(fields),
                                source: parent_isl_type.to_owned(),
                            },
                        ))
                    }
                } else {
                    unreachable!(
                        "The data model node will always be initialized with at least a name"
                    )
                }
            }
            IslConstraintValue::Type(isl_type) => {
                let type_name = self.fully_qualified_type_ref_name(isl_type, code_gen_context)?;

                if let Some(ref mut data_model_node) = code_gen_context.data_model_node {
                    // If the code gen type is already defined then we need to modify the underlying type name with the given `type_name`
                    if let Some(ref mut code_gen_type) = data_model_node.code_gen_type {
                        match code_gen_type {
                            AbstractDataType::WrappedScalar(ref mut wrapped_scalar) => {
                                wrapped_scalar.with_type(type_name.ok_or(
                                    invalid_abstract_data_type_raw_error(
                                        "Given type doesn't have a name",
                                    ),
                                )?)
                            }
                            AbstractDataType::Scalar(ref mut scalar) => scalar.with_type(
                                type_name.ok_or(invalid_abstract_data_type_raw_error(
                                    "Given type doesn't have a name",
                                ))?,
                            ),
                            AbstractDataType::WrappedSequence(ref mut wrapped_seq) => {
                                let sequence_type = if isl_type.name() == "list" {
                                    SequenceType::List
                                } else if isl_type.name() == "sexp" {
                                    SequenceType::SExp
                                } else {
                                    return invalid_abstract_data_type_error("Could not determine the abstract data type due to conflicting constraints");
                                };
                                wrapped_seq.with_sequence_type(sequence_type);
                            }
                            AbstractDataType::Sequence(ref mut seq) => {
                                let sequence_type = if isl_type.name() == "list" {
                                    SequenceType::List
                                } else if isl_type.name() == "sexp" {
                                    SequenceType::SExp
                                } else {
                                    return invalid_abstract_data_type_error("Could not determine the abstract data type due to conflicting constraints");
                                };
                                seq.with_sequence_type(sequence_type);
                            }
                            AbstractDataType::Structure(ref mut structure) => {
                                // by default fields aren't closed
                                structure.with_open_fields();
                            }
                        }
                    } else {
                        // If the code gen type is not defined then we need to match with the given ISL type name and
                        // add a new code gen type based on that.
                        let mut name = vec![];
                        if let Some(namespace) = &self.current_type_fully_qualified_name {
                            name = namespace.to_vec();
                        }

                        let abstract_data_type = match isl_type.name().as_str() {
                            "list" => {
                                if data_model_node.name.contains("NestedType") {
                                    AbstractDataType::Sequence(Sequence {
                                        name: vec![],
                                        doc_comment: None,
                                        element_type: None,
                                        sequence_type: Some(SequenceType::List),
                                        source: parent_isl_type.to_owned(),
                                    })
                                } else {
                                    AbstractDataType::WrappedSequence(WrappedSequence {
                                        name: FullyQualifiedTypeReference {
                                            type_name: name,
                                            parameters: vec![],
                                        },
                                        doc_comment: None,
                                        element_type: None,
                                        sequence_type: Some(SequenceType::List),
                                        source: parent_isl_type.to_owned(),
                                    })
                                }
                            }
                            "sexp" => {
                                if data_model_node.name.contains("NestedType") {
                                    AbstractDataType::Sequence(Sequence {
                                        name: vec![],
                                        doc_comment: None,
                                        element_type: None,
                                        sequence_type: Some(SequenceType::SExp),
                                        source: parent_isl_type.to_owned(),
                                    })
                                } else {
                                    AbstractDataType::WrappedSequence(WrappedSequence {
                                        name: FullyQualifiedTypeReference {
                                            type_name: name,
                                            parameters: vec![],
                                        },
                                        doc_comment: None,
                                        element_type: None,
                                        sequence_type: Some(SequenceType::SExp),
                                        source: parent_isl_type.to_owned(),
                                    })
                                }
                            }
                            "struct" => AbstractDataType::Structure(Structure {
                                name,
                                doc_comment: None,
                                is_closed: false,
                                fields: None,
                                source: parent_isl_type.to_owned(),
                            }),
                            _ => {
                                if data_model_node.name.contains("NestedType") {
                                    AbstractDataType::Scalar(Scalar {
                                        name: type_name.unwrap().type_name,
                                        doc_comment: None,
                                        source: parent_isl_type.to_owned(),
                                    })
                                } else {
                                    AbstractDataType::WrappedScalar(WrappedScalar {
                                        name: FullyQualifiedTypeReference {
                                            type_name: name,
                                            parameters: vec![type_name.unwrap()],
                                        },
                                        doc_comment: None,
                                        source: parent_isl_type.to_owned(),
                                    })
                                }
                            }
                        };
                        data_model_node.with_abstract_data_type(abstract_data_type);
                    }
                } else {
                    unreachable!(
                        "The data model node will always be initialized with at least a name"
                    )
                }
            }
            _ => {}
        }
        Ok(())
    }
}
