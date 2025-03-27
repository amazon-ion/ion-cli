use crate::commands::{CommandIo, IonCliCommand, WithIonCliArgument};
use crate::input::CommandInput;
use crate::output::CommandOutput;
use anyhow::bail;
use clap::{Arg, ArgMatches, Command};
use ion_rs::{AnyEncoding, Element, ElementReader, IonData, List, Reader, Sequence};
use jaq_core::path::Opt;
use jaq_core::val::Range;
use jaq_core::{Filter, Native, RcIter, ValR, ValX};
use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::ops::{Add, Deref, Div, Mul, Neg, Rem, Sub};
use std::str::FromStr;

pub struct JqCommand;

impl IonCliCommand for JqCommand {
    fn is_stable(&self) -> bool {
        false
    }

    fn is_porcelain(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "jq"
    }

    fn about(&self) -> &'static str {
        "A version of `jq` extended to support Ion streams. (See: jqlang.org for details.)"
    }

    fn configure_args(&self, command: Command) -> Command {
        command
            .with_input()
            .with_output()
            .with_format()
            .with_ion_version()
            .arg(
                Arg::new("expr")
                    .long("expr")
                    .short('e')
                    .default_value(".[]")
                    .help("A `jq` expression to evaluate"),
            )
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> anyhow::Result<()> {
        let jq_expr = args.get_one::<String>("expr").unwrap().as_str();
        let filter = compile_jq_filter(jq_expr);

        CommandIo::new(args)?.for_each_input(|output, input| {
            let _format = output.format();
            let _encoding = output.encoding();
            evaluate_jq_expr(&filter, input, output)?;
            Ok(())
        })
    }
}

fn compile_jq_filter(jq_expr:  &str) -> Filter<Native<JaqElement>> {
    use jaq_core::load::{Arena, File, Loader};
    let program = File {
        code: jq_expr, // a jq expression like ".[]"
        path: (),      // For error reporting, but not currently used by this program
    };

    // If we wanted to define our own Ion-centric stdlib methods, we'd do something like:
    //    Loader::new(jaq_std::defs().chain(jaq_ion::defs()))
    let loader = Loader::new(jaq_std::defs());
    let arena = Arena::default();

    // parse the filter
    let modules = loader.load(&arena, program).unwrap();

    // compile the filter
    jaq_core::Compiler::default()
        // Similar to `defs()` above, this would be our opportunity to extend the built-in filters
        .with_funs(jaq_std::funs::<JaqElement>())
        .compile(modules)
        .unwrap()
}

fn evaluate_jq_expr(
    filter: &Filter<Native<JaqElement>>,
    input: CommandInput,
    output: &mut CommandOutput,
) -> anyhow::Result<()> {

    let mut reader = Reader::new(AnyEncoding, input.into_source())?;
    let input_elements = reader.read_all_elements()?;
    let ion_stream_as_element = List::from(input_elements).into();

    let inputs = RcIter::new(core::iter::empty());
    // iterator over the output values
    let out = filter.run((jaq_core::Ctx::new([], &inputs), ion_stream_as_element));

    let mut writer = output.as_writer()?;
    for value in out {
        match value {
            Ok(element) => {
                writer.write(&element.0)?;
            }
            Err(e) => {
                bail!("jq processing failed: {e}");
            }
        }
    }
    writer.close()?;
    Ok(())
}

/// Wraps an `Element` so we can:
///  1. Define implementations of common traits like `Ord` and `Eq` without `Element` itself needing to.
///  2. Keep all logic related to `jq` behavior in one place.
#[derive(Clone, Eq, Debug)]
struct JaqElement(Element);

// Anything that can be turned into an Element can also be turned into a JaqElement
impl<T> From<T> for JaqElement
where
    Element: From<T>,
{
    fn from(value: T) -> Self {
        let element: Element = value.into();
        JaqElement(element)
    }
}

// When we have a JaqElement, we can call methods on the underlying Element
impl Deref for JaqElement {
    type Target = Element;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// jaq expects errors to include a value of the same type as Ok(value), but the error value
// may represent an error message, stack trace, or something else. In this impl, it's currently
// used to return an error message.
type JaqError = jaq_core::Error<JaqElement>;

// Convenience method to return a `jaq_core::ValR` (value result) with an error.
fn jaq_error(e: impl Into<Element>) -> ValR<JaqElement> {
    Err(jaq_err(e))
}

// Convenience method to return a bare `JaqError`, not wrapped in a Result::Err.
// This is useful inside closures like `ok_or_else`.
fn jaq_err(e: impl Into<Element>) -> JaqError {
    JaqError::new(e.into().into())
}

impl FromIterator<Self> for JaqElement {
    fn from_iter<T: IntoIterator<Item = Self>>(iter: T) -> Self {
        let items = Sequence::from_iter(iter.into_iter().map(|je| je.0));
        let element = Element::from(List::from_iter(items));
        JaqElement(element)
    }
}

impl PartialEq<Self> for JaqElement {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl PartialOrd for JaqElement {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(IonData::from(self).cmp(&IonData::from(other)))
    }
}

// === Math operator behaviors ===

impl Add for JaqElement {
    type Output = ValR<Self>;

    fn add(self, _rhs: Self) -> Self::Output {
        todo!()
    }
}

impl Sub for JaqElement {
    type Output = ValR<Self>;

    fn sub(self, _rhs: Self) -> Self::Output {
        todo!()
    }
}

impl Mul for JaqElement {
    type Output = ValR<Self>;

    fn mul(self, _rhs: Self) -> Self::Output {
        todo!()
    }
}

impl Div for JaqElement {
    type Output = ValR<Self>;

    fn div(self, _rhs: Self) -> Self::Output {
        todo!()
    }
}

impl Rem for JaqElement {
    type Output = ValR<Self>;

    fn rem(self, _rhs: Self) -> Self::Output {
        todo!()
    }
}

impl Neg for JaqElement {
    type Output = ValR<Self>;

    fn neg(self) -> Self::Output {
        todo!()
    }
}

impl Display for JaqElement {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl jaq_core::ValT for JaqElement {
    // Going from numeric text to an Element
    fn from_num(n: &str) -> ValR<Self> {
        match f64::from_str(n) {
            Ok(f) => Ok(Element::from(f).into()),
            Err(_) => jaq_error(format!("invalid number: {n}")),
        }
    }

    // Given a sequence of (name, value) pairs, make a 'Map' (or in our case: struct)
    fn from_map<I: IntoIterator<Item = (Self, Self)>>(iter: I) -> ValR<Self> {
        let mut strukt = ion_rs::Struct::builder();
        for (name, value) in iter {
            let field_name = name.expect_text().map_err(|_| {
                jaq_err(format!(
                    "struct field names must be symbols or strings, found {name:?}"
                ))
            })?;
            strukt = strukt.with_field(field_name, value.0);
        }

        Ok(strukt.build().into())
    }

    // Iterate over the child values of `self`.
    fn values(self) -> Box<dyn Iterator<Item = ValR<Self>>> {
        use ion_rs::Value::*;
        match self.0.into_value() {
            List(seq) | SExp(seq) => Box::new(seq.into_iter().map(JaqElement::from).map(Ok)),
            Struct(strukt) => Box::new(strukt.into_iter().map(|(_, v)| Ok(JaqElement::from(v)))),
            _ => Box::new(std::iter::empty()),
        }
    }

    // Get the child value corresponding to the given index Element.
    fn index(self, index: &Self) -> ValR<Self> {
        use ion_rs::Value::*;

        match (self.value(), index.value()) {
            (List(seq) | SExp(seq), Int(i)) => {
                let index = i
                    .expect_usize()
                    .map_err(|_| jaq_err("index must be usize"))?;
                let element = seq
                    .get(index)
                    .ok_or_else(|| jaq_err("index out of bounds"))?;
                Ok(JaqElement::from(element.to_owned()))
            }
            (Struct(strukt), String(name)) => strukt
                .get(name)
                .ok_or_else(|| jaq_err(format!("field name '{name}' not found")))
                .map(Element::to_owned)
                .map(JaqElement::from),
            (Struct(strukt), Symbol(name)) => strukt
                .get(name)
                .ok_or_else(|| jaq_err(format!("field name '{name}' not found")))
                .map(Element::to_owned)
                .map(JaqElement::from),
            (Struct(_), Int(i)) => jaq_error(format!("cannot index struct with number ({i})")),
            _ => jaq_error(format!("cannot index into {self:?}")),
        }
    }

    // Behavior for slicing containers.
    fn range(self, _range: Range<&Self>) -> ValR<Self> {
        todo!()
    }

    // Map a function over `self`'s child values
    fn map_values<'a, I: Iterator<Item = ValX<'a, Self>>>(
        self,
        _opt: Opt,
        _f: impl Fn(Self) -> I,
    ) -> ValX<'a, Self> {
        todo!()
    }

    // Map a function over the child value found at the given index
    fn map_index<'a, I: Iterator<Item = ValX<'a, Self>>>(
        self,
        _index: &Self,
        _opt: Opt,
        _f: impl Fn(Self) -> I,
    ) -> ValX<'a, Self> {
        todo!()
    }

    // Map a function over a range of child values
    fn map_range<'a, I: Iterator<Item = ValX<'a, Self>>>(
        self,
        _range: Range<&Self>,
        _opt: Opt,
        _f: impl Fn(Self) -> I,
    ) -> ValX<'a, Self> {
        todo!()
    }

    // If we want "truthiness" for containers (e.g. empty list -> false), define that here
    fn as_bool(&self) -> bool {
        self.0.as_bool().unwrap_or(false)
    }

    // If the element is a text value, return its text.
    fn as_str(&self) -> Option<&str> {
        self.as_text()
    }
}

impl Ord for JaqElement {
    fn cmp(&self, other: &Self) -> Ordering {
        IonData::from(self).cmp(&IonData::from(other))
    }
}

impl jaq_std::ValT for JaqElement {
    fn into_seq<S: FromIterator<Self>>(self) -> Result<S, Self> {
        todo!()
    }

    fn as_isize(&self) -> Option<isize> {
        todo!()
    }

    fn as_f64(&self) -> Result<f64, jaq_core::Error<Self>> {
        todo!()
    }
}
