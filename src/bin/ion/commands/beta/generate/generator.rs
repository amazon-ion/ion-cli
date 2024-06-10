use crate::commands::beta::generate::context::{AbstractDataType, CodeGenContext, SequenceType};
use crate::commands::beta::generate::result::{invalid_abstract_data_type_error, CodeGenResult};
use crate::commands::beta::generate::templates;
use crate::commands::beta::generate::utils::{
    Field, JavaLanguage, Language, NestedType, RustLanguage,
};
use crate::commands::beta::generate::utils::{IonSchemaType, Template};
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
    namespace: Option<&'a str>,
    // Represents a counter for naming nested type definitions
    pub(crate) nested_type_counter: usize,
    phantom: PhantomData<L>,
}

impl<'a> CodeGenerator<'a, RustLanguage> {
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
            namespace: None,
            nested_type_counter: 0,
            tera,
            phantom: PhantomData,
        }
    }
}

impl<'a> CodeGenerator<'a, JavaLanguage> {
    pub fn new(output: &'a Path, namespace: &'a str) -> CodeGenerator<'a, JavaLanguage> {
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
            namespace: Some(namespace),
            nested_type_counter: 0,
            tera,
            phantom: PhantomData,
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
            value.as_str().ok_or(tera::Error::msg(
                "`is_built_in_type` called with non-String Value",
            ))?,
        )))
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

        // Iterate through the ISL types, generate an abstract data type for each
        for isl_type in schema.types() {
            // unwrap here is safe because all the top-level type definition always has a name
            let isl_type_name = isl_type.name().clone().unwrap();
            self.generate_abstract_data_type(&isl_type_name, isl_type)?;
        }

        Ok(())
    }

    /// generates an nested type that can be part of another type definition.
    /// This will be used by the parent type to add this nested type in its namespace or module.
    fn generate_nested_type(
        &mut self,
        type_name: &String,
        isl_type: &IslType,
        nested_types: &mut Vec<NestedType>,
    ) -> CodeGenResult<()> {
        // Add an object called `nested_types` in tera context
        // This will have a list of `nested_type` where each will include fields, a target_kind_name and abstract_data_type
        let mut tera_fields = vec![];
        let mut code_gen_context = CodeGenContext::new();
        let mut nested_anonymous_types = vec![];
        let constraints = isl_type.constraints();
        for constraint in constraints {
            self.map_constraint_to_abstract_data_type(
                &mut nested_anonymous_types,
                &mut tera_fields,
                constraint,
                &mut code_gen_context,
            )?;
        }

        // TODO: verify the `occurs` value within a field, by default the fields are optional.
        if let Some(abstract_data_type) = &code_gen_context.abstract_data_type {
            // Add the nested type into parent type's tera context
            nested_types.push(NestedType {
                target_kind_name: type_name.to_case(Case::UpperCamel),
                fields: tera_fields,
                abstract_data_type: abstract_data_type.to_owned(),
                nested_types: nested_anonymous_types,
            });
        } else {
            return invalid_abstract_data_type_error(
                "Can not determine abstract data type, specified constraints do not map to an abstract data type.",
            );
        }

        Ok(())
    }

    fn generate_abstract_data_type(
        &mut self,
        isl_type_name: &String,
        isl_type: &IslType,
    ) -> CodeGenResult<()> {
        let mut context = Context::new();
        let mut tera_fields = vec![];
        let mut code_gen_context = CodeGenContext::new();
        let mut nested_types = vec![];

        // Set the ISL type name for the generated abstract data type
        context.insert("target_kind_name", &isl_type_name.to_case(Case::UpperCamel));

        let constraints = isl_type.constraints();
        for constraint in constraints {
            self.map_constraint_to_abstract_data_type(
                &mut nested_types,
                &mut tera_fields,
                constraint,
                &mut code_gen_context,
            )?;
        }

        // if any field in `tera_fields` contains a `None` `value_type` then it means there is a constraint that leads to open ended types.
        // Return error in such case.
        if tera_fields
            .iter()
            .any(|Field { value_type, .. }| value_type.is_none())
        {
            return invalid_abstract_data_type_error("Currently code generation does not support open ended types. \
            Error can be due to a missing `type` or `fields` or `element` constraint in the type definition.");
        }

        // add fields for template
        // TODO: verify the `occurs` value within a field, by default the fields are optional.
        if let Some(abstract_data_type) = &code_gen_context.abstract_data_type {
            context.insert("fields", &tera_fields);
            context.insert("abstract_data_type", abstract_data_type);
            context.insert("nested_types", &nested_types);
        } else {
            return invalid_abstract_data_type_error(
                    "Can not determine abstract data type, specified constraints do not map to an abstract data type.",
                );
        }

        self.render_generated_code(isl_type_name, &mut context, &mut code_gen_context)
    }

    fn render_generated_code(
        &mut self,
        type_name: &str,
        context: &mut Context,
        code_gen_context: &mut CodeGenContext,
    ) -> CodeGenResult<()> {
        // Add namespace to tera context
        if let Some(namespace) = self.namespace {
            context.insert("namespace", namespace);
        }
        // Render or generate file for the template with the given context
        let template: &Template = &code_gen_context.abstract_data_type.as_ref().try_into()?;
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

    /// Provides name of the type reference that will be used for generated abstract data type
    fn type_reference_name(
        &mut self,
        isl_type_ref: &IslTypeRef,
        nested_types: &mut Vec<NestedType>,
    ) -> CodeGenResult<Option<String>> {
        Ok(match isl_type_ref {
            IslTypeRef::Named(name, _) => {
                let schema_type: IonSchemaType = name.into();
                L::target_type(&schema_type)
            }
            IslTypeRef::TypeImport(_, _) => {
                unimplemented!("Imports in schema are not supported yet!");
            }
            IslTypeRef::Anonymous(type_def, _) => {
                let name = self.next_nested_type_name();
                self.generate_nested_type(&name, type_def, nested_types)?;

                Some(name)
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
        nested_types: &mut Vec<NestedType>,
        tera_fields: &mut Vec<Field>,
        constraint: &IslConstraint,
        code_gen_context: &mut CodeGenContext,
    ) -> CodeGenResult<()> {
        match constraint.constraint() {
            IslConstraintValue::Element(isl_type, _) => {
                let type_name = self.type_reference_name(isl_type, nested_types)?;

                self.verify_and_update_abstract_data_type(
                    AbstractDataType::Sequence {
                        element_type: type_name.to_owned(),
                        sequence_type: None,
                    },
                    tera_fields,
                    code_gen_context,
                )?;

                // Verify that the current type doesn't contains any nested types and that they are also of sequence or scalar type.
                // if found nested sequence/scalar types then remove them from nested types and set the sequence or scalar as a field in current class/struct.
                if let Some(type_reference_name) = &type_name {
                    if type_reference_name.contains("NestedType") {
                        // This is a nested type. Check for the abstract data type. If it is sequence type or scalar type,
                        // then add them into the current tera fields and remove them from `nested_types`. Scalar and sequence types
                        // doesn't need to have a separate class/struct created for them.
                        if let Some(nested_type) = nested_types.get_mut(0) {
                            if matches!(
                                nested_type.abstract_data_type,
                                AbstractDataType::Sequence { .. }
                            ) || nested_type.abstract_data_type == AbstractDataType::Value
                            {
                                // scalar and sequence types will only have 1 field. The field name here would be
                                // replaced with current `fields` constraint's field name.
                                // But `value_type` and ` isl_type_name` would be based on what we have in the `nested_type`.
                                let field = nested_type.fields.pop().unwrap();
                                self.generate_struct_field(
                                    tera_fields,
                                    L::target_type_as_sequence(&field.value_type),
                                    field.isl_type_name,
                                    "value",
                                    Some(nested_type.abstract_data_type.to_owned()),
                                )?;

                                // change the `element_type` of current AbstractDataType::Sequence { .. }. This should be the type of nested type.
                                if let Some(AbstractDataType::Sequence { sequence_type, .. }) =
                                    &code_gen_context.abstract_data_type
                                {
                                    code_gen_context.abstract_data_type =
                                        Some(AbstractDataType::Sequence {
                                            element_type: field.value_type,
                                            sequence_type: sequence_type.to_owned(),
                                        });
                                }

                                // remove this nested type from the list as it will now be part of this field without generating separate nested type.
                                nested_types.pop();
                                return Ok(());
                            }
                        }
                    }
                }

                // if the abstract data type is a sequence then pass the type name as the updated `element_type`.
                if let Some(AbstractDataType::Sequence {
                    element_type,
                    sequence_type: Some(_),
                }) = &code_gen_context.abstract_data_type
                {
                    self.generate_struct_field(
                        tera_fields,
                        L::target_type_as_sequence(element_type),
                        isl_type.name(),
                        "value",
                        None,
                    )?;
                } else {
                    self.generate_struct_field(tera_fields, None, isl_type.name(), "value", None)?;
                }
            }
            IslConstraintValue::Fields(fields, content_closed) => {
                // TODO: Check for `closed` annotation on fields and based on that return error while reading if there are extra fields.
                self.verify_and_update_abstract_data_type(
                    AbstractDataType::Structure(*content_closed),
                    tera_fields,
                    code_gen_context,
                )?;
                for (name, value) in fields.iter() {
                    let mut type_name =
                        self.type_reference_name(value.type_reference(), nested_types)?;
                    let mut abstract_data_type = None;
                    let mut isl_type_name = value.type_reference().name();

                    if let Some(type_reference_name) = &type_name {
                        if type_reference_name.contains("NestedType") {
                            // This is a nested type. Check for the abstract data type. If it is sequence type or scalar type,
                            // then add them into the current tera fields and remove them from `nested_types`. Scalar and sequence types
                            // doesn't need to have a separate class/struct created for them.
                            if let Some(nested_type) = nested_types.get_mut(0) {
                                if matches!(
                                    nested_type.abstract_data_type,
                                    AbstractDataType::Sequence { .. }
                                ) || nested_type.abstract_data_type == AbstractDataType::Value
                                {
                                    // scalar and sequence types will only have 1 field. The field name here would be
                                    // replaced with current `fields` constraint's field name.
                                    // But `value_type` and ` isl_type_name` would be based on what we have in the `nested_type`.
                                    let field = nested_type.fields.pop().unwrap();
                                    abstract_data_type =
                                        Some(nested_type.abstract_data_type.to_owned());
                                    isl_type_name = field.isl_type_name;
                                    type_name = field.value_type;

                                    // remove this nested type from the list as it will now be part of this field without generating separate nested type.
                                    nested_types.pop();
                                }
                            }
                        }
                    }
                    self.generate_struct_field(
                        tera_fields,
                        type_name,
                        isl_type_name,
                        name,
                        abstract_data_type,
                    )?;
                }
            }
            IslConstraintValue::Type(isl_type) => {
                let type_name = self.type_reference_name(isl_type, nested_types)?;

                self.verify_and_update_abstract_data_type(
                    if isl_type.name() == "list" {
                        AbstractDataType::Sequence {
                            element_type: type_name.clone(),
                            sequence_type: Some(SequenceType::List),
                        }
                    } else if isl_type.name() == "sexp" {
                        AbstractDataType::Sequence {
                            element_type: type_name.clone(),
                            sequence_type: Some(SequenceType::SExp),
                        }
                    } else if isl_type.name() == "struct" {
                        AbstractDataType::Structure(false) // by default contents aren't closed
                    } else {
                        AbstractDataType::Value
                    },
                    tera_fields,
                    code_gen_context,
                )?;

                // Verify that the current type doesn't contains any nested types and that they are of sequence or scalar type.
                // if found nested sequence/scalar types then remove them from `nested_types` and set the sequence or scalar as a field in current class/struct.
                if let Some(type_reference_name) = &type_name {
                    if type_reference_name.contains("NestedType") {
                        // This is a nested type. Check for the abstract data type. If it is sequence type or scalar type,
                        // then add them into the current tera fields and remove them from `nested_types`. Scalar and sequence types
                        // doesn't need to have a separate class/struct created for them.
                        if let Some(nested_type) = nested_types.get_mut(0) {
                            if matches!(
                                nested_type.abstract_data_type,
                                AbstractDataType::Sequence { .. }
                            ) || nested_type.abstract_data_type == AbstractDataType::Value
                            {
                                // scalar and sequence types will only have 1 field. The field name here would be
                                // replaced with current `fields` constraint's field name.
                                // But `value_type` and ` isl_type_name` would be based on what we have in the `nested_type`.
                                let field = nested_type.fields.pop().unwrap();
                                self.generate_struct_field(
                                    tera_fields,
                                    field.value_type,
                                    field.isl_type_name,
                                    "value",
                                    Some(nested_type.abstract_data_type.to_owned()),
                                )?;

                                // Update current `abstract_data_type` according to nested type
                                if let AbstractDataType::Sequence {
                                    element_type: nested_element_type,
                                    sequence_type: nested_sequence_type,
                                } = &nested_type.abstract_data_type
                                {
                                    code_gen_context.abstract_data_type =
                                        Some(AbstractDataType::Sequence {
                                            element_type: nested_element_type.to_owned(),
                                            sequence_type: nested_sequence_type.to_owned(),
                                        });
                                }

                                // remove this nested type from the list as it will now be part of this field without generating separate nested type.
                                nested_types.pop();
                                return Ok(());
                            }
                        }
                    }
                }

                // if the abstract data type is a sequence then pass the type name as the updated `element_type`.
                if let Some(AbstractDataType::Sequence { element_type, .. }) =
                    &code_gen_context.abstract_data_type
                {
                    self.generate_struct_field(
                        tera_fields,
                        L::target_type_as_sequence(element_type),
                        isl_type.name(),
                        "value",
                        None,
                    )?;
                } else {
                    self.generate_struct_field(
                        tera_fields,
                        type_name,
                        isl_type.name(),
                        "value",
                        None,
                    )?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Generates a struct field based on field name and value(data type)
    fn generate_struct_field(
        &mut self,
        tera_fields: &mut Vec<Field>,
        abstract_data_type_name: Option<String>,
        isl_type_name: String,
        field_name: &str,
        // This argument is used only for nested sequence type,
        // it will be `None` in all other cases.
        abstract_data_type: Option<AbstractDataType>,
    ) -> CodeGenResult<()> {
        tera_fields.push(Field {
            name: field_name.to_string(),
            value_type: abstract_data_type_name,
            isl_type_name,
            abstract_data_type,
        });
        Ok(())
    }

    /// Verify that the current abstract data type is same as previously determined abstract data type
    /// This is referring to abstract data type determined with each constraint that is verifies
    /// that all the constraints map to a single abstract data type and not different abstract data types.
    /// Also, updates the underlying `element_type` for List and SExp.
    /// e.g.
    /// ```
    /// type::{
    ///   name: foo,
    ///   type: string,
    ///   fields:{
    ///      source: String,
    ///      destination: String
    ///   }
    /// }
    /// ```
    /// For the above schema, both `fields` and `type` constraints map to different abstract data types
    /// respectively Struct(with given fields `source` and `destination`) and Value(with a single field that has String data type).
    fn verify_and_update_abstract_data_type(
        &mut self,
        current_abstract_data_type: AbstractDataType,
        tera_fields: &mut Vec<Field>,
        code_gen_context: &mut CodeGenContext,
    ) -> CodeGenResult<()> {
        if let Some(abstract_data_type) = &code_gen_context.abstract_data_type {
            match abstract_data_type {
                // In the case when a `type` constraint occurs before `element` constraint. The element type for the sequence
                // needs to be updated based on `element` constraint whereas sequence type will be used as per `type` constraint.
                // e.g. For a schema as below:
                // ```
                // type::{
                //   name: sequence_type,
                //   type: sexp,
                //   element: string,
                // }
                // ```
                // Here, first `type` constraint would set the `AbstractDataType::Sequence{ element_type: T, sequence_type: "sexp"}`
                // which uses generic type T and sequence type is sexp. Next `element` constraint would
                // set the `AbstractDataType::Sequence{ element_type: String, sequence_type: "list"}`.
                // Now this method performs verification that if the above described case occurs
                // then it updates the `element_type` as per `element` constraint
                // and `sequence_type` as per `type` constraint.
                AbstractDataType::Sequence {
                    element_type,
                    sequence_type,
                } if abstract_data_type != &current_abstract_data_type
                    && (element_type.is_none())
                    && matches!(
                        &current_abstract_data_type,
                        &AbstractDataType::Sequence { .. }
                    ) =>
                {
                    // if current abstract data type is sequence and element_type is generic T or Object,
                    // then this was set by a `type` constraint in sequence field,
                    // so remove all previous fields that allows `Object` and update with current abstract_data_type.
                    tera_fields.pop();
                    code_gen_context.with_abstract_data_type(AbstractDataType::Sequence {
                        element_type: current_abstract_data_type.element_type(),
                        sequence_type: sequence_type.to_owned(),
                    });
                }
                // In the case when a `type` constraint occurs before `element` constraint. The element type for the sequence
                // needs to be updated based on `element` constraint whereas sequence type will be used as per `type` constraint.
                // e.g. For a schema as below:
                // ```
                // type::{
                //   name: sequence_type,
                //   element: string,
                //   type: sexp,
                // }
                // ```
                // Here, first `element` constraint would set the `AbstractDataType::Sequence{ element_type: String, sequence_type: "list"}` ,
                // Next `type` constraint would set the `AbstractDataType::Sequence{ element_type: T, sequence_type: "sexp"}`
                // which uses generic type `T` and sequence type is sexp. Now this method performs verification that
                // if the above described case occurs then it updates the `element_type` as per `element` constraint
                // and `sequence_type` as per `type` constraint.
                AbstractDataType::Sequence { element_type, .. }
                    if abstract_data_type != &current_abstract_data_type
                        && (current_abstract_data_type.element_type().is_none())
                        && matches!(
                            &current_abstract_data_type,
                            &AbstractDataType::Sequence { .. }
                        ) =>
                {
                    // if `element` constraint has already set the abstract data_type to `Sequence`
                    // then remove previous fields as new fields will be added again after updating `element_type`.
                    // `type` constraint does update the ISL type name to either `list` or `sexp`,
                    // which needs to be updated within `abstract_data_type` as well.
                    tera_fields.pop();
                    code_gen_context.with_abstract_data_type(AbstractDataType::Sequence {
                        element_type: element_type.to_owned(),
                        sequence_type: current_abstract_data_type.sequence_type(),
                    })
                }
                // In the case when a `type` constraint occurs before `fields` constraint. The `content_closed` property for the struct
                // needs to be updated based on `fields` constraint.
                // e.g. For a schema as below:
                // ```
                // type::{
                //   name: struct_type,
                //   type: struct,
                //   fields: {}
                //      foo: string
                //   },
                // }
                // ```
                // Here, first `type` constraint would set tera_fields with `value_type: None` and with `fields` constraint this field should be popped,
                // and modify the `content_closed` property as per `fields` constraint.
                AbstractDataType::Structure(_)
                    if !tera_fields.is_empty()
                        && tera_fields[0].value_type.is_none()
                        && matches!(
                            &current_abstract_data_type,
                            &AbstractDataType::Structure(_)
                        ) =>
                {
                    tera_fields.pop();
                    // unwrap here is safe because we know the current_abstract_data_type is a `Structure`
                    code_gen_context.with_abstract_data_type(AbstractDataType::Structure(
                        current_abstract_data_type.is_content_closed().unwrap(),
                    ))
                }
                _ if abstract_data_type != &current_abstract_data_type => {
                    return invalid_abstract_data_type_error(format!("Can not determine abstract data type as current constraint {} conflicts with prior constraints for {}.", current_abstract_data_type, abstract_data_type));
                }
                _ => {}
            }
        } else {
            code_gen_context.with_abstract_data_type(current_abstract_data_type);
        }
        Ok(())
    }
}
