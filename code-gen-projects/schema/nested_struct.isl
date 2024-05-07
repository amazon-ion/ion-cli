type::{
 name: nested_struct,
 fields: {
    A: string,
    B: int,
    C: {
        fields: {
            D: bool,
            E: { type: list, element: int }
        }
    }
 }
}