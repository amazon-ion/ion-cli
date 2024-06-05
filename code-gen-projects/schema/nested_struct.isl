type::{
 name: nested_struct,
 type: struct,
 fields: {
    A: string,
    B: int,
    C: {
        type: struct,
        fields: {
            D: bool,
            E: { type: list, element: int }
        }
    }
 }
}