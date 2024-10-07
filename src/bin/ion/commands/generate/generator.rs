use crate::commands::generate::context::{CodeGenContext, SequenceType};
use crate::commands::generate::model::{
    AbstractDataType, DataModelNode, FieldPresence, FieldReference, FullyQualifiedTypeReference,
    ScalarBuilder, SequenceBuilder, StructureBuilder, WrappedScalarBuilder, WrappedSequenceBuilder,
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

    /// A [tera] filter that returns the parameter names for given fully qualified type name.
    ///
    /// For more information: <https://docs.rs/tera/1.19.0/tera/struct.Tera.html#method.register_filter>
    ///
    /// [tera]: <https://docs.rs/tera/latest/tera/>
    pub fn parameters(
        value: &tera::Value,
        _map: &HashMap<String, tera::Value>,
    ) -> Result<tera::Value, tera::Error> {
        let fully_qualified_type_ref: &FullyQualifiedTypeReference = &value.try_into()?;
        Ok(tera::Value::Array(
            fully_qualified_type_ref
                .parameters
                .iter()
                .map(|p| tera::Value::String(p.string_representation::<L>()))
                .collect(),
        ))
    }

    /// A [tera] filter that return primitive data type name for given wrapper class name.
    ///
    /// For more information: <https://docs.rs/tera/1.19.0/tera/struct.Tera.html#method.register_filter>
    ///
    /// [tera]: <https://docs.rs/tera/latest/tera/>
    pub fn primitive_data_type(
        value: &tera::Value,
        _map: &HashMap<String, tera::Value>,
    ) -> Result<tera::Value, tera::Error> {
        Ok(tera::Value::String(
            JavaLanguage::primitive_data_type(value.as_str().ok_or(tera::Error::msg(
                "Required string for `primitive_data_type` filter",
            ))?)
            .to_string(),
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
        Ok(tera::Value::String(
            fully_qualified_type_ref.string_representation::<L>(),
        ))
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

        self.tera.register_filter("parameters", Self::parameters);
        self.tera
            .register_filter("primitive_data_type", Self::primitive_data_type);

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
    /// _Note: `field_presence` is only used ofr variably occurring type references and currently that is only supported with `fields` constraint.
    /// For all other cases `field_presence` will be set as default `FieldPresence::Required`._
    fn generate_nested_type(
        &mut self,
        type_name: &String,
        isl_type: &IslType,
        field_presence: FieldPresence,
        parent_code_gen_context: &mut CodeGenContext,
    ) -> CodeGenResult<FullyQualifiedTypeReference> {
        let mut code_gen_context = CodeGenContext::new();
        let mut data_model_node = self.convert_isl_type_def_to_data_model_node(
            type_name,
            field_presence,
            isl_type,
            &mut code_gen_context,
            true,
        )?;

        // add this nested type to parent code gene context's current list of nested types
        parent_code_gen_context
            .nested_types
            .push(data_model_node.to_owned());

        // pop out the nested type name from the fully qualified namespace as it has been already added to the type store and to nested types
        self.current_type_fully_qualified_name.pop();
        match field_presence {
            FieldPresence::Optional => Ok(L::target_type_as_optional(
                data_model_node.fully_qualified_type_ref::<L>().ok_or(
                    invalid_abstract_data_type_raw_error(
                        "Can not determine fully qualified name for the data model",
                    ),
                )?,
            )),
            FieldPresence::Required => data_model_node.fully_qualified_type_ref::<L>().ok_or(
                invalid_abstract_data_type_raw_error(
                    "Can not determine fully qualified name for the data model",
                ),
            ),
        }
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
            FieldPresence::Required, // Sets `field_presence` as `Required`, as the top level type definition can not be `Optional`.
            isl_type,
            &mut code_gen_context,
            false,
        )?;

        // add the entire type store and the data model node into tera's context to be used to render template
        context.insert(
            "type_store",
            &self
                .data_model_store
                .iter()
                .map(|(k, v)| (k.string_representation::<L>(), v))
                .collect::<HashMap<String, &DataModelNode>>(),
        );
        context.insert("model", &data_model_node);

        self.render_generated_code(isl_type_name, &mut context, &data_model_node)
    }

    /// _Note: `field_presence` is only used ofr variably occurring type references and currently that is only supported with `fields` constraint.
    /// For all other cases `field_presence` will be set as default `FieldPresence::Required`._
    fn convert_isl_type_def_to_data_model_node(
        &mut self,
        isl_type_name: &String,
        field_presence: FieldPresence,
        isl_type: &IslType,
        code_gen_context: &mut CodeGenContext,
        is_nested_type: bool,
    ) -> CodeGenResult<DataModelNode> {
        L::add_type_to_namespace(
            is_nested_type,
            isl_type_name,
            &mut self.current_type_fully_qualified_name,
        );

        let constraints = isl_type.constraints();

        // Initialize `AbstractDataType` according to the list of constraints
        // Below are some checks to verify which AbstractDatatype variant should be constructed based on given ISL constraints:
        // * If given list of constraints has any `fields` constraint then `AbstractDataType::Structure` needs to be constructed.
        //      * Since currently, code generation doesn't support open ended types having `type: struct` alone is not enough for constructing
        //        `AbstractDataType::Structure`.
        // * If given list of constraints has any `element` constraint then `AbstractDataType::Sequence` needs to be constructed.
        //      * Since currently, code generation doesn't support open ended types having `type: list` or `type: sexp` alone is not enough for constructing
        //        `AbstractDataType::Sequence`.
        //      * The sequence type for `Sequence` will be stored based on `type` constraint with either `list` or `sexp`.
        // * If given list of constraints has any `type` constraint except `type: list`, `type: struct` and `type: sexp`, then `AbstractDataType::Scalar` needs to be constructed.
        //      * The `base_type` for `Scalar` will be stored based on `type` constraint.
        // * All the other constraints except the above ones are not yet supported by code generator.
        let abstract_data_type = if constraints
            .iter()
            .any(|it| matches!(it.constraint(), IslConstraintValue::Fields(_, _)))
        {
            self.build_structure_from_constraints(constraints, code_gen_context, isl_type)?
        } else if constraints
            .iter()
            .any(|it| matches!(it.constraint(), IslConstraintValue::Element(_, _)))
        {
            if is_nested_type {
                self.build_sequence_from_constraints(constraints, code_gen_context, isl_type)?
            } else {
                self.build_wrapped_sequence_from_constraints(
                    constraints,
                    code_gen_context,
                    isl_type,
                )?
            }
        } else if Self::contains_scalar_constraints(constraints) {
            if is_nested_type {
                self.build_scalar_from_constraints(constraints, code_gen_context, isl_type)?
            } else {
                self.build_wrapped_scalar_from_constraints(constraints, code_gen_context, isl_type)?
            }
        } else {
            todo!("Support for maps and tuples not implemented yet.")
        };

        let data_model_node = DataModelNode {
            name: isl_type_name.to_case(Case::UpperCamel),
            code_gen_type: Some(abstract_data_type.to_owned()),
            nested_types: code_gen_context.nested_types.to_owned(),
        };

        // TODO: verify the `occurs` value within a field, by default the fields are optional.
        // add current data model node into the data model store
        // verify if the field presence was provided as optional and set the type reference name as optional.
        let type_name = match field_presence {
            FieldPresence::Optional => abstract_data_type.fully_qualified_type_ref::<L>().ok_or(
                invalid_abstract_data_type_raw_error(
                    "Can not determine fully qualified name for the data model",
                ),
            )?,
            FieldPresence::Required => abstract_data_type.fully_qualified_type_ref::<L>().ok_or(
                invalid_abstract_data_type_raw_error(
                    "Can not determine fully qualified name for the data model",
                ),
            )?,
        };

        self.data_model_store
            .insert(type_name, data_model_node.to_owned());

        Ok(data_model_node)
    }

    /// Verifies if the given constraints contain a `type` constraint without any container type references. (e.g. `sexp`, `list`, `struct`)
    fn contains_scalar_constraints(constraints: &[IslConstraint]) -> bool {
        constraints.iter().any(|it| matches!(it.constraint(), IslConstraintValue::Type(isl_type_ref) if isl_type_ref.name().as_str() != "list"
                     && isl_type_ref.name().as_str() != "sexp"
                     && isl_type_ref.name().as_str() != "struct"))
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
    /// Returns `None` when the given ISL type is `struct`, `list` or `sexp` as open-ended types are not supported currently.
    /// _Note: `field_presence` is only used ofr variably occurring type references and currently that is only supported with `fields` constraint.
    /// For all other cases `field_presence` will be set as default `FieldPresence::Required`._
    fn fully_qualified_type_ref_name(
        &mut self,
        isl_type_ref: &IslTypeRef,
        field_presence: FieldPresence,
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
                    .and_then(|t| {
                        if field_presence == FieldPresence::Optional {
                            Some(L::target_type_as_optional(t))
                        } else {
                            Some(t)
                        }
                    })
            }
            IslTypeRef::TypeImport(_, _) => {
                unimplemented!("Imports in schema are not supported yet!");
            }
            IslTypeRef::Anonymous(type_def, _) => {
                let name = self.next_nested_type_name();
                Some(self.generate_nested_type(
                    &name,
                    type_def,
                    field_presence,
                    parent_code_gen_context,
                )?)
            }
        })
    }

    /// Provides the name of the next nested type
    fn next_nested_type_name(&mut self) -> String {
        self.nested_type_counter += 1;
        let name = format!("NestedType{}", self.nested_type_counter);
        name
    }

    /// Returns error if duplicate constraints are present based `found_constraint` flag
    fn handle_duplicate_constraint(
        &mut self,
        found_constraint: bool,
        constraint_name: &str,
        isl_type: &IslTypeRef,
        field_presence: FieldPresence,
        code_gen_context: &mut CodeGenContext,
    ) -> CodeGenResult<FullyQualifiedTypeReference> {
        if found_constraint {
            return invalid_abstract_data_type_error(format!(
                "Multiple `{}` constraints in the type definitions are not supported in code generation as it can lead to conflicting types.", constraint_name
            ));
        }

        self.fully_qualified_type_ref_name(isl_type, field_presence, code_gen_context)?
            .ok_or(invalid_abstract_data_type_raw_error(format!(
                "Could not determine `FullQualifiedTypeReference` for type {:?}",
                isl_type
            )))
    }

    /// Builds `AbstractDataType::Structure` from the given constraints.
    /// e.g. for a given type definition as below:
    /// ```
    /// type::{
    ///   name: Foo,
    ///   type: struct,
    ///   fields: {
    ///      a: string,
    ///      b: int,
    ///   }
    /// }
    /// ```
    /// This method builds `AbstractDataType`as following:
    /// ```
    /// AbstractDataType::Structure(
    ///  Structure {
    ///     name: vec!["org", "example", "Foo"], // assuming the namespace is `org.example`
    ///     fields: {
    ///         a: FieldReference { FullyQualifiedTypeReference { type_name: vec!["String"], parameters: vec![] }, FieldPresence::Optional },
    ///         b: FieldReference { FullyQualifiedTypeReference { type_name: vec!["int"], parameters: vec![] }, FieldPresence::Optional },
    ///     }, // HashMap with fields defined through `fields` constraint above
    ///     doc_comment: None // There is no doc comment defined in above ISL type def
    ///     source: IslType {name: "foo", .. } // Represents the `IslType` that is getting converted to `AbstractDataType`
    ///     is_closed: false, // If the fields constraint was annotated with `closed` then this would be true.
    ///  }
    /// )
    /// ```
    fn build_structure_from_constraints(
        &mut self,
        constraints: &[IslConstraint],
        code_gen_context: &mut CodeGenContext,
        parent_isl_type: &IslType,
    ) -> CodeGenResult<AbstractDataType> {
        let mut structure_builder = StructureBuilder::default();
        structure_builder
            .name(self.current_type_fully_qualified_name.to_owned())
            .source(parent_isl_type.to_owned());
        for constraint in constraints {
            match constraint.constraint() {
                IslConstraintValue::Fields(struct_fields, is_closed) => {
                    // TODO: Check for `closed` annotation on fields and based on that return error while reading if there are extra fields.
                    let mut fields = HashMap::new();
                    for (name, value) in struct_fields.iter() {
                        let field_presence = if value.occurs().inclusive_endpoints() == (0, 1) {
                            FieldPresence::Optional
                        } else if value.occurs().inclusive_endpoints() == (1, 1) {
                            FieldPresence::Required
                        } else {
                            // TODO: change the field presence based on occurs constraint
                            return invalid_abstract_data_type_error("Fields with occurs as a range aren't supported with code generation");
                        };
                        let type_name = self
                            .fully_qualified_type_ref_name(
                                value.type_reference(),
                                field_presence,
                                code_gen_context,
                            )?
                            .ok_or(invalid_abstract_data_type_raw_error(
                                "Given type doesn't have a name",
                            ))?;
                        fields.insert(
                            name.to_string(),
                            FieldReference(type_name.to_owned(), field_presence),
                        );
                    }
                    // unwrap here is safe as the `current_abstract_data_type_builder` will either be initialized with default implementation
                    // or already initialized with a previous structure related constraint at this point.
                    structure_builder.fields(fields).is_closed(*is_closed);
                }
                IslConstraintValue::Type(_) => {
                    // by default fields aren't closed
                    structure_builder.is_closed(false);
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

    /// Builds `AbstractDataType::WrappedScalar` from the given constraints.
    /// ```
    /// type::{
    ///   name: Foo,
    ///   type: string,
    /// }
    /// ```
    /// This method builds `AbstractDataType`as following:
    /// ```
    /// AbstractDataType::WrappedScalar(
    ///  WrappedScalar {
    ///     name: vec!["org", "example", "Foo"], // assuming the namespace is `org.example`
    ///     base_type: FullyQualifiedTypeReference { type_name: vec!["String"], parameters: vec![] }
    ///     doc_comment: None // There is no doc comment defined in above ISL type def
    ///     source: IslType {name: "foo", .. } // Represents the `IslType` that is getting converted to `AbstractDataType`
    ///  }
    /// )
    /// ```
    ///
    /// _Note: Currently code generator would return an error when there are multiple `type` constraints in the type definition.
    /// This avoids providing conflicting type constraints in the type definition._
    fn build_wrapped_scalar_from_constraints(
        &mut self,
        constraints: &[IslConstraint],
        code_gen_context: &mut CodeGenContext,
        parent_isl_type: &IslType,
    ) -> CodeGenResult<AbstractDataType> {
        let mut wrapped_scalar_builder = WrappedScalarBuilder::default();
        wrapped_scalar_builder
            .name(self.current_type_fully_qualified_name.to_owned())
            .source(parent_isl_type.to_owned());

        let mut found_base_type = false;
        for constraint in constraints {
            match constraint.constraint() {
                IslConstraintValue::Type(isl_type) => {
                    let type_name = self.handle_duplicate_constraint(
                        found_base_type,
                        "type",
                        isl_type,
                        FieldPresence::Required,
                        code_gen_context,
                    )?;
                    wrapped_scalar_builder.base_type(type_name);
                    found_base_type = true;
                }
                IslConstraintValue::ContainerLength(_) => {
                    // TODO: add support for container length
                    // this is currently not supported and is a no-op
                }
                _ => {
                    return invalid_abstract_data_type_error(
                        "Could not determine the abstract data type due to conflicting constraints",
                    );
                }
            }
        }

        Ok(AbstractDataType::WrappedScalar(
            wrapped_scalar_builder.build()?,
        ))
    }

    /// Builds `AbstractDataType::Scalar` from the given constraints.
    /// ```
    /// { type: string }
    /// ```
    /// This method builds `AbstractDataType`as following:
    /// ```
    /// AbstractDataType::Scalar(
    ///  Scalar {
    ///     base_type: FullyQualifiedTypeReference { type_name: vec!["String"], parameters: vec![] }
    ///     doc_comment: None // There is no doc comment defined in above ISL type def
    ///     source: IslType { .. } // Represents the `IslType` that is getting converted to `AbstractDataType`
    ///  }
    /// )
    /// ```
    ///
    /// _Note: Currently code generator would return an error when there are multiple `type` constraints in the type definition.
    /// This avoids providing conflicting type constraints in the type definition._
    fn build_scalar_from_constraints(
        &mut self,
        constraints: &[IslConstraint],
        code_gen_context: &mut CodeGenContext,
        parent_isl_type: &IslType,
    ) -> CodeGenResult<AbstractDataType> {
        let mut scalar_builder = ScalarBuilder::default();
        scalar_builder.source(parent_isl_type.to_owned());

        let mut found_base_type = false;
        for constraint in constraints {
            match constraint.constraint() {
                IslConstraintValue::Type(isl_type) => {
                    let type_name = self.handle_duplicate_constraint(
                        found_base_type,
                        "type",
                        isl_type,
                        FieldPresence::Required,
                        code_gen_context,
                    )?;
                    scalar_builder.base_type(type_name);
                    found_base_type = true;
                }
                _ => {
                    return invalid_abstract_data_type_error(
                        "Could not determine the abstract data type due to conflicting constraints",
                    );
                }
            }
        }

        Ok(AbstractDataType::Scalar(scalar_builder.build()?))
    }

    /// Builds `AbstractDataType::WrappedSequence` from the given constraints.
    /// ```
    /// type::{
    ///   name: foo,
    ///   type: list,
    ///   element: string,
    /// }
    /// ```
    /// This method builds `AbstractDataType`as following:
    /// ```
    /// AbstractDataType::WrappedSequence(
    ///  WrappedSequence {
    ///     name: vec!["org", "example", "Foo"] // assuming the namespace here is `org.example`
    ///     element_type: FullyQualifiedTypeReference { type_name: vec!["String"], parameters: vec![] } // Represents the element type for the list
    ///     sequence_type: SequenceType::List, // Represents list type for the given sequence
    ///     doc_comment: None // There is no doc comment defined in above ISL type def
    ///     source: IslType { .. } // Represents the `IslType` that is getting converted to `AbstractDataType`
    ///  }
    /// )
    /// ```
    fn build_wrapped_sequence_from_constraints(
        &mut self,
        constraints: &[IslConstraint],
        code_gen_context: &mut CodeGenContext,
        parent_isl_type: &IslType,
    ) -> CodeGenResult<AbstractDataType> {
        let mut wrapped_sequence_builder = WrappedSequenceBuilder::default();
        wrapped_sequence_builder
            .name(self.current_type_fully_qualified_name.to_owned())
            .source(parent_isl_type.to_owned());
        let mut found_base_type = false;
        let mut found_element_constraint = false;
        for constraint in constraints {
            match constraint.constraint() {
                IslConstraintValue::Element(isl_type_ref, _) => {
                    let type_name = self.handle_duplicate_constraint(
                        found_element_constraint,
                        "type",
                        isl_type_ref,
                        FieldPresence::Required,
                        code_gen_context,
                    )?;

                    wrapped_sequence_builder.element_type(type_name);
                    found_element_constraint = true;
                }
                IslConstraintValue::Type(isl_type_ref) => {
                    if found_base_type {
                        return invalid_abstract_data_type_error(
                            "Multiple `type` constraints in the type definitions are not supported in code generation as it can lead to conflicting types."
                        );
                    }
                    if isl_type_ref.name() == "sexp" {
                        wrapped_sequence_builder.sequence_type(SequenceType::SExp);
                    } else if isl_type_ref.name() == "list" {
                        wrapped_sequence_builder.sequence_type(SequenceType::List);
                    }
                    found_base_type = true;
                }
                IslConstraintValue::ContainerLength(_) => {
                    // TODO: add support for container length
                    // this is currently not supported and is a no-op
                }
                _ => {
                    return invalid_abstract_data_type_error(
                        "Could not determine the abstract data type due to conflicting constraints",
                    );
                }
            }
        }
        Ok(AbstractDataType::WrappedSequence(
            wrapped_sequence_builder.build()?,
        ))
    }

    /// Builds `AbstractDataType::Sequence` from the given constraints.
    /// ```
    /// {
    ///   type: list,
    ///   element: string,
    /// }
    /// ```
    /// This method builds `AbstractDataType`as following:
    /// ```
    /// AbstractDataType::Sequence(
    ///  Sequence {
    ///     element_type: FullyQualifiedTypeReference { type_name: vec!["String"], parameters: vec![] } // Represents the element type for the list
    ///     sequence_type: SequenceType::List, // Represents list type for the given sequence
    ///     doc_comment: None // There is no doc comment defined in above ISL type def
    ///     source: IslType { .. } // Represents the `IslType` that is getting converted to `AbstractDataType`
    ///  }
    /// )
    /// ```
    fn build_sequence_from_constraints(
        &mut self,
        constraints: &[IslConstraint],
        code_gen_context: &mut CodeGenContext,
        parent_isl_type: &IslType,
    ) -> CodeGenResult<AbstractDataType> {
        let mut sequence_builder = SequenceBuilder::default();
        sequence_builder.source(parent_isl_type.to_owned());
        for constraint in constraints {
            match constraint.constraint() {
                IslConstraintValue::Element(isl_type_ref, _) => {
                    let type_name = self
                        .fully_qualified_type_ref_name(
                            isl_type_ref,
                            FieldPresence::Required,
                            code_gen_context,
                        )?
                        .ok_or(invalid_abstract_data_type_raw_error(format!(
                            "Could not determine `FullQualifiedTypeReference` for type {:?}",
                            isl_type_ref
                        )))?;

                    sequence_builder.element_type(type_name);
                }
                IslConstraintValue::Type(isl_type_ref) => {
                    if isl_type_ref.name() == "sexp" {
                        sequence_builder.sequence_type(SequenceType::SExp);
                    } else if isl_type_ref.name() == "list" {
                        sequence_builder.sequence_type(SequenceType::List);
                    }
                }
                IslConstraintValue::ContainerLength(_) => {
                    // TODO: add support for container length
                    // this is currently not supported and is a no-op
                }
                _ => {
                    return invalid_abstract_data_type_error(
                        "Could not determine the abstract data type due to conflicting constraints",
                    );
                }
            }
        }
        Ok(AbstractDataType::Sequence(sequence_builder.build()?))
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
            FieldPresence::Required,
            &isl_type,
            &mut CodeGenContext::new(),
            false,
        )?;
        let abstract_data_type = data_model_node.code_gen_type.unwrap();
        assert_eq!(
            abstract_data_type
                .fully_qualified_type_ref::<JavaLanguage>()
                .unwrap(),
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
                                type_name: vec![
                                    "java".to_string(),
                                    "util".to_string(),
                                    "Optional".to_string()
                                ],
                                parameters: vec![FullyQualifiedTypeReference {
                                    type_name: vec!["String".to_string()],
                                    parameters: vec![]
                                }]
                            },
                            FieldPresence::Optional
                        )
                    ),
                    (
                        "bar".to_string(),
                        FieldReference(
                            FullyQualifiedTypeReference {
                                type_name: vec![
                                    "java".to_string(),
                                    "util".to_string(),
                                    "Optional".to_string()
                                ],
                                parameters: vec![FullyQualifiedTypeReference {
                                    type_name: vec!["Integer".to_string()],
                                    parameters: vec![]
                                }]
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
                            type: struct,
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
            FieldPresence::Required,
            &isl_type,
            &mut CodeGenContext::new(),
            false,
        )?;
        let abstract_data_type = data_model_node.code_gen_type.unwrap();
        assert_eq!(
            abstract_data_type
                .fully_qualified_type_ref::<JavaLanguage>()
                .unwrap(),
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
                                    "java".to_string(),
                                    "util".to_string(),
                                    "Optional".to_string()
                                ],
                                parameters: vec![FullyQualifiedTypeReference {
                                    type_name: vec![
                                        "org".to_string(),
                                        "example".to_string(),
                                        "MyNestedStruct".to_string(),
                                        "NestedType1".to_string()
                                    ],
                                    parameters: vec![]
                                }]
                            },
                            FieldPresence::Optional
                        )
                    ),
                    (
                        "bar".to_string(),
                        FieldReference(
                            FullyQualifiedTypeReference {
                                type_name: vec![
                                    "java".to_string(),
                                    "util".to_string(),
                                    "Optional".to_string()
                                ],
                                parameters: vec![FullyQualifiedTypeReference {
                                    type_name: vec!["Integer".to_string()],
                                    parameters: vec![]
                                }]
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
                    .fully_qualified_type_ref::<JavaLanguage>(),
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
