schema_header::{
    imports: [
        { id: "utils/fruits.isl", type: fruits }
    ]
}

type::{
    name: sequence_with_import,
    type: list,
    element: fruits
}

schema_footer::{}