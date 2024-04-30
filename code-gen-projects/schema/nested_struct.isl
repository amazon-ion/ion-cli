type::{
 name: nested_struct,
 fields: {
    A: string,
    B: int,
    C: {
        fields: {
            D: bool,
            E: { element: int } // default sequence type is `list`
        }
    }
 }
}