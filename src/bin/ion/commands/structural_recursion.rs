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

/// Iteratively applies a mapper to all elements in an Ion structure
/// This function uses an explicit work stack instead of recursion
/// It processes the structure in post-order (children before parents) to enable reconstruction
pub fn map_structure<M: ElementMapper>(root: Element, mapper: &M) -> Result<Element> {
    enum WorkItem {
        Process(Element), // Process a single element (apply mapper or recurse into children)
        BuildList(usize),
        BuildStruct(Vec<String>),
    }

    let mut stack = vec![WorkItem::Process(root)];
    let mut results = Vec::new(); // Store processed elements in post-order

    while let Some(item) = stack.pop() {
        match item {
            WorkItem::Process(element) => {
                match element.ion_type() {
                    IonType::List => {
                        // For lists, collect all elements, then push reconstruction work
                        let list = element.as_sequence().unwrap();
                        let elements: Vec<_> = list.elements().cloned().collect();
                        stack.push(WorkItem::BuildList(elements.len()));
                        for elem in elements.into_iter().rev() {
                            stack.push(WorkItem::Process(elem));
                        }
                    }
                    IonType::Struct => {
                        // For structs, collect field names & values, then push reconstruction work
                        let struct_val = element.as_struct().unwrap();
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
                        let mapped = mapper.map(element)?;
                        results.push(mapped);
                    }
                }
            }
            WorkItem::BuildList(size) => {
                let elements = results.split_off(results.len() - size);
                results.push(Element::from(ion_rs::List::from(elements)));
            }
            WorkItem::BuildStruct(field_names) => {
                // Take the last `field_names.len()` elements & build a struct
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
