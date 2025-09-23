use crate::commands::jq::JaqElement;
use ion_rs::{Element, IonType};
use jaq_core::{box_iter::box_once, Native, RunPtr, ValT};
use jaq_std::Filter;

/// Helper to create error for invalid input type
fn input_error(expected: &str) -> jaq_core::Exn<'_, JaqElement> {
    jaq_core::Error::str(format!("{} filter requires a string input", expected)).into()
}

/// Ion-specific jq function definitions (filters implemented as definitions)
pub fn ion_defs() -> impl Iterator<Item = jaq_core::load::parse::Def<&'static str>> {
    const ION_DEFS: &str = r#"
# Ion type predicates
def istimestamp: ion_type == "timestamp";
def issexp: ion_type == "sexp";
def issymbol: ion_type == "symbol";
def isdecimal: ion_type == "decimal";

# Ion type selectors
def timestamps: select(istimestamp);
def sexps: select(issexp);
def symbols: select(issymbol);
def decimals: select(isdecimal);

# Ion conversion helpers
def to_symbol: if type == "string" then symbol else error("to_symbol requires string input") end;
def to_sexp: if type == "array" then sexp else error("to_sexp requires array input") end;
def to_timestamp: if type == "string" then timestamp else error("to_timestamp requires string input") end;
"#;

    jaq_core::load::parse(ION_DEFS, |p| p.defs())
        .unwrap_or_default()
        .into_iter()
}

/// Ion-specific native jq functions
pub fn ion_funs() -> impl Iterator<Item = Filter<Native<JaqElement>>> {
    [timestamp_fn(), sexp_fn(), symbol_fn(), ion_type_fn()].into_iter()
}

/// Creates a timestamp from a string
fn timestamp_fn() -> Filter<Native<JaqElement>> {
    let run: RunPtr<JaqElement> = |_, (_, v)| match v.as_str() {
        Some(s) => match Element::read_one(s.as_bytes()) {
            Ok(element) if element.ion_type() == IonType::Timestamp => {
                box_once(Ok(JaqElement::from(element)))
            }
            _ => box_once(Err(jaq_core::Error::str("invalid timestamp format").into())),
        },
        None => box_once(Err(input_error("timestamp"))),
    };

    ("timestamp", Box::new([]), Native::new(run))
}

/// Creates an S-expression from an array  
fn sexp_fn() -> Filter<Native<JaqElement>> {
    let run: RunPtr<JaqElement> = |_, (_, v)| match v.values().collect::<Result<Vec<_>, _>>() {
        Ok(items) => {
            let elements: Vec<Element> = items.into_iter().map(|je| je.into_inner()).collect();
            box_once(Ok(JaqElement::from(Element::from(
                ion_rs::SExp::from_iter(elements),
            ))))
        }
        Err(e) => box_once(Err(e.into())),
    };

    ("sexp", Box::new([]), Native::new(run))
}

/// Creates a symbol from a string
fn symbol_fn() -> Filter<Native<JaqElement>> {
    let run: RunPtr<JaqElement> = |_, (_, v)| match v.as_str() {
        Some(s) => box_once(Ok(JaqElement::from(Element::from(ion_rs::Symbol::from(s))))),
        None => box_once(Err(input_error("symbol"))),
    };

    ("symbol", Box::new([]), Native::new(run))
}

/// Returns the Ion type name as a string
fn ion_type_fn() -> Filter<Native<JaqElement>> {
    let run: RunPtr<JaqElement> = |_, (_, v)| {
        let type_name = format!("{:?}", v.ion_type()).to_lowercase();
        box_once(Ok(JaqElement::from(Element::from(type_name))))
    };

    ("ion_type", Box::new([]), Native::new(run))
}
