type::{
 name: struct_with_enum_fields,
 type: struct,
 fields: {
    A: string,
    B: int,
    C: { element: string, type: sexp, occurs: required },
    D: float,
    E: { valid_values: [foo, bar, baz] }
 }
}

