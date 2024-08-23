use std::fmt::Display;
use std::io::Write;
use std::str::FromStr;

use crate::commands::{CommandIo, IonCliCommand, WithIonCliArgument};
use anyhow::{bail, Context, Result};
use clap::builder::ValueParser;
use clap::{Arg, ArgAction, ArgMatches, Command};
use ion_rs::v1_0::{EncodedBinaryValue, RawValueRef};
use ion_rs::*;

// The `inspect` command uses the `termcolor` crate to colorize its text when STDOUT is a TTY.
use termcolor::{Color, ColorSpec, WriteColor};
// When writing to a named file instead of STDOUT, `inspect` will use a `FileWriter` instead.
// `FileWriter` ignores all requests to emit TTY color escape codes.
use crate::output::CommandOutput;

pub struct InspectCommand;

impl IonCliCommand for InspectCommand {
    fn is_stable(&self) -> bool {
        true
    }

    fn is_porcelain(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "inspect"
    }

    fn about(&self) -> &'static str {
        "Displays hex-encoded binary Ion alongside its equivalent text Ion.
Its output prioritizes human readability and is likely to change
between versions. Stable output for programmatic use cases is a
non-goal."
    }

    fn configure_args(&self, command: Command) -> Command {
        command
            .with_input()
            .with_output()
            .arg(
                // This is named `skip-bytes` instead of `skip` to accommodate a future `skip-values` option.
                Arg::new("skip-bytes")
                    .long("skip-bytes")
                    .short('s')
                    .default_value("0")
                    .hide_default_value(true)
                    .help("Do not display any user values for the first `n` bytes of Ion data.")
                    .long_help(
                        "When specified, the inspector will skip ahead `n` bytes before
beginning to display the contents of the stream. If the requested number
of bytes falls in the middle of a scalar, the whole value (complete with
field ID and annotations if applicable) will be displayed. If the value
is nested in one or more containers, the opening delimiters of those
containers be displayed.",
                    ),
            )
            .arg(
                // This is named `limit-bytes` instead of `limit` to accommodate a future `limit-values` option.
                Arg::new("limit-bytes")
                    .long("limit-bytes")
                    .short('l')
                    .default_value("0")
                    .hide_default_value(true)
                    .help("Only display the next 'n' bytes of Ion data.")
                    .long_help(
                        "When specified, the inspector will stop printing values after
processing `n` bytes of Ion data. If `n` falls within a scalar, the
complete value will be displayed. If `n` falls within one or more containers,
the closing delimiters for those containers will be displayed. If this flag
is used with `--skip-bytes`, `n` is counted from the beginning of the first
value start after `--skip-bytes`.
",
                    ),
            )
            .arg(
                Arg::new("hide-expansion")
                    .long("hide-expansion")
                    .default_value("false")
                    .action(ArgAction::SetTrue)
                    .value_parser(ValueParser::bool())
                    .help("Do not show values produced by macro evaluation.")
                    .long_help(
                        "When specified, the inspector will display e-expressions
(that is: data stream macro invocations) but will not show values produced
 by evaluating those e-expressions. If an e-expression produces a 'system'
 value that modifies the encoding context (that is: a symbol table or
 encoding directive), that value will still be displayed.",
                    ),
            )
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        // On macOS and Linux, the `inspect` command's output will automatically be rerouted to a paging
        // utility like `less` when STDOUT is a TTY.
        // TODO find a cross-platform pager implementation.
        #[cfg(not(target_os = "windows"))]
        {
            // If STDOUT is a TTY, direct output to the pager specified by the PAGER environment
            // variable, or "less -FIRX" if the environment variable is not set.
            pager::Pager::with_default_pager("less -FIRX").setup();
        }

        // `--skip-bytes` has a default value, so we can unwrap this safely.
        let skip_bytes_arg = args.get_one::<String>("skip-bytes").unwrap().as_str();

        let bytes_to_skip = usize::from_str(skip_bytes_arg)
            // The `anyhow` crate allows us to augment a given Result with some arbitrary context that
            // will be displayed if it bubbles up to the end user.
            .with_context(|| format!("Invalid value for '--skip-bytes': '{}'", skip_bytes_arg))?;

        // `--limit-bytes` has a default value, so we can unwrap this safely.
        let limit_bytes_arg = args.get_one::<String>("limit-bytes").unwrap().as_str();

        let mut limit_bytes = usize::from_str(limit_bytes_arg)
            .with_context(|| format!("Invalid value for '--limit-bytes': '{}'", limit_bytes_arg))?;

        // If unset, --limit-bytes is effectively usize::MAX. However, it's easier on users if we let
        // them specify "0" on the command line to mean "no limit".
        if limit_bytes == 0 {
            limit_bytes = usize::MAX;
        }

        let hide_expansion = args.get_flag("hide-expansion");

        CommandIo::new(args).for_each_input(|output, input| {
            let input_name = input.name().to_owned();
            let input = input.into_source();
            inspect_input(
                &input_name,
                input,
                output,
                bytes_to_skip,
                limit_bytes,
                hide_expansion,
            )
        })
    }
}

/// Prints a table showing the offset, length, binary encoding, and text encoding of the Ion stream
/// contained in `input`.
fn inspect_input<Input: IonInput>(
    input_name: &str,
    input: Input,
    output: &mut CommandOutput,
    bytes_to_skip: usize,
    limit_bytes: usize,
    hide_expansion: bool,
) -> Result<()> {
    let mut reader = SystemReader::new(AnyEncoding, input);
    let mut inspector = IonInspector::new(output, bytes_to_skip, limit_bytes, hide_expansion)?;
    // This inspects all values at the top level, recursing as necessary.
    inspector
        .inspect_top_level(&mut reader)
        .with_context(|| format!("input: {input_name}"))?;
    Ok(())
}

// See the Wikipedia page for Unicode Box Drawing[1] for other potentially useful glyphs.
// [1] https://en.wikipedia.org/wiki/Box-drawing_characters#Unicode
const VERTICAL_LINE: &str = "│";
const START_OF_HEADER: &str =
    "┌──────────────┬──────────────┬─────────────────────────┬──────────────────────┐";
const END_OF_HEADER: &str =
    "├──────────────┼──────────────┼─────────────────────────┼──────────────────────┘";
const ROW_SEPARATOR: &str = r#"
├──────────────┼──────────────┼─────────────────────────┤"#;
const END_OF_TABLE: &str = r#"
└──────────────┴──────────────┴─────────────────────────┘"#;

struct IonInspector<'a, 'b> {
    output: &'a mut CommandOutput<'b>,
    bytes_to_skip: usize,
    skip_complete: bool,
    limit_bytes: usize,
    hide_expansion: bool,
    // Text Ion writer for formatting scalar values
    text_writer: v1_0::RawTextWriter<Vec<u8>>,
}

// This buffer is used by the IonInspector's `text_writer` to format scalar values.
const TEXT_WRITER_INITIAL_BUFFER_SIZE: usize = 128;

// The number of hex-encoded bytes to show in each row of the `Binary Ion` column.
const BYTES_PER_ROW: usize = 8;

/// Friendly trait alias (by way of an empty extension) for a closure that takes an output reference
/// and a value and writes a comment for that value. Returns `true` if it wrote a comment, `false`
/// otherwise.
trait CommentFn<'x>: FnMut(&mut CommandOutput, LazyValue<'x, AnyEncoding>) -> Result<bool> {}

impl<'x, F> CommentFn<'x> for F
where
    F: FnMut(&mut CommandOutput, LazyValue<'x, AnyEncoding>) -> Result<bool>,
{}

/// Returns a `CommentFn` implementation that does nothing.
fn no_comment<'x>() -> impl CommentFn<'x> {
    |_, _| Ok(false)
}

impl<'a, 'b> IonInspector<'a, 'b> {
    fn new(
        out: &'a mut CommandOutput<'b>,
        bytes_to_skip: usize,
        limit_bytes: usize,
        hide_expansion: bool,
    ) -> IonResult<IonInspector<'a, 'b>> {
        let text_writer = WriteConfig::<v1_0::Text>::new(TextFormat::Compact)
            .build_raw_writer(Vec::with_capacity(TEXT_WRITER_INITIAL_BUFFER_SIZE))?;
        let inspector = IonInspector {
            output: out,
            bytes_to_skip,
            hide_expansion,
            skip_complete: bytes_to_skip == 0,
            limit_bytes,
            text_writer,
        };
        Ok(inspector)
    }

    fn confirm_encoding_is_supported(&self, encoding: IonEncoding) -> Result<()> {
        use IonEncoding::*;
        match encoding {
            Text_1_0 | Text_1_1 => {
                bail!("`inspect` does not support text Ion streams.");
            }
            Binary_1_0 | Binary_1_1 => Ok(()),
            // `IonEncoding` is #[non_exhaustive]
            _ => bail!("`inspect does not yet support {}", encoding.name()),
        }
    }

    /// Iterates over the items in `reader`, printing a table section for each top level value.
    fn inspect_top_level<Input: IonInput>(
        &mut self,
        reader: &mut SystemReader<AnyEncoding, Input>,
    ) -> Result<()> {
        const TOP_LEVEL_DEPTH: usize = 0;
        use ExpandedStreamItem::*;

        let mut is_first_item = true;
        let mut has_printed_skip_message = false;
        loop {
            if is_first_item {
                self.write_table_header()?;
            }

            let expr = reader.next_expanded_item()?;

            let maybe_raw_item = expr.raw_item();
            // If this item is backed by bytes on the wire, make sure we support the encoding.
            if let Some(raw_item) = maybe_raw_item {
                self.confirm_encoding_is_supported(raw_item.encoding())?;
            }

            let is_last_item = matches!(expr, EndOfStream(_));

            match self.select_action(
                TOP_LEVEL_DEPTH,
                &mut has_printed_skip_message,
                &maybe_raw_item,
                "stream items",
                "ending",
            )? {
                InspectorAction::Skip => {
                    is_first_item = false;
                    continue;
                }
                InspectorAction::Inspect => {}
                InspectorAction::LimitReached => break,
            }

            if !is_first_item && !is_last_item && !expr.is_ephemeral() {
                // If this item is neither the first nor last in the stream, print a row separator.
                write!(self.output, "{ROW_SEPARATOR}")?;
            }

            match expr {
                EExp(eexp) => {
                    self.inspect_eexp(0, eexp)?;
                }
                SymbolTable(lazy_struct) => {
                    self.inspect_symbol_table(lazy_struct)?;
                }
                EncodingDirective(lazy_sexp) => {
                    self.inspect_value(0, "", lazy_sexp.as_value(), no_comment())?;
                }
                Value(lazy_value) => {
                    if lazy_value.expanded().is_ephemeral() && self.hide_expansion {
                        // The user has requested that we not show ephemeral values from macros.
                    } else {
                        self.inspect_value(0, "", lazy_value, no_comment())?;
                    }
                }
                VersionMarker(marker) => {
                    self.confirm_encoding_is_supported(marker.encoding())?;
                    self.inspect_ivm(marker)?;
                }
                EndOfStream(end) => {
                    self.inspect_end_of_stream(end.range().start)?;
                    break;
                }
            }

            is_first_item = false;
        }
        self.output.write_all(END_OF_TABLE.as_bytes())?;
        Ok(())
    }

    /// If `maybe_item` is:
    ///    * `Some(entity)`, checks to see if the entity's final byte offset is beyond the configured
    ///                      number of bytes to skip.
    ///    * `None`, then there is no stream-level entity backing the item (that is: it was the result
    ///              of a macro expansion). Checks to see if the inspector has already completed its
    ///              skipping phase on an earlier item.
    fn should_skip<T: HasRange>(&mut self, maybe_item: &Option<T>) -> bool {
        match maybe_item {
            // If this item came from an input literal, see if the input literal ends after
            // the requested number of bytes to skip. If not, we'll move to the next one.
            Some(item) => item.range().end <= self.bytes_to_skip,
            // If this item came from a macro, there's no corresponding input literal. If we
            // haven't finished skipping input literals, we'll skip this ephemeral value.
            None => !self.skip_complete,
        }
    }

    /// If `maybe_item` is:
    ///    * `Some(entity)`, checks to see if the entity's final byte offset is beyond the configured
    ///                      number of bytes to inspect.
    ///    * `None`, then there is no stream-level entity backing the item. These will always be
    ///              inspected; if the e-expression that produced the value was not beyond the limit,
    ///              none of the ephemeral values it produces are either.
    fn is_past_limit<T: HasRange>(&self, maybe_item: &Option<T>) -> bool {
        let limit = self.bytes_to_skip.saturating_add(self.limit_bytes);
        maybe_item
            .as_ref()
            .map(|item| item.range().start >= limit)
            .unwrap_or(false)
    }

    /// Convenience method to set the output stream to the specified color/style for the duration of `write_fn`
    /// and then reset it upon completion.
    fn with_style(
        &mut self,
        style: ColorSpec,
        write_fn: impl FnOnce(&mut CommandOutput) -> Result<()>,
    ) -> Result<()> {
        self.output.set_color(&style)?;
        write_fn(self.output)?;
        self.output.reset()?;
        Ok(())
    }

    /// Convenience method to set the output stream to the specified color/style, write `text`,
    /// and then reset the output stream's style again.
    fn write_with_style(&mut self, style: ColorSpec, text: &str) -> Result<()> {
        self.with_style(style, |out| {
            out.write_all(text.as_bytes())?;
            Ok(())
        })
    }

    /// Convenience method to move output to the next line.
    fn newline(&mut self) -> Result<()> {
        Ok(self.output.write_all(b"\n")?)
    }

    /// Inspects an Ion Version Marker.
    fn inspect_ivm(&mut self, marker: LazyRawAnyVersionMarker<'_>) -> Result<()> {
        const BINARY_IVM_LENGTH: usize = 4;
        self.newline()?;
        let mut formatter = BytesFormatter::new(
            BYTES_PER_ROW,
            vec![IonBytes::new(
                BytesKind::VersionMarker,
                marker.span().bytes(),
            )],
        );
        self.write_offset_length_and_bytes(
            0,
            marker.range().start,
            BINARY_IVM_LENGTH,
            &mut formatter,
        )?;
        self.with_style(BytesKind::VersionMarker.style(), |out| {
            let (major, minor) = marker.major_minor();
            write!(out, "$ion_{major}_{minor}")?;
            Ok(())
        })?;

        self.with_style(comment_style(), |out| {
            write!(out, " // Version marker")?;
            Ok(())
        })?;
        self.output.reset()?;
        Ok(())
    }

    fn inspect_end_of_stream(&mut self, position: usize) -> Result<()> {
        let mut empty_bytes = BytesFormatter::new(BYTES_PER_ROW, vec![]);
        self.newline()?;
        self.write_offset_length_and_bytes(0, position, "", &mut empty_bytes)?;
        self.write_with_style(comment_style(), "// End of stream")
    }

    fn inspect_eexp(&mut self, depth: usize, eexp: EExpression<AnyEncoding>) -> Result<()> {
        let mut formatter = BytesFormatter::new(
            BYTES_PER_ROW,
            vec![
                // TODO: Add methods to EExpression that allow nuanced access to its encoded spans
                // TODO: Length-prefixed and multibyte e-expression addresses
                IonBytes::new(BytesKind::MacroId, &eexp.span().bytes()[0..1]),
            ],
        );
        self.newline()?;
        self.write_offset_length_and_bytes(
            depth,
            eexp.range().start,
            eexp.span().len(),
            &mut formatter,
        )?;
        self.with_style(eexp_style(), |out| {
            write!(out, "(:{}", eexp.invoked_macro().id_text())?;
            Ok(())
        })?;
        for (param, arg_result) in eexp
            .invoked_macro()
            .signature()
            .parameters()
            .iter()
            .zip(eexp.arguments())
        {
            let arg = arg_result?;
            match arg {
                ValueExpr::ValueLiteral(value) => {
                    self.inspect_value(depth + 1, "", LazyValue::from(value), |out, _value| {
                        write!(out, " // {}", param.name())?;
                        Ok(true)
                    })?;
                }
                ValueExpr::MacroInvocation(invocation) => {
                    use MacroExprKind::*;
                    match invocation.kind() {
                        EExp(eexp_arg) => self.inspect_eexp(depth + 1, eexp_arg)?,
                        EExpArgGroup(_) => todo!("e-exp arg groups"),
                        TemplateMacro(_) => {
                            unreachable!("e-exp args by definition cannot be template invocations")
                        }
                    }
                }
            }
        }
        self.write_text_only_line(depth, eexp_style(), ")")?;
        Ok(())
    }

    fn write_text_only_line(&mut self, depth: usize, style: ColorSpec, text: &str) -> Result<()> {
        self.newline()?;
        self.write_blank_offset_length_and_bytes(depth)?;
        self.write_indentation(depth)?;
        self.write_with_style(style, text)
    }

    /// Inspects all values (however deeply nested) starting at the current level.
    fn inspect_value<'x>(
        &mut self,
        depth: usize,
        delimiter: &str,
        value: LazyValue<'x, AnyEncoding>,
        comment_fn: impl CommentFn<'x>,
    ) -> Result<()> {
        self.newline()?;
        if value.has_annotations() {
            self.inspect_annotations(depth, value)?;
            self.newline()?;
        }
        use ValueRef::*;
        match value.read()? {
            SExp(sexp) => self.inspect_sexp(depth, delimiter, sexp),
            List(list) => self.inspect_list(depth, delimiter, list),
            Struct(struct_) => self.inspect_struct(depth, delimiter, struct_),
            _ => self.inspect_scalar(depth, delimiter, value, comment_fn),
        }
    }

    /// Inspects the scalar `value`. If this value appears in a list or struct, the caller can set
    /// `delimiter` to a comma (`","`) and it will be appended to the value's text representation.
    fn inspect_scalar<'x>(
        &mut self,
        depth: usize,
        delimiter: &str,
        value: LazyValue<'x, AnyEncoding>,
        comment_fn: impl CommentFn<'x>,
    ) -> Result<()> {
        use ExpandedValueSource::*;
        match value.expanded().source() {
            // If the value is backed by an encoded literal AND that literal wasn't passed as an
            // argument to an e-expression, inspect the encoded value.
            ValueLiteral(value_literal) if !value.expanded().is_parameter() => {
                use LazyRawValueKind::*;
                match value_literal.kind() {
                    Binary_1_0(bin_val) => {
                        self.inspect_literal_scalar(depth, delimiter, value, bin_val, comment_fn)
                    }
                    Binary_1_1(bin_val) => {
                        self.inspect_literal_scalar(depth, delimiter, value, bin_val, comment_fn)
                    }
                    Text_1_0(_) | Text_1_1(_) => unreachable!("text value"),
                }
            }
            // Otherwise, display the value without showing its encoding (if any)
            _ => self.inspect_ephemeral_scalar(depth, delimiter, value, comment_fn),
        }
    }

    /// Inspects the s-expression `sexp`, including all of its child values. If this sexp appears
    /// in a list or struct, the caller can set `delimiter` to a comma (`","`) and it will be appended
    /// to the sexp's text representation.
    fn inspect_sexp(
        &mut self,
        depth: usize,
        delimiter: &str,
        sexp: LazySExp<'_, AnyEncoding>,
    ) -> Result<()> {
        use ExpandedSExpSource::*;
        match sexp.expanded().source() {
            ValueLiteral(raw_sexp) => {
                use LazyRawSExpKind::*;
                match raw_sexp.kind() {
                    Text_1_0(_) | Text_1_1(_) => unreachable!("text value"),
                    Binary_1_0(v) => {
                        self.inspect_literal_sexp(depth, delimiter, sexp, v.as_value())
                    }
                    Binary_1_1(v) => {
                        self.inspect_literal_sexp(depth, delimiter, sexp, v.as_value())
                    }
                }
            }
            Template(_, _element) => {
                self.inspect_ephemeral_sequence(depth, "(", "", ")", delimiter, sexp, no_comment())
            }
        }
    }

    /// Inspects the list `list`, including all of its child values. If this list appears inside
    /// a list or struct, the caller can set `delimiter` to a comma (`","`) and it will be appended
    /// to the list's text representation.
    fn inspect_list(
        &mut self,
        depth: usize,
        delimiter: &str,
        list: LazyList<'_, AnyEncoding>,
    ) -> Result<()> {
        use ExpandedListSource::*;
        match list.expanded().source() {
            ValueLiteral(raw_list) => {
                use LazyRawListKind::*;
                match raw_list.kind() {
                    Text_1_0(_) | Text_1_1(_) => unreachable!("text value"),
                    Binary_1_0(v) => self.inspect_literal_sequence(
                        depth,
                        "[",
                        ",",
                        "]",
                        delimiter,
                        list.iter(),
                        v.as_value(),
                        no_comment(),
                    ),
                    Binary_1_1(v) => self.inspect_literal_sequence(
                        depth,
                        "[",
                        ",",
                        "]",
                        delimiter,
                        list.iter(),
                        v.as_value(),
                        no_comment(),
                    ),
                }
            }
            Template(_, _element) => self.inspect_ephemeral_sequence(
                depth,
                "[",
                ",",
                "]",
                delimiter,
                list.iter(),
                no_comment(),
            ),
        }
    }

    /// Inspects the struct `struct_`, including all of its fields. If this struct appears inside
    /// a list or struct, the caller can set `delimiter` to a comma (`","`) and it will be appended
    /// to the struct's text representation.
    fn inspect_struct(
        &mut self,
        depth: usize,
        delimiter: &str,
        struct_: LazyStruct<'_, AnyEncoding>,
    ) -> Result<()> {
        match struct_.expanded().source() {
            ExpandedStructSource::ValueLiteral(raw_struct) => {
                use LazyRawValueKind::*;
                match raw_struct.as_value().kind() {
                    Binary_1_0(v) => self.inspect_literal_struct(depth, delimiter, struct_, v),
                    Binary_1_1(v) => self.inspect_literal_struct(depth, delimiter, struct_, v),
                    Text_1_0(_) | Text_1_1(_) => unreachable!("text value"),
                }
            }
            ExpandedStructSource::Template(_, _, _) => {
                self.inspect_ephemeral_struct(depth, delimiter, struct_)
            }
        }
    }

    fn inspect_symbol_table(&mut self, struct_: LazyStruct<'_, AnyEncoding>) -> Result<()> {
        let value = struct_.as_value();
        if value.has_annotations() {
            self.newline()?;
            self.inspect_annotations(0, value)?;
        }
        let raw_struct = match struct_.expanded().source() {
            ExpandedStructSource::ValueLiteral(raw_struct) => raw_struct,
            ExpandedStructSource::Template(_, _, _) => todo!("Ion 1.1 template symbol table"),
        };

        use LazyRawValueKind::*;
        match raw_struct.as_value().kind() {
            Binary_1_0(v) => self.inspect_literal_symbol_table(struct_, raw_struct, v),
            Binary_1_1(v) => self.inspect_literal_symbol_table(struct_, raw_struct, v),
            Text_1_0(_) | Text_1_1(_) => unreachable!("text value"),
        }
    }

    /// Determines the source of the annotations on the provided value (if any) and adds them to the
    /// output table.
    ///
    /// If the annotations are from a stream literal, their color-coded encoding bytes will also be
    /// displayed.
    fn inspect_annotations(&mut self, depth: usize, value: LazyValue<AnyEncoding>) -> Result<()> {
        if !value.has_annotations() {
            return Ok(());
        }
        match value.expanded().source() {
            ExpandedValueSource::ValueLiteral(raw_value) => {
                use LazyRawValueKind::*;
                match raw_value.kind() {
                    Binary_1_0(v) => self.inspect_literal_annotations(depth, value, v),
                    Binary_1_1(v) => self.inspect_literal_annotations(depth, value, v),
                    Text_1_0(_) | Text_1_1(_) => unreachable!("text value"),
                }
            }
            ExpandedValueSource::Template(_env, element) => self.inspect_ephemeral_annotations(
                depth,
                element.annotations().iter().map(|s| Ok(SymbolRef::from(s))),
            ),
            ExpandedValueSource::Constructed(annotations, _) => self.inspect_ephemeral_annotations(
                depth,
                annotations.iter().copied().map(|s| Ok(s.into())),
            ),
            ExpandedValueSource::SingletonEExp(eexp) => self.inspect_ephemeral_annotations(
                depth,
                eexp.require_singleton_annotations().map(|s| Ok(s.into())),
            ),
        }
    }

    fn inspect_literal_annotations<'x, D: Decoder>(
        &mut self,
        depth: usize,
        value: LazyValue<'x, AnyEncoding>,
        encoded_value: impl EncodedBinaryValue<'x, D>,
    ) -> Result<()> {
        if !value.has_annotations() {
            return Ok(());
        }

        let mut formatter = BytesFormatter::new(
            BYTES_PER_ROW,
            vec![
                IonBytes::new(
                    BytesKind::AnnotationsHeader,
                    encoded_value.annotations_header_span().bytes(),
                ),
                IonBytes::new(
                    BytesKind::AnnotationsSequence,
                    encoded_value.annotations_sequence_span().bytes(),
                ),
            ],
        );
        let range = encoded_value.annotations_span().range();
        self.write_offset_length_and_bytes(depth, range.start, range.len(), &mut formatter)?;

        self.display_annotations_with_raw_encoding_comment(
            value.annotations(),
            encoded_value.annotations(),
        )
    }

    fn inspect_ephemeral_annotations<'x>(
        &mut self,
        depth: usize,
        annotations: impl Iterator<Item=IonResult<SymbolRef<'x>>>,
    ) -> Result<()> {
        self.write_blank_offset_length_and_bytes(depth)?;
        self.with_style(ephemeral_annotations_style(), |out| {
            for annotation in annotations {
                IoValueFormatter::new(&mut *out)
                    .value_formatter()
                    .format_symbol(annotation?)?;
                write!(out, "::")?;
            }
            Ok(())
        })?;
        Ok(())
    }

    /// When the annotations' source is a stream literal, this method adds a text comment indicating
    /// how each annotation was encoded: as a SID (`$10`) or as inline UTF-8 bytes (`foo`).
    fn display_annotations_with_raw_encoding_comment<'x>(
        &mut self,
        annotations: impl Iterator<Item=IonResult<SymbolRef<'x>>>,
        raw_annotations: impl Iterator<Item=IonResult<RawSymbolRef<'x>>>,
    ) -> Result<()> {
        let formatted_annotations = self.format_annotations(annotations)?;
        self.write_with_style(annotations_style(), formatted_annotations.as_str())?;
        self.with_style(comment_style(), |out| {
            write!(out, " // ")?;
            for (index, raw_annotation) in raw_annotations.enumerate() {
                if index > 0 {
                    write!(out, ", ")?;
                }
                match raw_annotation? {
                    RawSymbolRef::SymbolId(sid) => write!(out, "${sid}"),
                    RawSymbolRef::Text(_) => write!(out, "<text>"),
                }?;
            }
            Ok(())
        })
    }

    // ===== Binary Ion 1.0 ======

    // When inspecting a container, the container's header gets its own row in the output table.
    // Unlike a scalar, the bytes of the container body do not begin immediately after the header
    // bytes.
    // This prints the container's offset, length, and header bytes, leaving the cursor positioned
    // at the beginning of the `Text Ion` column.
    fn inspect_literal_container_header<'x, D: Decoder>(
        &mut self,
        depth: usize,
        encoded_value: impl EncodedBinaryValue<'x, D>,
    ) -> Result<()> {
        let opcode_bytes: &[u8] = encoded_value.value_opcode_span().bytes();
        let mut formatter = BytesFormatter::new(
            BYTES_PER_ROW,
            vec![
                IonBytes::new(BytesKind::Opcode, opcode_bytes),
                IonBytes::new(
                    BytesKind::TrailingLength,
                    encoded_value.value_length_span().bytes(),
                ),
            ],
        );

        let range = encoded_value.value_span().range();
        self.write_offset_length_and_bytes(depth, range.start, range.len(), &mut formatter)
    }

    fn inspect_literal_sexp<'x, D: Decoder>(
        &mut self,
        depth: usize,
        delimiter: &str,
        sexp: LazySExp<'x, AnyEncoding>,
        encoded_value: impl EncodedBinaryValue<'x, D>,
    ) -> Result<()> {
        self.inspect_literal_sequence(
            depth,
            "(",
            "",
            ")",
            delimiter,
            sexp.iter(),
            encoded_value,
            no_comment(),
        )
    }

    fn inspect_literal_sequence<'x, D: Decoder>(
        &mut self,
        depth: usize,
        opening_delimiter: &str,
        value_delimiter: &str,
        closing_delimiter: &str,
        trailing_delimiter: &str,
        nested_values: impl IntoIterator<Item=IonResult<LazyValue<'x, AnyEncoding>>>,
        encoded_value: impl EncodedBinaryValue<'x, D>,
        value_comment_fn: impl CommentFn<'x>,
    ) -> Result<()> {
        self.inspect_literal_container_header(depth, encoded_value)?;
        self.with_style(text_ion_style(), |out| {
            write!(out, "{opening_delimiter}")?;
            Ok(())
        })?;

        self.inspect_sequence_body(depth + 1, value_delimiter, nested_values, value_comment_fn)?;

        self.newline()?;
        self.write_blank_offset_length_and_bytes(depth)?;
        self.with_style(text_ion_style(), |out| {
            write!(out, "{closing_delimiter}{trailing_delimiter}")?;
            Ok(())
        })
    }

    fn inspect_ephemeral_sequence<'x>(
        &mut self,
        depth: usize,
        opening_delimiter: &str,
        value_delimiter: &str,
        closing_delimiter: &str,
        trailing_delimiter: &str,
        nested_values: impl IntoIterator<Item=IonResult<LazyValue<'x, AnyEncoding>>>,
        value_comment_fn: impl CommentFn<'x>,
    ) -> Result<()> {
        self.write_blank_offset_length_and_bytes(depth)?;
        self.with_style(ephemeral_value_style(), |out| {
            write!(out, "{opening_delimiter}")?;
            Ok(())
        })?;

        self.inspect_sequence_body(depth + 1, value_delimiter, nested_values, value_comment_fn)?;

        self.newline()?;
        self.write_blank_offset_length_and_bytes(depth)?;
        self.with_style(ephemeral_value_style(), |out| {
            write!(out, "{closing_delimiter}{trailing_delimiter}")?;
            Ok(())
        })
    }

    fn inspect_sequence_body<'x>(
        &mut self,
        depth: usize,
        value_delimiter: &str,
        nested_values: impl IntoIterator<Item=IonResult<LazyValue<'x, AnyEncoding>>>,
        mut value_comment_fn: impl CommentFn<'x>,
    ) -> Result<()> {
        let mut has_printed_skip_message = false;
        for value_res in nested_values {
            let nested_value = value_res?;
            // If this value is a literal in the stream, see if it is in the bounds of byte
            // ranges we care about.

            match self.select_action(
                depth,
                &mut has_printed_skip_message,
                &nested_value.raw(),
                "values",
                "stepping out",
            )? {
                InspectorAction::Skip => continue,
                InspectorAction::Inspect => {}
                InspectorAction::LimitReached => break,
            }

            self.inspect_value(depth, value_delimiter, nested_value, no_comment())?;
            self.output.set_color(&comment_style())?;
            value_comment_fn(self.output, nested_value)?;
            self.output.reset()?;
        }
        Ok(())
    }

    fn select_action<T: HasRange>(
        &mut self,
        depth: usize,
        has_printed_skip_message: &mut bool,
        maybe_item: &Option<T>,
        name_of_skipped_item: &str,
        name_of_limit_action: &str,
    ) -> Result<InspectorAction> {
        if self.should_skip(maybe_item) {
            if !*has_printed_skip_message {
                self.write_skipping_message(depth, name_of_skipped_item)?;
                *has_printed_skip_message = true;
            }
            return Ok(InspectorAction::Skip);
        }
        self.skip_complete = true;

        if self.is_past_limit(maybe_item) {
            self.write_limiting_message(depth, name_of_limit_action)?;
            return Ok(InspectorAction::LimitReached);
        }

        Ok(InspectorAction::Inspect)
    }

    fn inspect_literal_field_name(
        &mut self,
        depth: usize,
        raw_name: LazyRawAnyFieldName,
        name: SymbolRef,
    ) -> Result<()> {
        self.newline()?;
        let range = raw_name.range();
        let raw_name_bytes = raw_name.span().bytes();
        let offset = range.start;
        let length = range.len();
        let mut formatter = BytesFormatter::new(
            BYTES_PER_ROW,
            vec![IonBytes::new(BytesKind::FieldId, raw_name_bytes)],
        );
        self.write_offset_length_and_bytes(depth, offset, length, &mut formatter)?;
        self.with_style(field_id_style(), |out| {
            IoValueFormatter::new(out)
                .value_formatter()
                .format_symbol(name)?;
            Ok(())
        })?;
        write!(self.output, ": ")?;
        // Print a text Ion comment showing how the field name was encoded, ($SID or text)
        self.with_style(comment_style(), |out| {
            match raw_name.read()? {
                RawSymbolRef::SymbolId(sid) => {
                    write!(out, " // ${sid}")
                }
                RawSymbolRef::Text(_) => {
                    write!(out, " // <text>")
                }
            }?;
            Ok(())
        })
    }

    /// Inspects all values (however deeply nested) starting at the current field.
    fn inspect_field(&mut self, depth: usize, field: LazyField<AnyEncoding>) -> Result<()> {
        let name = field.name()?;
        if let Some(raw_name) = field.raw_name() {
            self.inspect_literal_field_name(depth, raw_name, name)?;
        } else {
            self.newline()?;
            self.write_blank_offset_length_and_bytes(depth)?;
            self.with_style(ephemeral_field_id_style(), |out| {
                IoValueFormatter::new(out)
                    .value_formatter()
                    .format_symbol(name)?;
                Ok(())
            })?;
            write!(self.output, ": ")?;
        };
        self.inspect_value(depth, ",", field.value(), no_comment())?;
        Ok(())
    }

    fn inspect_literal_struct<'x, D: Decoder>(
        &mut self,
        depth: usize,
        delimiter: &str,
        struct_: LazyStruct<AnyEncoding>,
        encoded_value: impl EncodedBinaryValue<'x, D>,
    ) -> Result<()> {
        self.inspect_literal_container_header(depth, encoded_value)?;
        self.write_with_style(text_ion_style(), "{")?;
        self.inspect_struct_body(depth, struct_)?;
        self.newline()?;
        self.write_blank_offset_length_and_bytes(depth)?;
        self.with_style(text_ion_style(), |out| {
            write!(out, "}}{delimiter}")?;
            Ok(())
        })
    }

    fn inspect_ephemeral_struct<'x>(
        &mut self,
        depth: usize,
        delimiter: &str,
        struct_: LazyStruct<AnyEncoding>,
    ) -> Result<()> {
        self.write_blank_offset_length_and_bytes(depth)?;
        self.with_style(ephemeral_value_style(), |out| {
            write!(out, "{{")?;
            Ok(())
        })?;
        self.inspect_struct_body(depth, struct_)?;
        self.newline()?;
        self.write_blank_offset_length_and_bytes(depth)?;
        self.with_style(ephemeral_value_style(), |out| {
            write!(out, "}}{delimiter}")?;
            Ok(())
        })
    }

    fn inspect_struct_body<'x>(
        &mut self,
        depth: usize,
        struct_: LazyStruct<AnyEncoding>,
    ) -> Result<()> {
        let mut has_printed_skip_message: bool = false;
        for field_result in struct_.iter() {
            let field = field_result?;
            match self.select_action(
                depth + 1,
                &mut has_printed_skip_message,
                &field.range(),
                "fields",
                "stepping out",
            )? {
                InspectorAction::Skip => continue,
                InspectorAction::Inspect => self.inspect_field(depth + 1, field)?,
                InspectorAction::LimitReached => break,
            }
        }
        Ok(())
    }

    fn inspect_literal_symbol_table<'x, D: Decoder>(
        &mut self,
        struct_: LazyStruct<AnyEncoding>,
        raw_struct: LazyRawAnyStruct,
        encoded_value: impl EncodedBinaryValue<'x, D>,
    ) -> Result<()> {
        // The processing for a symbol table is very similar to that of a regular struct,
        // but with special handling defined for the `imports` and `symbols` fields when present.
        // Because symbol tables are always at the top level, there is no need for indentation.
        const TOP_LEVEL_DEPTH: usize = 0;
        self.newline()?;
        self.inspect_literal_container_header(TOP_LEVEL_DEPTH, encoded_value)?;
        self.with_style(text_ion_style(), |out| {
            write!(out, "{{")?;
            Ok(())
        })?;
        let mut has_printed_skip_message = false;
        for (raw_field_result, field_result) in raw_struct.iter().zip(struct_.iter()) {
            let field = field_result?;
            let raw_field = raw_field_result?;

            match self.select_action(
                TOP_LEVEL_DEPTH + 1,
                &mut has_printed_skip_message,
                &Some(raw_field),
                "fields",
                "stepping out",
            )? {
                InspectorAction::Skip => continue,
                InspectorAction::Inspect if field.name()? == "symbols" => {
                    self.inspect_lst_symbols_field(struct_, field, raw_field)?
                }
                // TODO: if field.name()? == "imports" => {}
                InspectorAction::Inspect => self.inspect_field(TOP_LEVEL_DEPTH + 1, field)?,
                InspectorAction::LimitReached => break,
            }
        }
        // ===== Closing delimiter =====
        self.newline()?;
        self.write_blank_offset_length_and_bytes(TOP_LEVEL_DEPTH)?;
        self.with_style(text_ion_style(), |out| {
            write!(out, "}}")?;
            Ok(())
        })
    }

    fn inspect_lst_symbols_field(
        &mut self,
        symtab_struct: LazyStruct<AnyEncoding>,
        field: LazyField<AnyEncoding>,
        raw_field: LazyRawFieldExpr<AnyEncoding>,
    ) -> Result<()> {
        const SYMBOL_LIST_DEPTH: usize = 1;
        let (raw_name, raw_value) = raw_field.expect_name_value()?;
        self.inspect_literal_field_name(SYMBOL_LIST_DEPTH, raw_name, field.name()?)?;

        let symbols_list = match field.value().read()? {
            ValueRef::List(list) => list,
            _ => {
                return self.inspect_value(SYMBOL_LIST_DEPTH, ",", field.value(), |out, _value| {
                    out.write_all(b" // Invalid, ignored")?;
                    Ok(true)
                });
            }
        };

        let raw_symbols_list = raw_value.read()?.expect_list()?;
        let nested_raw_values = raw_symbols_list.iter();
        let nested_values = symbols_list.iter();

        self.newline()?;
        match raw_value.kind() {
            LazyRawValueKind::Binary_1_0(raw_value) => {
                self.inspect_literal_container_header(SYMBOL_LIST_DEPTH, raw_value)?;
            }
            LazyRawValueKind::Binary_1_1(raw_value) => {
                self.inspect_literal_container_header(SYMBOL_LIST_DEPTH, raw_value)?;
            }
            other_kind => unreachable!("binary encoding already confirmed; found {other_kind:?}"),
        }
        self.with_style(text_ion_style(), |out| {
            write!(out, "[")?;
            Ok(())
        })?;

        // TODO: This does not account for shared symbol table imports. However, the CLI does not
        //       yet support specifying a catalog, so it's correct enough for the moment.
        let symtab_value = symtab_struct.as_value();
        let mut next_symbol_id = symtab_value.symbol_table().len();
        let is_append = symtab_struct.get("imports")?
            == Some(ValueRef::Symbol(SymbolRef::with_text("$ion_symbol_table")));
        if !is_append {
            next_symbol_id = 10; // First available SID after system symbols in Ion 1.0
        }

        let mut has_printed_skip_message = false;
        for (raw_value_res, value_res) in nested_raw_values.zip(nested_values) {
            let (raw_nested_value, nested_value) = (raw_value_res?, value_res?);
            match self.select_action(
                SYMBOL_LIST_DEPTH + 1,
                &mut has_printed_skip_message,
                &Some(raw_nested_value),
                "values",
                "stepping out",
            )? {
                InspectorAction::Skip => continue,
                InspectorAction::Inspect => {}
                InspectorAction::LimitReached => break,
            }

            self.output.set_color(&comment_style())?;
            self.inspect_value(SYMBOL_LIST_DEPTH + 1, ",", nested_value, |out, value| {
                match value.read()? {
                    ValueRef::String(_s) => write!(out, " // -> ${next_symbol_id}"),
                    _other => write!(out, " // -> ${next_symbol_id} (no text)"),
                }?;
                next_symbol_id += 1;
                Ok(true)
            })?;
            self.output.reset()?;
        }

        self.newline()?;
        self.write_blank_offset_length_and_bytes(SYMBOL_LIST_DEPTH)?;
        self.with_style(text_ion_style(), |out| {
            write!(out, "],")?;
            Ok(())
        })
    }

    fn inspect_literal_scalar<'x, D: Decoder>(
        &mut self,
        depth: usize,
        delimiter: &str,
        value: LazyValue<'x, AnyEncoding>,
        encoded_value: impl EncodedBinaryValue<'x, D>,
        mut comment_fn: impl CommentFn<'x>,
    ) -> Result<()> {
        let range = encoded_value.value_span().range();

        let opcode_bytes = IonBytes::new(BytesKind::Opcode, encoded_value.value_opcode_span());
        let length_bytes =
            IonBytes::new(BytesKind::TrailingLength, encoded_value.value_length_span());
        let body_bytes = IonBytes::new(BytesKind::ValueBody, encoded_value.value_body_span());

        let mut formatter =
            BytesFormatter::new(BYTES_PER_ROW, vec![opcode_bytes, length_bytes, body_bytes]);

        self.write_offset_length_and_bytes(depth, range.start, range.len(), &mut formatter)?;

        let formatted_value = self.format_scalar_body(value)?;
        self.with_style(text_ion_style(), |out| {
            write!(out, "{formatted_value}{delimiter}")?;
            Ok(())
        })?;
        self.with_style(comment_style(), |out| {
            let wrote_comment = comment_fn(out, value)?;
            if let RawValueRef::Symbol(RawSymbolRef::SymbolId(symbol_id)) = encoded_value.read()? {
                match wrote_comment {
                    true => write!(out, " (${symbol_id})"),
                    false => write!(out, " // ${symbol_id}"),
                }?;
            }
            Ok(())
        })?;

        while !formatter.is_empty() {
            self.newline()?;
            self.write_offset_length_and_bytes(depth, "", "", &mut formatter)?;
        }

        Ok(())
    }

    fn inspect_ephemeral_scalar<'x>(
        &mut self,
        depth: usize,
        delimiter: &str,
        value: LazyValue<'x, AnyEncoding>,
        mut comment_fn: impl CommentFn<'x>,
    ) -> Result<()> {
        self.write_blank_offset_length_and_bytes(depth)?;
        let formatted_value = self.format_scalar_body(value)?;
        let style = if let Some(variable) = value.expanded().variable() {
            self.write_offset_length_and_bytes_comment(depth, "", "", variable.name())?;
            ephemeral_value_style().set_underline(true).clone()
        } else {
            ephemeral_value_style().clone()
        };

        self.with_style(style.clone(), |out| {
            write!(out, "{formatted_value}")?;
            Ok(())
        })?;
        self.write_with_style(style.clone().set_underline(false).clone(), delimiter)?;
        self.with_style(comment_style(), |out| {
            comment_fn(out, value)?;
            Ok(())
        })?;
        Ok(())
    }

    fn format_annotations<'x>(
        &self,
        annotations: impl Iterator<Item=IonResult<SymbolRef<'x>>>,
    ) -> Result<String> {
        use std::fmt::Write;
        let mut formatted_annotations = String::new();
        for annotation in annotations {
            write!(
                &mut formatted_annotations,
                "{}::",
                annotation?.text().unwrap_or("$0")
            )?;
        }
        Ok(formatted_annotations)
    }

    fn format_scalar_body(&mut self, value: LazyValue<AnyEncoding>) -> Result<String> {
        self.text_writer
            .write(value.read()?)
            .expect("failed to write text value to in-memory buffer")
            .flush()?;

        let encoded_bytes = self.text_writer.output_mut().trim_ascii_end();
        let formatted_body = std::str::from_utf8(encoded_bytes)?.to_owned();
        self.text_writer.output_mut().clear();
        Ok(formatted_body)
    }

    // ===== Table-writing methods =====

    /// Prints the header of the output table
    fn write_table_header(&mut self) -> Result<()> {
        self.output.write_all(START_OF_HEADER.as_bytes())?;
        write!(self.output, "\n{VERTICAL_LINE}")?;
        self.write_with_style(header_style(), "    Offset    ")?;
        write!(self.output, "{VERTICAL_LINE}")?;
        self.write_with_style(header_style(), "    Length    ")?;
        write!(self.output, "{VERTICAL_LINE}")?;
        self.write_with_style(header_style(), "       Binary Ion        ")?;
        write!(self.output, "{VERTICAL_LINE}")?;
        self.write_with_style(header_style(), "       Text Ion       ")?;
        writeln!(self.output, "{VERTICAL_LINE}")?;
        self.output.write_all(END_OF_HEADER.as_bytes())?;
        Ok(())
    }

    /// Writes a spacing string `depth` times.
    fn write_indentation(&mut self, depth: usize) -> Result<()> {
        // This spacing string includes a unicode dot to make it easy to see what level of depth
        // the current value is found at. This dot is displayed with a muted color; its appearance
        // is subtle.
        const INDENTATION_WITH_GUIDE: &str = "· ";

        let mut color_spec = ColorSpec::new();
        color_spec
            .set_dimmed(false)
            .set_intense(true)
            .set_bold(true)
            .set_fg(Some(Color::Rgb(100, 100, 100)));
        self.with_style(color_spec, |out| {
            for _ in 0..depth {
                out.write_all(INDENTATION_WITH_GUIDE.as_bytes())?;
            }
            Ok(())
        })
    }

    /// Prints the given `offset` and `length` in the first and second table columns.
    /// The `offset` and `length` are typically `usize`, but can be anything that implements `Display`.
    /// Prints the provided value in the `bytes` column using 'comment' styling.
    fn write_offset_length_and_bytes_comment(
        &mut self,
        depth: usize,
        offset: impl Display,
        length: impl Display,
        bytes: impl Display,
    ) -> Result<()> {
        write!(
            self.output,
            "{VERTICAL_LINE} {offset:12} {VERTICAL_LINE} {length:12} {VERTICAL_LINE} "
        )?;
        self.with_style(ephemeral_bytes_style(), |out| {
            write!(out, "{bytes:>23}")?;
            Ok(())
        })?;
        write!(self.output, " {VERTICAL_LINE} ")?;
        self.write_indentation(depth)?;
        Ok(())
    }

    /// Prints the given `offset` and `length` in the first and second table columns, then uses the
    /// `formatter` to print a single row of hex-encoded bytes in the third column ("Binary Ion").
    /// The `offset` and `length` are typically `usize`, but can be anything that implements `Display`.
    fn write_offset_length_and_bytes(
        &mut self,
        depth: usize,
        offset: impl Display,
        length: impl Display,
        formatter: &mut BytesFormatter,
    ) -> Result<()> {
        write!(
            self.output,
            "{VERTICAL_LINE} {offset:12} {VERTICAL_LINE} {length:12} {VERTICAL_LINE} "
        )?;
        formatter.write_row(self.output)?;
        write!(self.output, "{VERTICAL_LINE} ")?;
        self.write_indentation(depth)?;
        Ok(())
    }

    /// Prints a row with blank fiends in the `Offset`, `Length`, and `Binary Ion` columns. This method
    /// does not print a trailing newline, allowing the caller to populate the `Text Ion` column as needed.
    fn write_blank_offset_length_and_bytes(&mut self, depth: usize) -> Result<()> {
        let mut formatter = BytesFormatter::new(BYTES_PER_ROW, vec![]);
        self.write_offset_length_and_bytes(depth, "", "", &mut formatter)
    }

    /// Prints a row with an ellipsis (`...`) in the first three columns, and a text Ion comment in
    /// the final column indicating what is being skipped over.
    fn write_skipping_message(&mut self, depth: usize, name_of_skipped_item: &str) -> Result<()> {
        write!(self.output, "\n{VERTICAL_LINE} {:>12} {VERTICAL_LINE} {:>12} {VERTICAL_LINE} {:23} {VERTICAL_LINE} ", "...", "...", "...")?;
        self.write_indentation(depth)?;
        self.with_style(comment_style(), |out| {
            write!(out, "// ...skipping {name_of_skipped_item}...")?;
            Ok(())
        })
    }

    /// Prints a row with an ellipsis (`...`) in the first three columns, and a text Ion comment in
    /// the final column indicating that we have reached the maximum number of bytes to process
    /// as determined by the `--limit-bytes` flag.
    fn write_limiting_message(&mut self, depth: usize, action: &str) -> Result<()> {
        write!(self.output, "\n{VERTICAL_LINE} {:>12} {VERTICAL_LINE} {:>12} {VERTICAL_LINE} {:23} {VERTICAL_LINE} ", "...", "...", "...")?;
        self.write_indentation(depth)?;
        let limit_bytes = self.limit_bytes;
        self.with_style(comment_style(), |out| {
            write!(out, "// --limit-bytes {} reached, {action}.", limit_bytes)?;
            Ok(())
        })
    }
}

pub enum InspectorAction {
    /// The current value appears before the offset specified by `--skip-bytes`. Ignore it.
    Skip,
    /// The current value appears after `--skip-bytes` and before `--limit-bytes`. Inspect it.
    Inspect,
    /// The current value appears after `--limit-bytes`, stop inspecting values.
    LimitReached,
}

// ===== Named styles =====

fn header_style() -> ColorSpec {
    let mut style = ColorSpec::new();
    style.set_bold(true).set_intense(true);
    style
}

fn eexp_style() -> ColorSpec {
    let mut style = ColorSpec::new();
    style
        .set_fg(Some(Color::Green))
        .set_bold(true)
        .set_intense(true);
    style
}

fn comment_style() -> ColorSpec {
    let mut style = ColorSpec::new();
    style.set_dimmed(true);
    style
}

fn ephemeral_bytes_style() -> ColorSpec {
    eexp_style().set_dimmed(true).clone()
}

fn ephemeral_value_style() -> ColorSpec {
    let mut style = ColorSpec::new();
    style.set_fg(Some(Color::White));
    style
}

fn ephemeral_field_id_style() -> ColorSpec {
    let mut style = field_id_style();
    style.set_dimmed(true);
    style
}

fn text_ion_style() -> ColorSpec {
    let mut style = ColorSpec::new();
    style.set_fg(Some(Color::Rgb(255, 255, 255)));
    style
}

fn field_id_style() -> ColorSpec {
    let mut style = ColorSpec::new();
    style.set_fg(Some(Color::Cyan)).set_intense(true);
    style
}

fn annotations_style() -> ColorSpec {
    let mut style = ColorSpec::new();
    style.set_fg(Some(Color::Magenta));
    style
}

fn ephemeral_annotations_style() -> ColorSpec {
    annotations_style().set_dimmed(true).clone()
}

/// Kinds of encoding primitives found in a binary Ion stream.
#[derive(Copy, Clone, Debug)]
enum BytesKind {
    MacroId,
    FieldId,
    Opcode,
    TrailingLength,
    ValueBody,
    AnnotationsHeader,
    AnnotationsSequence,
    VersionMarker,
}

impl BytesKind {
    /// Returns a [`ColorSpec`] that should be used when printing bytes of the specified `BytesKind`.
    fn style(&self) -> ColorSpec {
        use BytesKind::*;
        let mut color = ColorSpec::new();
        match self {
            VersionMarker => color.set_fg(Some(Color::Yellow)).set_intense(true),
            FieldId => color.set_fg(Some(Color::Cyan)).set_intense(true),
            Opcode => color
                .set_bold(true)
                .set_fg(Some(Color::Rgb(0, 0, 0)))
                .set_bg(Some(Color::Rgb(255, 255, 255))),
            MacroId => color
                .set_bold(true)
                .set_fg(Some(Color::Rgb(0, 0, 0)))
                .set_bg(Some(Color::Green))
                .set_bold(true)
                .set_intense(true),
            TrailingLength => color
                .set_bold(true)
                .set_underline(true)
                .set_fg(Some(Color::White))
                .set_intense(true),
            ValueBody => color
                .set_bold(false)
                .set_fg(Some(Color::White))
                .set_intense(false),
            AnnotationsHeader => color
                .set_bold(false)
                .set_fg(Some(Color::Black))
                .set_bg(Some(Color::Magenta)),
            AnnotationsSequence => color.set_bold(false).set_fg(Some(Color::Magenta)),
        };
        color
    }
}

/// A slice of Ion bytes to be printed in the `Binary Ion` column.
///
/// Each `IonBytes` has a `BytesKind` that maps to a display style as well as a counter tracking
/// how many of its bytes have been printed so far.
#[derive(Copy, Clone, Debug)]
struct IonBytes<'a> {
    // The actual slice of bytes
    pub bytes: &'a [u8],
    // What the slice of bytes represents in Ion
    pub kind: BytesKind,
    // How many of this slice's bytes have been printed so far.
    pub bytes_written: usize,
}

impl<'a> IonBytes<'a> {
    fn new(kind: BytesKind, bytes: impl Into<&'a [u8]>) -> Self {
        Self {
            bytes: bytes.into(),
            kind,
            bytes_written: 0,
        }
    }

    fn mark_bytes_written(&mut self, num_bytes: usize) {
        self.bytes_written += num_bytes
    }

    fn next_n_bytes(&self, num_bytes: usize) -> &[u8] {
        &self.bytes[self.bytes_written..self.bytes_written + num_bytes]
    }

    fn bytes_remaining(&self) -> usize {
        self.bytes.len() - self.bytes_written
    }

    fn is_empty(&self) -> bool {
        self.bytes_remaining() == 0
    }

    fn style(&self) -> ColorSpec {
        self.kind.style()
    }
}

/// Prints bytes as colorized, hex-encoded rows of a configurable size.
///
/// Stores a sequence of [`IonBytes`] instances to display. Upon request, writes out the next `n`
/// colorized, hex-encoded bytes, remembering where to resume when the next row is needed.
struct BytesFormatter<'a> {
    slices: Vec<IonBytes<'a>>,
    slices_written: usize,
    formatted_bytes_per_row: usize,
}

impl<'a> BytesFormatter<'a> {
    pub fn new(formatted_bytes_per_row: usize, slices: Vec<IonBytes<'a>>) -> Self {
        Self {
            slices,
            slices_written: 0,
            formatted_bytes_per_row,
        }
    }

    /// Writes a row of `n` hex-encoded, colorized bytes, where `n` is determined by the
    /// `formatted_bytes_per_row` argument in [`BytesFormatter::new`].
    ///
    /// If there are fewer than `n` bytes remaining, prints all remaining bytes.
    pub fn write_row(&mut self, output: &mut impl WriteColor) -> Result<()> {
        let num_bytes = self.formatted_bytes_per_row;
        let bytes_written = self.write_bytes(num_bytes, output)?;
        let bytes_remaining = num_bytes - bytes_written;
        // If we printed fewer bytes than are needed to make a row, write out enough padding
        // to keep the columns aligned.
        for _ in 0..bytes_remaining {
            write!(output, "   ")?; // Empty space the width of a formatted byte
        }
        Ok(())
    }

    /// Helper method to iterate over the remaining [`IonBytes`], printing their contents until
    /// `num_bytes` is reached.
    fn write_bytes(&mut self, num_bytes: usize, output: &mut impl WriteColor) -> Result<usize> {
        let mut bytes_remaining = num_bytes;
        while bytes_remaining > 0 && !self.is_empty() {
            bytes_remaining -= self.write_bytes_from_current_slice(bytes_remaining, output)?;
            if self.is_empty() {
                // Even though `bytes_remaining` hasn't reached zero, we're out of data.
                break;
            }
        }

        Ok(num_bytes - bytes_remaining)
    }

    /// Helper method to print up to `num_bytes` bytes from the current [`IonBytes`].
    fn write_bytes_from_current_slice(
        &mut self,
        num_bytes: usize,
        output: &mut impl WriteColor,
    ) -> Result<usize> {
        let Some(slice) = self.current_slice() else {
            // No more to write
            return Ok(0);
        };

        if slice.bytes.is_empty() {
            self.slices_written += 1;
            return Ok(0);
        }

        // We're going to write whichever is smaller:
        //   1. the requested number of bytes from the current slice
        //      OR
        //   2. the number of bytes remaining in the current slice
        let bytes_to_write = num_bytes.min(slice.bytes_remaining());

        // Set the appropriate style for this byte slice.
        let style: ColorSpec = slice.style();
        output.set_color(&style)?;
        write!(
            output,
            "{}",
            hex_contents(slice.next_n_bytes(bytes_to_write))
        )?;
        slice.mark_bytes_written(bytes_to_write);
        output.reset()?;

        // If we completed the slice OR we finished writing all of the requested bytes
        if slice.is_empty() || num_bytes == bytes_to_write {
            write!(output, " ")?;
        }

        if slice.is_empty() {
            // This slice has been exhausted, we should resume from the beginning of the next one.
            self.slices_written += 1;
        }

        Ok(bytes_to_write)
    }

    /// Returns a reference to the [`IonBytes`] from which the next bytes should be pulled.
    fn current_slice(&mut self) -> Option<&mut IonBytes<'a>> {
        if self.is_empty() {
            return None;
        }
        Some(&mut self.slices[self.slices_written])
    }

    /// Returns `true` if all of the slices have been exhausted.
    fn is_empty(&self) -> bool {
        self.slices_written == self.slices.len()
    }
}

/// Converts the given byte slice to a string containing hex-encoded bytes
fn hex_contents(source: &[u8]) -> String {
    if source.is_empty() {
        return String::new();
    }
    use std::fmt::Write;
    let mut buffer = String::new();
    let bytes = source.iter();

    let mut is_first = true;
    for byte in bytes {
        if is_first {
            write!(buffer, "{:02x?}", byte).unwrap();
            is_first = false;
            continue;
        }
        write!(buffer, " {:02x?}", byte).unwrap();
    }
    buffer
}
