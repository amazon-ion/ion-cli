use anyhow::Result;
use ion_rs::{AnyEncoding, Element, IonType, LazyValue, ValueRef};

/// Trait for operations that transform Ion elements,
/// used for transformations like timestamp conversions
pub trait ElementMapper {
    fn map(&self, element: Element) -> Result<Element>;
}

/// Trait for operations that analyze Ion values without transformation,
/// used for analysis like depth calculation that only need to examine values
pub trait ValueVisitor<T> {
    fn visit(&mut self, value: ValueRef<AnyEncoding>, depth: usize) -> Result<()>;
    fn result(self) -> T;
}

/// Iteratively applies a mapper to all elements in an Ion structure using pre-order traversal
///
/// Pre-order means the mapper is applied to each element BEFORE descending into its children.
/// This enables early termination and structural mutations that post-order cannot handle efficiently.
pub fn map_structure<M: ElementMapper>(root: Element, mapper: &M) -> Result<Element> {
    enum WorkItem {
        Process(Element),         // Apply mapper to element, then decide whether to descend
        BuildList(usize),         // Reconstruct list from the next size results
        BuildStruct(Vec<String>), // Reconstruct struct using field names and next size results
    }

    let mut stack = vec![WorkItem::Process(root)];
    let mut results = Vec::new();

    while let Some(item) = stack.pop() {
        match item {
            WorkItem::Process(element) => {
                let mapped = mapper.map(element)?; // Applying mapper first
                match mapped.ion_type() {
                    IonType::List => {
                        let list = mapped.as_sequence().unwrap(); // Mapper returned a list, now processing its children
                        let children: Vec<_> = list.elements().cloned().collect();
                        stack.push(WorkItem::BuildList(children.len()));
                        for child in children.into_iter().rev() {
                            // Push children in reverse order
                            stack.push(WorkItem::Process(child));
                        }
                    }
                    IonType::Struct => {
                        let struct_val = mapped.as_struct().unwrap();
                        let fields: Vec<_> = struct_val.fields().collect();
                        let field_names: Vec<_> = fields
                            .iter()
                            .map(|(k, _)| k.text().unwrap().to_string())
                            .collect();
                        stack.push(WorkItem::BuildStruct(field_names));
                        for (_, value) in fields.into_iter().rev() {
                            stack.push(WorkItem::Process(value.clone()));
                        }
                    }
                    _ => {
                        results.push(mapped);
                    }
                }
            }
            WorkItem::BuildList(size) => {
                // Reconstructing the list from the last size processed results
                let elements = results.split_off(results.len() - size);
                results.push(Element::from(ion_rs::List::from(elements)));
            }
            WorkItem::BuildStruct(field_names) => {
                // Reconstructing the struct from field names and processed values
                let values = results.split_off(results.len() - field_names.len());
                let mut struct_builder = ion_rs::Struct::builder();
                for (name, value) in field_names.into_iter().zip(values) {
                    struct_builder = struct_builder.with_field(name, value);
                }
                results.push(Element::from(struct_builder.build()));
            }
        }
    }

    Ok(results
        .into_iter()
        .next()
        .unwrap_or_else(|| Element::null(IonType::Null)))
}

/// Iteratively visits all values in an Ion structure for analysis
/// This function performs a depth-first traversal without reconstruction
pub fn visit_structure<V: ValueVisitor<T>, T>(
    root: LazyValue<AnyEncoding>,
    mut visitor: V,
) -> Result<T> {
    let mut stack = vec![(root, 0)];

    while let Some((current_value, depth)) = stack.pop() {
        let value_ref = current_value.read()?;
        visitor.visit(value_ref, depth)?;

        // For container types, add children to the stack with incremented depth
        match value_ref {
            ValueRef::Struct(s) => {
                for field in s {
                    stack.push((field?.value(), depth + 1));
                }
            }
            ValueRef::List(s) => {
                // Add all list elements to stack
                for element in s {
                    stack.push((element?, depth + 1));
                }
            }
            ValueRef::SExp(s) => {
                // Add all s-expression elements to stack
                for element in s {
                    stack.push((element?, depth + 1));
                }
            }
            _ => continue,
        }
    }

    Ok(visitor.result())
}
