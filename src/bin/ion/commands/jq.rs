use crate::commands::jq::ion_math::DecimalMath;
use crate::commands::{CommandIo, IonCliCommand, WithIonCliArgument};
use crate::input::CommandInput;
use crate::output::{CommandOutput, CommandOutputWriter};
use anyhow::bail;
use bigdecimal::ToPrimitive;
use clap::{arg, ArgMatches, Command};
use ion_rs::{
    AnyEncoding, Element, ElementReader, IonData, IonType, List, Reader, Sequence, Value,
};
use itertools::Itertools;
use jaq_core::path::Opt;
use jaq_core::val::Range;
use jaq_core::{Ctx, Filter, Native, RcIter, ValR, ValX};
use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::iter::Empty;
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
            .arg(arg!(<filter> "A `jq` filter expression to evaluate"))
            .arg(arg!(-s --slurp "Read all inputs into an array and use it as the single input value"))
            .with_input()
            .with_output()
            .with_format()
            .with_ion_version()
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> anyhow::Result<()> {
        let slurp = args.get_flag("slurp");

        let jq_expr = args.get_one::<String>("filter").unwrap().as_str();
        let filter = compile_jq_filter(jq_expr);

        CommandIo::new(args)?.for_each_input(|output, input| {
            let _format = output.format();
            let _encoding = output.encoding();
            evaluate_jq_expr(input, output, &filter, slurp)?;
            Ok(())
        })
    }
}

fn compile_jq_filter(jq_expr: &str) -> Filter<Native<JaqElement>> {
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
    input: CommandInput,
    output: &mut CommandOutput,
    filter: &Filter<Native<JaqElement>>,
    slurp: bool,
) -> anyhow::Result<()> {
    let mut reader = Reader::new(AnyEncoding, input.into_source())?;
    let mut writer = output.as_writer()?;

    if slurp {
        let all_input_elements = reader.read_all_elements()?;
        let slurped = List::from(all_input_elements).into();
        filter_and_print(filter, &mut writer, slurped)?;
    } else {
        for item in reader.elements() {
            let item: JaqElement = item?.into();
            filter_and_print(filter, &mut writer, item)?;
        }
    }

    writer.close()?;
    Ok(())
}

fn filter_and_print(
    filter: &Filter<Native<JaqElement>>,
    writer: &mut CommandOutputWriter,
    item: JaqElement,
) -> anyhow::Result<()> {
    const EMPTY_ITER: RcIter<Empty<Result<JaqElement, String>>> = RcIter::new(core::iter::empty());

    let inputs = &EMPTY_ITER; // filter evaluation starts here, no other contextual inputs exist
    let ctx = Ctx::new([], inputs); // manages variables etc., use one per filter execution
    let out = filter.run((ctx, item));

    for value in out {
        match value {
            Ok(element) => writer.write(&element.0)?,
            Err(e) => bail!("ion jq: {e}"),
        };
    }
    Ok(())
}

/// Wraps an `Element` so we can:
///  1. Define implementations of common traits like `Ord` and `Eq` without `Element` itself needing to.
///  2. Keep all logic related to `jq` behavior in one place.
#[derive(Clone, Eq, Debug)]
struct JaqElement(Element);
//TODO: move to sibling module so that people can't construct this and have to go through 'from'
// this will allow consistent construction/transformation rules e.g. field deduplication

impl JaqElement {
    pub fn into_inner(self) -> Element {
        self.0
    }

    pub fn into_value(self) -> Value {
        self.into_inner().into_value()
    }
}

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

fn jaq_unary_error(a: Value, reason: &str) -> ValR<JaqElement> {
    let alpha = a.ion_type();
    jaq_error(format!("{alpha} ({a}) {reason}"))
}

fn jaq_binary_error(a: Value, b: Value, reason: &str) -> ValR<JaqElement> {
    let (alpha, beta) = (a.ion_type(), b.ion_type());
    jaq_error(format!("{alpha} ({a}) and {beta} ({b}) {reason}"))
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
        JaqElement::from(element)
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

    /// From: https://jqlang.org/manual/#addition
    ///
    /// > The operator `+` takes two filters, applies them both to the same input, and adds the
    /// results together. What "adding" means depends on the types involved:
    /// >
    /// > - Numbers are added by normal arithmetic.
    /// >
    /// > - Arrays are added by being concatenated into a larger array.
    /// >
    /// > - Strings are added by being joined into a larger string.
    /// >
    /// > - Objects are added by merging, that is, inserting all the key-value pairs from both
    /// > objects into a single combined object. If both objects contain a value for the same key,
    /// > the object on the right of the `+` wins. (For recursive merge use the `*` operator.)
    /// >
    /// > `null` can be added to any value, and returns the other value unchanged.
    ///
    /// For Ion values we have slightly different semantics–we haven't yet implemented the
    /// overriding and deduplicating of keys for structs, so structs are simply merged
    fn add(self, _rhs: Self) -> Self::Output {
        let (lhv, rhv) = (self.into_value(), _rhs.into_value());

        use ion_math::{DecimalMath, ToFloat};
        use Value::*;

        let elt: Element = match (lhv, rhv) {
            // jq treats JSON's untyped null as an additive identity, e.g. 0 / "" / [] / {}
            (Null(IonType::Null), a) | (a, Null(IonType::Null)) => a.into(),

            // Typed nulls we must handle differently, we can only add similar types
            (Null(a), Null(b)) if a == b => Null(a).into(),
            (Null(a), b) | (b, Null(a)) if a == b.ion_type() => b.into(),

            // Sequences and strings concatenate
            (List(a), List(b)) => ion_rs::List::from_iter(a.into_iter().chain(b)).into(),
            (SExp(a), SExp(b)) => ion_rs::SExp::from_iter(a.into_iter().chain(b)).into(),
            //TODO: Does it make sense to concatenate a String and a Symbol? What type results?
            (String(a), String(b)) => format!("{}{}", a.text(), b.text()).into(),
            (Symbol(a), Symbol(b)) => match (a.text(), b.text()) {
                (Some(ta), Some(tb)) => format!("{}{}", ta, tb),
                //TODO: Handle symbols with unknown text?
                _ => return jaq_binary_error(Symbol(a), Symbol(b), "cannot be added"),
            }
            .into(),

            // Structs merge
            //TODO: Recursively remove duplicate fields, see doc comment for rules
            (Struct(a), Struct(b)) => a.clone_builder().with_fields(b.fields()).build().into(),

            // Number types, only lossless operations
            (Int(a), Int(b)) => (a + b).into(),
            (Float(a), Float(b)) => a.add(b).into(),
            (Decimal(a), Decimal(b)) => a.add(b).into(),
            (Decimal(a), Int(b)) | (Int(b), Decimal(a)) => a.add(b).into(),

            // Only try potentially lossy Float conversions when we've run out of the other options
            (a @ Int(_) | a @ Decimal(_), Float(b)) => (a.to_f64().unwrap() + b).into(),
            (Float(a), b @ Int(_) | b @ Decimal(_)) => (a + b.to_f64().unwrap()).into(),

            (a, b) => return jaq_binary_error(a, b, "cannot be added"),
        };

        Ok(JaqElement::from(elt))
    }
}

impl Sub for JaqElement {
    type Output = ValR<Self>;

    /// From: https://jqlang.org/manual/#subtraction
    ///
    /// > As well as normal arithmetic subtraction on numbers, the `-` operator can be used on
    /// > arrays to remove all occurrences of the second array's elements from the first array.
    fn sub(self, _rhs: Self) -> Self::Output {
        let (lhv, rhv) = (self.into_value(), _rhs.into_value());

        use ion_math::{DecimalMath, ToFloat};
        use Value::*;

        // b.iter.contains() will make these implementations O(N^2).
        // Neither Element nor Value implement Hash or Ord, so faster lookup isn't available
        // Perhaps someday we can do something more clever with ionhash or IonOrd?
        fn remove_elements(a: Sequence, b: &Sequence) -> impl Iterator<Item = Element> + '_ {
            a.into_iter().filter(|i| !b.iter().contains(i))
        }

        let elt: Element = match (lhv, rhv) {
            // Sequences and strings do set subtraction with RHS
            (List(a), List(b)) => ion_rs::List::from_iter(remove_elements(a, &b)).into(),
            (SExp(a), SExp(b)) => ion_rs::SExp::from_iter(remove_elements(a, &b)).into(),

            // Number types, only lossless operations
            (Int(a), Int(b)) => (a + -b).into(), //TODO: use bare - with ion-rs > rc.11
            (Float(a), Float(b)) => a.sub(b).into(),
            (Decimal(a), Decimal(b)) => a.sub(b).into(),
            (Decimal(a), Int(b)) => a.sub(b).into(),
            (Int(a), Decimal(b)) => a.sub(b).into(),

            // Only try potentially lossy Float conversions when we've run out of the other options
            (a @ Int(_) | a @ Decimal(_), Float(b)) => (a.to_f64().unwrap() - b).into(),
            (Float(a), b @ Int(_) | b @ Decimal(_)) => (a - b.to_f64().unwrap()).into(),

            (a, b) => return jaq_binary_error(a, b, "cannot be subtracted"),
        };

        Ok(JaqElement::from(elt))
    }
}

impl Mul for JaqElement {
    type Output = ValR<Self>;

    /// From: https://jqlang.org/manual/#multiplication-division-modulo
    ///
    /// > - Multiplying a string by a number produces the concatenation of that string that many times.
    /// > `"x" * 0` produces `""`.
    /// >
    /// > - Multiplying two objects will merge them recursively: this works like addition but if both
    /// > objects contain a value for the same key, and the values are objects, the two are merged
    /// > with the same strategy.
    fn mul(self, _rhs: Self) -> Self::Output {
        let (lhv, rhv) = (self.into_value(), _rhs.into_value());

        use ion_math::{DecimalMath, ToFloat};
        use Value::*;

        let elt: Element = match (lhv, rhv) {
            (String(a), Int(b)) | (Int(b), String(a)) => match b.as_usize() {
                Some(n) => a.text().repeat(n).into(),
                None => Null(IonType::Null).into(),
            },

            (Symbol(a), Int(b)) | (Int(b), Symbol(a)) => match (b.as_usize(), a.text()) {
                (Some(n), Some(t)) => t.repeat(n).into(),
                _ => Null(IonType::Null).into(), //TODO: Handle symbols with unknown text? How?
            },

            // Structs merge recursively
            //TODO: Recursively remove duplicate fields, see doc comments for rules
            (Struct(a), Struct(b)) => a.clone_builder().with_fields(b.fields()).build().into(),

            // Number types, only lossless operations
            //TODO: use (a*b) when using ion-rs > rc.11
            (Int(a), Int(b)) => (a.expect_i128().unwrap() * b.expect_i128().unwrap()).into(),
            (Float(a), Float(b)) => (a * b).into(),
            (Decimal(a), Decimal(b)) => a.mul(b).into(),
            (Decimal(a), Int(b)) | (Int(b), Decimal(a)) => a.mul(b).into(),

            // Only try potentially lossy Float conversions when we've run out of the other options
            (a @ Int(_) | a @ Decimal(_), Float(b)) => (a.to_f64().unwrap() * b).into(),
            (Float(a), b @ Int(_) | b @ Decimal(_)) => (a * b.to_f64().unwrap()).into(),

            (a, b) => return jaq_binary_error(a, b, "cannot be multiplied"),
        };

        Ok(JaqElement::from(elt))
    }
}

impl Div for JaqElement {
    type Output = ValR<Self>;

    /// From: https://jqlang.org/manual/#multiplication-division-modulo
    ///
    /// > Dividing a string by another splits the first using the second as separators.
    fn div(self, _rhs: Self) -> Self::Output {
        let (lhv, rhv) = (self.into_value(), _rhs.into_value());

        use ion_math::{DecimalMath, ToFloat};
        use Value::*;

        let elt: Element = match (lhv, rhv) {
            // Dividing a string by another splits the first using the second as separators.
            (String(a), String(b)) => {
                let (ta, tb) = (a.text(), b.text());
                let iter = ta.split(tb).map(ion_rs::Str::from).map(Element::from);
                ion_rs::List::from_iter(iter).into()
            }
            (Symbol(a), Symbol(b)) => match (a.text(), b.text()) {
                (Some(ta), Some(tb)) => {
                    let iter = ta.split(tb).map(ion_rs::Symbol::from).map(Element::from);
                    ion_rs::List::from_iter(iter)
                }
                //TODO: Handle symbols with unknown text?
                _ => return jaq_binary_error(Symbol(a), Symbol(b), "cannot be divided"),
            }
            .into(),

            // Number types, only lossless operations
            (Int(a), Int(b)) => (a.expect_i128().unwrap() / b.expect_i128().unwrap()).into(),
            (Float(a), Float(b)) => (a / b).into(),
            (Decimal(a), Decimal(b)) => a.div(b).into(),
            (Decimal(a), Int(b)) => a.div(b).into(),
            (Int(a), Decimal(b)) => a.div(b).into(),

            // Only try potentially lossy Float conversions when we've run out of the other options
            (a @ Int(_) | a @ Decimal(_), Float(b)) => (a.to_f64().unwrap() / b).into(),
            (Float(a), b @ Int(_) | b @ Decimal(_)) => (a / b.to_f64().unwrap()).into(),

            (a, b) => return jaq_binary_error(a, b, "cannot be divided"),
        };

        Ok(JaqElement::from(elt))
    }
}

impl Rem for JaqElement {
    type Output = ValR<Self>;

    fn rem(self, _rhs: Self) -> Self::Output {
        let (lhv, rhv) = (self.into_value(), _rhs.into_value());

        use ion_math::{DecimalMath, ToFloat};
        use Value::*;

        let elt: Element = match (lhv, rhv) {
            // Number types, only lossless operations
            (Int(a), Int(b)) => (a.expect_i128().unwrap() % b.expect_i128().unwrap()).into(),
            (Float(a), Float(b)) => (a % b).into(),
            (Decimal(a), Decimal(b)) => a.rem(b).into(),
            (Decimal(a), Int(b)) => a.rem(b).into(),
            (Int(a), Decimal(b)) => a.rem(b).into(),

            // Only try potentially lossy Float conversions when we've run out of the other options
            (a @ Int(_) | a @ Decimal(_), Float(b)) => (a.to_f64().unwrap() % b).into(),
            (Float(a), b @ Int(_) | b @ Decimal(_)) => (a % b.to_f64().unwrap()).into(),

            (a, b) => return jaq_binary_error(a, b, "cannot be divided (remainder)"),
        };

        Ok(JaqElement::from(elt))
    }
}

impl Neg for JaqElement {
    type Output = ValR<Self>;

    fn neg(self) -> Self::Output {
        let val = self.into_value();

        use ion_math::DecimalMath;
        use Value::*;

        let elt: Element = match val {
            // Only number types can be negated
            Int(a) => (-a).into(),
            Float(a) => (-a).into(),
            Decimal(a) => (-a.into_big_decimal()).into_decimal().into(),

            other => return jaq_unary_error(other, "cannot be negated"),
        };

        Ok(JaqElement::from(elt))
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

        trait OrOwnedNull {
            fn or_owned_null(self) -> Element;
        }

        impl OrOwnedNull for Option<&Element> {
            fn or_owned_null(self) -> Element {
                self.map_or_else(|| Null(IonType::Null).into(), Element::to_owned)
            }
        }

        /// Handles the case where we want to index into a Sequence with a potentially-negative
        /// value. Negative numbers index from the back of the sequence.
        /// Returns an owned Null Element if the index is out of bounds.
        fn index_i128(seq: &Sequence, index: Option<i128>) -> Element {
            let opt = match index {
                Some(i @ ..0) => (seq.len() as i128 + i).to_usize(),
                Some(i) => i.to_usize(),
                None => None,
            };

            opt.and_then(|u| seq.get(u)).or_owned_null()
        }

        let elt: Element = match (self.value(), index.value()) {
            (List(seq) | SExp(seq), Int(i)) => index_i128(seq, i.as_i128()),
            (List(seq) | SExp(seq), Float(f)) => index_i128(seq, Some(*f as i128)),
            (List(seq) | SExp(seq), Decimal(d)) => index_i128(seq, d.into_big_decimal().to_i128()),
            (Struct(strukt), String(name)) => strukt.get(name).or_owned_null(),
            (Struct(strukt), Symbol(name)) => strukt.get(name).or_owned_null(),

            (a, b) => {
                let (alpha, beta) = (a.ion_type(), b.ion_type());
                return jaq_error(format!("cannot index {} with {}", alpha, beta));
            }
        };

        Ok(JaqElement::from(elt))
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

    /// From https://jqlang.org/manual/#if-then-else-end
    ///
    /// > `if A then B else C end` will act the same as `B` if `A` produces a value other than
    /// > `false` or `null`, but act the same as `C` otherwise.
    fn as_bool(&self) -> bool {
        match self.0.value() {
            Value::Null(_) | Value::Bool(false) => false,
            _ => true
        }
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
        match self.0.value() {
            Value::Int(i) => i.expect_i64().unwrap().to_isize(),
            Value::Decimal(d) => d.into_big_decimal().to_isize(),
            _ => None,
        }
    }

    fn as_f64(&self) -> Result<f64, jaq_core::Error<Self>> {
        use ion_math::ToFloat;
        self.0
            .value()
            .clone()
            .to_f64()
            .ok_or_else(|| jaq_err(format!("{self:?} cannot be an f64")))
    }
}

/// The general philosophy of number type conversions used here is that lowest precision wins:
/// 1. Decimal can express any Int, so any binary operation involving a Decimal and an Int produces
///    a Decimal.
/// 2. Floats have less precision than a Decimal and less range than an Int, so any binary operation
///.   involving a Float produces a Float. A Decimal may degrade and lose precision when converted
///    a Float for arithmetic, but the operation will fail if an operand is out of range for Float.
pub(crate) mod ion_math {
    use bigdecimal::num_bigint::BigInt;
    use bigdecimal::{BigDecimal, ToPrimitive};
    use ion_rs::decimal::coefficient::Sign;
    use ion_rs::{Decimal, Int, Value};

    /// We can't provide math traits for Decimal directly, so we have a helper trait
    pub(crate) trait DecimalMath: Sized {
        fn into_big_decimal(self) -> BigDecimal;
        fn into_decimal(self) -> Decimal;

        fn add(self, v2: impl DecimalMath) -> Decimal {
            (self.into_big_decimal() + v2.into_big_decimal()).into_decimal()
        }

        fn sub(self, v2: impl DecimalMath) -> Decimal {
            (self.into_big_decimal() - v2.into_big_decimal()).into_decimal()
        }

        fn mul(self, v2: impl DecimalMath) -> Decimal {
            (self.into_big_decimal() * v2.into_big_decimal()).into_decimal()
        }

        fn div(self, v2: impl DecimalMath) -> Decimal {
            (self.into_big_decimal() / v2.into_big_decimal()).into_decimal()
        }

        fn rem(self, v2: impl DecimalMath) -> Decimal {
            (self.into_big_decimal() % v2.into_big_decimal()).into_decimal()
        }
    }

    impl DecimalMath for Decimal {
        fn into_big_decimal(self) -> BigDecimal {
            let magnitude = self.coefficient().magnitude().as_u128().unwrap();
            let bigint = match self.coefficient().sign() {
                Sign::Negative => -BigInt::from(magnitude),
                Sign::Positive => BigInt::from(magnitude),
            };
            BigDecimal::new(bigint, self.scale())
        }

        fn into_decimal(self) -> Decimal {
            self
        }
    }

    impl DecimalMath for Int {
        fn into_big_decimal(self) -> BigDecimal {
            let data = self.expect_i128().unwrap(); // error case is unreachable with current ion-rs
            BigDecimal::from(data)
        }

        fn into_decimal(self) -> Decimal {
            let data = self.expect_i128().unwrap(); // error case is unreachable with current ion-rs
            Decimal::new(data, 0)
        }
    }

    impl DecimalMath for BigDecimal {
        fn into_big_decimal(self) -> BigDecimal {
            self
        }

        fn into_decimal(self) -> Decimal {
            let (coeff, exponent) = self.into_bigint_and_exponent();
            let data = coeff.to_i128().unwrap();
            Decimal::new(data, -exponent)
        }
    }

    /// A helper trait to allow conversion of various Ion value types into f64. This is inherently a
    /// lossy conversion for most possible expressible Decimal and Integer values even inside f64's
    /// range of expression, so we accept that and move on. The only `None` case for any of these
    /// conversions is when converting a non-numeric `Value` type. A large enough `Int` or `Decimal`
    /// may convert to `Inf` as a float, but that's just the cost of doing business with floating
    /// point math.
    pub(crate) trait ToFloat {
        fn to_f64(self) -> Option<f64>;
    }

    impl ToFloat for f64 {
        fn to_f64(self) -> Option<f64> {
            Some(self)
        }
    }

    impl ToFloat for Int {
        fn to_f64(self) -> Option<f64> {
            self.as_i128().map(|data| data as f64)
        }
    }

    impl ToFloat for Decimal {
        fn to_f64(self) -> Option<f64> {
            self.into_big_decimal().to_f64()
        }
    }

    impl ToFloat for Value {
        fn to_f64(self) -> Option<f64> {
            match self {
                Value::Int(i) => i.to_f64(),
                Value::Decimal(d) => d.to_f64(),
                Value::Float(f) => f.to_f64(),
                _ => None,
            }
        }
    }
}
