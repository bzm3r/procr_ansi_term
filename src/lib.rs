//! This is a library for controlling colors and formatting, such as red bold
//! text or blue underlined text, on ANSI terminals.
//!
//! ## Should I use this crate?
//!
//! Short answer: no.
//! [`owo_colors`](https://docs.rs/owo-colors/latest/owo_colors/) does
//! everything this crate does, but more rationally, and more effectively.
//!
//! ## History
//!
//! [`nu-ansi-term`](https://github.com/nushell/nu-ansi-term) is used by
//! [`tracing_subscriber`](https://github.com/tokio-rs/tracing/tree/master/tracing-subscriber)
//! (for what may be considered [historical
//! reasons](https://github.com/tokio-rs/tracing/pull/2040)), and was a copy of
//! `ansi_term` but with `Colour` changed to `Color` and various colors added.
//! `procr-ansi-term`, born in a time when I didn't know better, adds
//! functionality to `nu-ansi-term` which allows ANSI formatting of
//! `format_args!`, and also allows various ANSI strings/format args to be
//! nested in styling, with a primitive parent-child inheritance of said
//! styling.
//!
//! It is ultimately best understood as the result of hyperfocus/procrastination
//! (hence the name `procr_ansi_term`).
//!
//! ## What is this crate useful for?
//!
//! Styling data that will be eventually rendered by a terminal capable of
//! interpreting ANSI style codes.
//!
//!
//! ## Basic usage
//!
//! There are a few basic types: [`Style`](crate::style::Style), [`Color`](crate::style::Color), [`AnsiGenericString`],
//! and [`Content`].
//!
//! A [`Style`] holds stylistic information: foreground and background colors,
//! whether the text should be bold, or blinking, or other properties. The
//! [`Color`] enum represents the available colors. And an [`AnsiString`] is a
//! string paired with a [`Style`].
//!
//! To format some [`Content`], call the `paint` method on a [`Style`] or a `Color`,
//! passing in some input that can be converted into [`Content`] (`impl
//! Into<Content<'a, S>>`) you want to format as the argument. For example,
//! here’s how to get some red text:
//!
//! ```
//! use procr_ansi_term::Color::Red;
//!
//! println!("This is in red: {}", Red.paint("a red string"));
//! ```
//!
//! The `paint` method does not return a
//! string with the ANSI control characters surrounding it. Instead, it returns
//! an [`AnsiString`] value that has a [`Display`](std::fmt::Display) (note: not [`Debug`](std::fmt::Debug)! implementation that, when
//! formatted, returns the characters.
//!
//! If you *do* want to get at the escape codes, then you can convert the
//! [`AnsiString`] to a string as you would any other `Display` value:
//!
//! ```
//! use procr_ansi_term::Color::Red;
//!
//! let red_string = Red.paint("a red string").to_string();
//! ```
//!
//!
//! ## Bold, underline, background, and other styles
//!
//! For anything more complex than plain foreground color changes, you need to
//! construct `Style` values themselves, rather than beginning with a `Color`.
//! You can do this by chaining methods based on a new `Style`, created with
//! [`Style::new()`]. Each method creates a new style that has that specific
//! property set. For example:
//!
//! ```
//! use procr_ansi_term::Style;
//!
//! println!("How about some {} and {}?",
//!          Style::new().bold().paint("bold"),
//!          Style::new().underline().paint("underline"));
//! ```
//!
//! For brevity, these methods have also been implemented for `Color` values, so
//! you can give your styles a foreground color without having to begin with an
//! empty `Style` value:
//!
//! ```
//! use procr_ansi_term::Color::{Blue, Yellow};
//!
//! println!("Demonstrating {} and {}!",
//!          Blue.bold().paint("blue bold"),
//!          Yellow.underline().paint("yellow underline"));
//!
//! println!("Yellow on blue: {}", Yellow.on_bg(Blue).paint("wow!"));
//! ```
//!
//! The complete list of styles you can use are: [`bold`], [`dimmed`],
//! [`italic`], [`underline`], [`blink`], [`reverse`], [`hidden`],
//! [`strikethrough`], and [`on`] for background colors.
//!
//! In some cases, you may find it easier to change the foreground on an
//! existing `Style` rather than starting from the appropriate `Color`. You can
//! do this using the [`fg`] method:
//!
//! ```
//! use procr_ansi_term::Style;
//! use procr_ansi_term::Color::{Blue, Cyan, Yellow};
//!
//! println!("Yellow on blue: {}", Style::new().fg(Blue).fg(Yellow).paint("yow!"));
//! println!("Also yellow on blue: {}", Cyan.on_bg(Blue).fg(Yellow).paint("zow!"));
//! ```
//!
//! You can turn a [`Color`] into a [`Style`] with the [`as_fg`](crate::color::as_fg) or
//! [`as_bg`](crate::Color::as_bg) methods. This will
//! produce the exact same `AnsiString` as if you just used the `paint` method
//! on the `Color` directly, but it’s useful in certain cases: for example, you
//! may have a method that returns `Styles`, and need to represent both the “red
//! bold” and “red, but not bold” styles with values of the same type. The
//! `Style` struct also has a [`Default`] implementation if you want to have a
//! style with *nothing* set.
//!
//! ```
//! use procr_ansi_term::Style;
//! use procr_ansi_term::Color::Red;
//!
//! println!("{}", Red.as_fg().paint("yet another red string"));
//! println!("{}", Style::default().paint("a completely regular string"));
//! ```
//!
//!
//! ## Extended colors
//!
//! You can access the extended range of 256 colors by using the `Color::Fixed`
//! variant, which takes an argument of the color number to use. This can be
//! included wherever you would use a `Color`:
//!
//! ```
//! use procr_ansi_term::Color::Fixed;
//!
//! println!("{}", Fixed(134).paint("A sort of light purple"));
//! println!("{}", Fixed(221).on_bg(Fixed(124)).paint("Mustard in the ketchup"));
//! ```
//!
//! The first sixteen of these values are the same as the normal and bold
//! standard color variants. There’s nothing stopping you from using these as
//! `Fixed` colors instead, but there’s nothing to be gained by doing so either.
//!
//! You can also access full 24-bit color by using the `Color::Rgb` variant,
//! which takes separate `u8` arguments for red, green, and blue:
//!
//! ```
//! use procr_ansi_term::Color::Rgb;
//!
//! println!("{}", Rgb(70, 130, 180).paint("Steel blue"));
//! ```
//!
//! ## Combining successive colored strings
//!
//! This crate can (somewhat) optimise the ANSI codes that get printed in
//! situations where it knows exactly which [`AnsiGenericString`]s are to be
//! printed in sequence. Such a sequence is held by the type:
//! [`AnsiGenericStrings`].
//!
//! The following code snippet uses this to enclose a binary number displayed in
//! red bold text inside some red, but not bold, brackets:
//!
//! ```
//! use procr_ansi_term::Color::Red;
//! use procr_ansi_term::{AnsiString, AnsiStrings};
//!
//! let some_value = format!("{:b}", 42);
//! let strings: [AnsiString<'static>] = [
//!     Red.paint("["),
//!     Red.bold().paint(some_value),
//!     Red.paint("]"),
//! ];
//!
//! println!("Value: {}", AnsiStrings(strings));
//! ```
//!
//! ## Byte strings
//!
//! This library also supports formatting `\[u8]` byte strings; this supports
//! applications working with text in an unknown encoding.  [`Style`] and
//! [`Color`] support painting `\[u8]` values, resulting in an
//! [`AnsiByteString`]. This type does not implement [`Display`], as it may not
//! contain UTF-8, but it does provide a method [`write_to`] to write the result
//! to any value that implements [`Write`]:
//!
//! ```
//! use procr_ansi_term::Color::Green;
//!
//! Green.paint("user data".as_bytes()).write_to(&mut std::io::stdout()).unwrap();
//! ```
//!
//! Similarly, the type [`AnsiByteStrings`] supports writing a list of
//! [`AnsiByteString`] values with minimal escape sequences:
//!
//! ```
//! use procr_ansi_term::Color::Green;
//! use procr_ansi_term::AnsiByteStrings;
//!
//! AnsiByteStrings([
//!     Green.paint("user data 1\n".as_bytes()),
//!     Green.bold().paint("user data 2\n".as_bytes()),
//! ]).write_to(&mut std::io::stdout()).unwrap();
//! ```

#![crate_name = "procr_ansi_term"]
#![crate_type = "rlib"]
#![warn(missing_copy_implementations)]
// #![warn(missing_docs)]
#![warn(trivial_casts, trivial_numeric_casts)]
// #![warn(unused_extern_crates, unused_qualifications)]

#[cfg(test)]
doc_comment::doctest!("../README.md");

/// Functionality to map an
/// [`AnsiGenericString`](crate::display::AnsiGenericString) into a sequence of
/// relevant ANSI escape codes.
pub mod ansi;
pub mod utils;
pub use ansi::{Infix, Prefix, Suffix};

mod style;
pub use style::{Color, Style};

mod difference;
/// Functionality to write an ANSI string to [`AnyWrite`] implementors.
mod display;
pub use display::*;

/// Traits and objects which allow writing "generically" to either
/// [`fmt::Write`] or [`io::Write`] implementors.
pub mod write;
pub use write::*;

/// Helpers for managing MS Windows related details.
mod windows;
pub use crate::windows::*;

/// Helpers for debugging ANSI strings.
mod debug;

/// Helpers for creating color gradients.
pub mod gradient;
pub use gradient::*;

/// Helpers for specifying RGB colors.
mod rgb;
pub use rgb::*;

pub use procr_ansi_format::ansi_format;
extern crate self as procr_ansi_term;
