use itertools::Itertools;

use crate::{
    difference::StyleDelta,
    fmt_write,
    style::{Coloring, FormatFlags, Style},
    write::Content,
    AnsiGenericString, Color, Infix,
};
use std::{
    collections::HashSet,
    fmt::{self, Debug},
};

fn debug_write_style_flags_to(
    f: &mut dyn fmt::Write,
    flags: FormatFlags,
) -> Result<(), fmt::Error> {
    let mut flags = flags
        .iter_names()
        .map(|n| {
            let name = n.0;
            name.to_lowercase()
        })
        .collect::<Vec<String>>();
    flags.sort_unstable();
    f.write_str(&flags.join(", "))
}

fn debug_write_coloring_to(
    f: &mut dyn fmt::Write,
    fg: Option<Color>,
    bg: Option<Color>,
    mut sep_required: bool,
) -> Result<bool, fmt::Error> {
    if let Some(color) = fg {
        f.write_fmt(format_args!(
            "{}foreground({color:?})",
            if sep_required { ", " } else { "" }
        ))?;
        sep_required = true;
    }
    if let Some(color) = bg {
        f.write_fmt(format_args!(
            "{}background({color:?})",
            if sep_required { ", " } else { "" }
        ))?;
    }
    Ok(sep_required)
}

fn debug_write_style_to(
    f: &mut dyn fmt::Write,
    flags: FormatFlags,
    fg: Option<Color>,
    bg: Option<Color>,
) -> Result<(), fmt::Error> {
    f.write_str("Style { ")?;

    let mut sep_required = false;
    let mut flag_strings: Vec<String> = flags
        .iter_names()
        .filter_map(|n| {
            let name = n.0;
            if name == "FOREGROUND" || name == "BACKGROUND" {
                None
            } else {
                sep_required = true;
                name.to_lowercase().into()
            }
        })
        .collect();
    flag_strings.sort_unstable();
    f.write_str(&flag_strings.join(", "))?;
    debug_write_coloring_to(f, fg, bg, sep_required)?;
    f.write_str(" }")
}

fn debug_style_to_string(
    flags: FormatFlags,
    fg: Option<Color>,
    bg: Option<Color>,
) -> Result<String, fmt::Error> {
    let mut dbg_s = String::new();
    let f = fmt_write!(&mut dbg_s);
    debug_write_style_to(f, flags, fg, bg)?;
    Ok(dbg_s)
}

/// In debug mode, [`Style`]s show flags that are on, and any corresponding
/// colors, if they exist. In alternate debug mode (`:#?`), [`Style`]s show
/// themselves in full detail.
///
///
/// ```
///     use procr_ansi_term::Color::{Red, Blue};
///     assert_eq!(
///         "Style { bold, italic, foreground(Red), background(Blue) }",
///        format!("{:#?}", Red.on(Blue).bold().italic())
///     );
/// ```
impl Debug for Style {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !f.alternate() {
            f.debug_struct("Style")
                .field("flags", &self.formats)
                .field("coloring", &self.coloring)
                .finish()
        } else {
            write!(
                f,
                "{}",
                debug_style_to_string(self.formats, self.coloring.fg, self.coloring.bg)?
            )
        }
    }
}

impl Debug for Coloring {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !f.alternate() {
            f.debug_struct("Coloring")
                .field("foreground", &self.fg)
                .field("background", &self.bg)
                .finish()
        } else {
            debug_write_coloring_to(f, self.fg, self.bg, false)?;
            Ok(())
        }
    }
}

/// [`FormatFlags`] have a special [`Debug`] implementation that only shows the fields that
/// are set. Fields that haven’t been touched aren’t included in the output.
///
/// This behaviour gets bypassed when using the alternate formatting mode
/// `format!("{:#?}")`.
///
/// ```
///     use procr_ansi_term::Color::{Red, Blue};
///     assert_eq!("bold, italic",
///                format!("{:#?}", Red.on(Blue).bold().italic().formats));
///     assert_eq!("foreground(Red), background(Blue)",
///                format!("{:#?}", Red.on(Blue).bold().italic().coloring));
/// ```
impl Debug for FormatFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !f.alternate() {
            let x = FormatFlags::all()
                .iter_names()
                .map(|(name, flag)| format!("{}: {}", name, self.contains(flag)))
                .join(", ");
            f.debug_tuple("FormatFlags").field(&x).finish()
        } else {
            debug_write_style_flags_to(f, *self)
        }
    }
}

/// Test helper: ssert that the `outcome` is the same as the `expected`.
#[macro_export]
macro_rules! assert_required {
    ($outcome:expr, $expected:expr) => {
        assert!(
            $outcome == $expected,
            "Disagreement:\n(test)\t{:?} !=\n\t{:?} (expected)",
            $outcome,
            $expected
        )
    };
}

/// Test helper: compares the debug string form of `tests_style` with `expected`.
///
/// Macro usage (for usage examples see test source code): `($test_style:expr, $expected:expr)`
#[macro_export]
macro_rules! test_style_eq {
    ($test_style:expr, $expected:expr) => {
        let test = format!("{:#?}", $test_style);
        assert_required!(test.as_str(), $expected);
    };
}

/// Test helper: creates test requiring comparison of an `expected` (a
/// string literal) value with a `test_style` applied
/// to some `content` (a string literal).
///
/// Macro usage (for usage examples see test source code): `($test_style:expr, $content:literal, $expected:literal)`
#[macro_export]
macro_rules! test_styled_content_eq {
    ($test_style:expr, $content:literal, $expected:literal) => {
        use $crate::debug::DebugStylePaint;
        let test_result = ($test_style).style_input($content).to_string();
        let expected = $expected.to_string();
        let required_bytes = expected.as_bytes().to_owned();

        $crate::assert_required!(test_result, expected);

        let mut v = Vec::new();
        $test_style
            .style_input($content.as_bytes())
            .write_to(&mut v)
            .unwrap();
        let slice_v = v.as_slice();
        let expected = required_bytes;

        $crate::assert_required!(slice_v, expected);
    };
}

/// Test helper: single entry point for creating various tests useful for this crate.
///
/// Macro usage (for usage examples, see test source code):
/// *  `(@str_cmp $name:ident : try:$test:expr; req:$req:literal)`: create a
///    string comparison test which compares a string produced by `test`
///    with the string specified in `req` (required).
/// * `@style_eq $name: ident : try:$test:expr; req:($flags:expr, $fg:expr,
///   $bg:expr)`: create a style comparison test by comparing the output of the
///   style provided in `test` with manually specified flags it is expected to have.
/// * `@style_eq $name: ident: try:$test:expr; req:$($req:tt)*`: create a style
///   comparison test by comparing the output of the provided style in `test`
///   with some sort expression that evaluates to a string which can be compared
///   with the debug form of style.
/// * `@content_eq $name: ident: try:$test:expr; content:$content:expr;
///   req:$req:expr`: create a a test which compares a `test` style applied to
///   some `content`, with the expected specified in `req`
#[macro_export]
macro_rules! style_test {
    (@str_cmp $name:ident : try:$test:expr; req:$req:literal) => {
        #[test]
        fn $name() {
            $crate::assert_required!($test, $req)
        }
    };
    (@style_eq $name: ident : try:$test:expr; req:($flags:expr, $fg:expr, $bg:expr)) => {
        #[test]
        fn $name() {
            $crate::test_style_eq!($test, &debug_style_to_string($flags, $fg, $bg).unwrap());
        }
    };
    (@style_eq $name: ident: try:$test:expr; req:$($req:tt)*) => {
        #[test]
        fn $name() {
            $crate::test_style_eq!($test, $($req)*);
        }
    };
    (@content_eq $name: ident: try:$test:expr; content:$content:expr; req:$req:expr) => {
        #[test]
        fn $name() {
            $crate::test_styled_content_eq!($test, $content, $req);
        }
    };
}

pub trait DebugStylePaint: Clone + Copy {
    fn into_style(self) -> Style;

    /// Paints the given text with this style, returning an ANSI string.
    #[inline]
    fn style_input<'a, I: ToOwned, S: 'a + ToOwned + ?Sized>(
        &self,
        input: I,
    ) -> AnsiGenericString<'a, S>
    where
        I: Into<Content<'a, S>>,
    {
        AnsiGenericString::new(self.into_style(), input.into(), None)
    }
}

impl DebugStylePaint for Style {
    fn into_style(self) -> Style {
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FgColor(Color);

impl DebugStylePaint for FgColor {
    fn into_style(self) -> Style {
        self.0.normal()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BgColor(Color);

impl DebugStylePaint for BgColor {
    fn into_style(self) -> Style {
        self.0.bg()
    }
}

impl DebugStylePaint for Color {
    fn into_style(self) -> Style {
        self.normal()
    }
}

impl DebugStylePaint for Infix {
    fn into_style(self) -> Style {
        match self.0.compute_delta(self.1) {
            StyleDelta::ExtraStyles(style) => style,
            StyleDelta::Empty => Style::new(),
        }
    }
}

pub trait DebugDiff: Debug + PartialEq + Eq {
    fn debug_diff(&self, expected: &Self) -> String;
}

impl DebugDiff for FormatFlags {
    fn debug_diff(&self, expected: &Self) -> String {
        if self == expected {
            format!("{self:#?} (no diff)")
        } else {
            let outcome_flags = HashSet::<_>::from_iter(self.iter_names().map(|(n, _)| n));
            let expected_flags = HashSet::<_>::from_iter(expected.iter_names().map(|(n, _)| n));
            let in_outcome_not_expected = outcome_flags
                .difference(&expected_flags)
                .copied()
                .collect::<Vec<&str>>();
            let in_expected_not_outcome = expected_flags
                .difference(&outcome_flags)
                .copied()
                .collect::<Vec<&str>>();
            format!(
                "FormatFlags((outcome) {self:#?} (missing: {in_expected_not_outcome:#?}) <~> (missing: {in_outcome_not_expected:#?}) {expected:#?} (expected))",
            )
        }
    }
}

impl DebugDiff for Color {
    fn debug_diff(&self, expected: &Self) -> String {
        if self != expected {
            format!("Color((outcome) {self:#?} != {expected:#?} (expected))",)
        } else {
            format!("{self:#?} (no diff)",)
        }
    }
}

impl<T: DebugDiff> DebugDiff for Option<T> {
    fn debug_diff(&self, expected: &Self) -> String {
        if self == expected {
            format!("{self:#?} (no diff)")
        } else {
            match (self, expected) {
                (None, Some(e)) => {
                    format!("(outcome) {self:#?} != {e:#?} (expected)")
                }
                (Some(out), None) => {
                    format!("(outcome) {out:#?} != {expected:#?} (expected)")
                }
                (Some(o), Some(e)) => {
                    format!("Option<{}>", o.debug_diff(e))
                }
                _ => unreachable!(),
            }
        }
    }
}

impl DebugDiff for Coloring {
    fn debug_diff(&self, expected: &Self) -> String {
        format!(
            "Coloring {{\n\tforeground: {},\n\tbackground: {},\n}}",
            self.fg.debug_diff(&expected.fg),
            expected.bg.debug_diff(&expected.bg)
        )
    }
}

impl DebugDiff for Style {
    fn debug_diff(&self, expected: &Self) -> String {
        format!(
            "Style {{\n\tflags: {},\n\tcoloring: {},\n}}",
            self.formats.debug_diff(&expected.formats),
            self.coloring.debug_diff(&expected.coloring),
        )
    }
}

impl DebugDiff for StyleDelta {
    fn debug_diff(&self, expected: &Self) -> String {
        if self == expected {
            format!("{self:#?} (no diff)")
        } else {
            match (self, expected) {
                (StyleDelta::ExtraStyles(o), StyleDelta::ExtraStyles(e)) => {
                    format!("StyleDelta::PrefixUsing({})", o.debug_diff(e))
                }
                (StyleDelta::ExtraStyles(o), StyleDelta::Empty) => {
                    format!("StyleDelta::PrefixUsing({o:#?} != {expected:#?})")
                }
                (StyleDelta::Empty, StyleDelta::ExtraStyles(e)) => {
                    format!("StyleDelta::PrefixUsing({self:#?} != {e:#?})")
                }
                _ => unreachable!(),
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::debug::debug_style_to_string;
    use crate::style::Color::*;
    use crate::style::{FormatFlags, Style};
    use crate::style_test;
    use paste::paste;

    macro_rules! create_style_eq_tests {
        () => {};
        ([$test_name:ident: $color:expr, $req:literal] $($args:tt)*) => {
            paste! {
                style_test!(
                    @style_eq $test_name:
                    try:$color ;
                    req:$req
                );
            }
            create_style_eq_tests!($($args)*);
        };
        ([$test_name:ident: $($flag:ident),*] $($args:tt)*) => {
            paste! {
                style_test!(
                    @style_eq $test_name:
                    try:Style::new()$(.$flag())* ;
                    req:($(FormatFlags::[< $flag:upper >])|*, None, None)
                );
            }
            create_style_eq_tests!($($args)*);
        };
    }

    style_test!(
        @style_eq empty:
        try:Style::new() ;
        req:(FormatFlags::empty(), None, None)
    );

    create_style_eq_tests!(
        [bold: bold]
        [italic: italic]
        [both: bold, italic]
        [red: Red.normal(), "Style { foreground(Red) }"]
        [redblue: Red.on(Rgb(3, 2, 4)), "Style { foreground(Red), background(Rgb(3, 2, 4)) }"]
        [everything: Red.on(Blue).blink().bold().dimmed().hidden().italic().reverse().strikethrough().underline(), "Style { blink, bold, dimmed, hidden, italic, reverse, strikethrough, underline, foreground(Red), background(Blue) }"]
    );
}
