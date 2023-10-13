use crate::ansi::RESET;
use crate::difference::StyleDelta;
use crate::style::{BasedOn, Color, Style};
use crate::write::{AnyWrite, Content, StrLike, WriteResult};
use crate::{fmt_write, io_write, write_fmt, write_str};
use std::borrow::Cow;
use std::cell::{Ref, RefCell, RefMut};
use std::fmt::{self, Debug};
use std::io;

/// Represents various features that require "OS Control" ANSI codes.
pub enum OSControl<'a, S: 'a + ToOwned + ?Sized> {
    /// Set the title of a terminal window.
    Title,
    /// Create a clickable-link.
    Link {
        /// The url underlying the clickable link.
        url: Content<'a, S>,
    },
}

/// We manually implement [`Debug`](fmt::Debug) so that it is specifically only
/// implemented when `S` also implements `Debug`.
impl<'a, S: 'a + ToOwned + ?Sized> Debug for OSControl<'a, S>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Title => write!(f, "Title"),
            Self::Link { url } => f.debug_struct("Link").field("url", url).finish(),
        }
    }
}

impl<'a, S: 'a + ToOwned + ?Sized> Clone for OSControl<'a, S> {
    fn clone(&self) -> Self {
        match self {
            Self::Link { url: u } => Self::Link { url: u.clone() },
            Self::Title => Self::Title,
        }
    }
}

/// An `AnsiGenericString` includes a generic string type and a `Style` to
/// display that string.  `AnsiString` and `AnsiByteString` are aliases for
/// this type on `str` and `\[u8]`, respectively.
pub struct AnsiGenericString<'a, S: 'a + ToOwned + ?Sized> {
    pub(crate) style: Style,
    pub(crate) content: Content<'a, S>,
    oscontrol: Option<OSControl<'a, S>>,
}

/// We manually implement [`Debug`](fmt::Debug) so that it is specifically only
/// implemented when `S` also implements `Debug`.
impl<'a, S: 'a + ToOwned + ?Sized> Debug for AnsiGenericString<'a, S>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnsiGenericString")
            .field("style", &self.style)
            .field("content", &self.content)
            .field("oscontrol", &self.oscontrol)
            .finish()
    }
}

/// Cloning an `AnsiGenericString` will clone its underlying string.
///
/// # Examples
///
/// ```
/// use procr_ansi_term::AnsiString;
///
/// let plain_string = AnsiString::from("a plain string");
/// let clone_string = plain_string.clone();
/// assert_eq!(clone_string.to_string(), plain_string.to_string());
/// ```
impl<'a, S: 'a + ToOwned + ?Sized> Clone for AnsiGenericString<'a, S> {
    fn clone(&self) -> AnsiGenericString<'a, S> {
        AnsiGenericString {
            style: self.style,
            content: self.content.clone(),
            oscontrol: self.oscontrol.clone(),
        }
    }
}

// You might think that the hand-written Clone impl above is the same as the
// one that gets generated with #[derive]. But it’s not *quite* the same!
//
// `str` is not Clone, and the derived Clone implementation puts a Clone
// constraint on the S type parameter (generated using --pretty=expanded):
//
//                  ↓_________________↓
//     impl <'a, S: ::std::clone::Clone + 'a + ToOwned + ?Sized> ::std::clone::Clone
//     for ANSIGenericString<'a, S> where
//     <S as ToOwned>::Owned: fmt::Debug { ... }
//
// This resulted in compile errors when you tried to derive Clone on a type
// that used it:
//
//     #[derive(PartialEq, Debug, Clone, Default)]
//     pub struct TextCellContents(Vec<AnsiString<'static>>);
//                                 ^^^^^^^^^^^^^^^^^^^^^^^^^
//     error[E0277]: the trait `std::clone::Clone` is not implemented for `str`
//
// The hand-written impl above can ignore that constraint and still compile.

impl<'a, S: 'a + ToOwned + ?Sized> From<&'a S> for AnsiGenericString<'a, S>
where
    S: AsRef<S>,
{
    fn from(s: &'a S) -> Self {
        AnsiGenericString {
            style: Style::default(),
            content: s.into(),
            oscontrol: None,
        }
    }
}

impl<'a, S: 'a + ToOwned + ?Sized> From<fmt::Arguments<'a>> for AnsiGenericString<'a, S> {
    fn from(args: fmt::Arguments<'a>) -> Self {
        AnsiGenericString {
            style: Style::default(),
            content: args.into(),
            oscontrol: None,
        }
    }
}

impl<'a, S: 'a + ToOwned + ?Sized> From<AnsiGenericStrings<'a, S>> for AnsiGenericString<'a, S> {
    fn from(strings: AnsiGenericStrings<'a, S>) -> Self {
        AnsiGenericString {
            style: Style::default(),
            content: strings.into(),
            oscontrol: None,
        }
    }
}

/// An ANSI String is a string coupled with the `Style` to display it
/// in a terminal.
///
/// Although not technically a string itself, it can be turned into
/// one with the `to_string` method.
///
/// # Examples
///
/// ```
/// use procr_ansi_term::AnsiString;
/// use procr_ansi_term::Color::Red;
///
/// let red_string = Red.paint("a red string");
/// println!("{}", red_string);
/// ```
///
/// ```
/// use procr_ansi_term::AnsiString;
///
/// let plain_string = AnsiString::from("a plain string");
/// ```
pub type AnsiString<'a> = AnsiGenericString<'a, str>;

/// An `AnsiByteString` represents a formatted series of bytes.  Use
/// `AnsiByteString` when styling text with an unknown encoding.
pub type AnsiByteString<'a> = AnsiGenericString<'a, [u8]>;

impl<'a, S: 'a + ToOwned + ?Sized> AnsiGenericString<'a, S> {
    /// Create an [`AnsiByteString`] from the given data.
    pub const fn new(
        style: Style,
        content: Content<'a, S>,
        oscontrol: Option<OSControl<'a, S>>,
    ) -> Self {
        Self {
            style,
            content,
            oscontrol,
        }
    }

    /// Directly access the style
    pub const fn style_ref(&self) -> &Style {
        &self.style
    }

    /// Directly access the style mutably
    pub fn style_ref_mut(&mut self) -> &mut Style {
        &mut self.style
    }

    /// Get the (text) content in this generic string.
    pub const fn content(&self) -> &Content<'a, S> {
        &self.content
    }

    /// Get the [`OSControl`] settings associated with this generic string, if
    /// any exist.
    pub const fn oscontrol(&self) -> &Option<OSControl<'a, S>> {
        &self.oscontrol
    }

    // Instances that imply wrapping in OSC sequences
    // and do not get displayed in the terminal text
    // area.
    //
    /// Produce an ANSI string that changes the title shown
    /// by the terminal emulator.
    ///
    /// # Examples
    ///
    /// ```
    /// use procr_ansi_term::AnsiGenericString;
    /// let title_string = AnsiGenericString::title("My Title");
    /// println!("{}", title_string);
    /// ```
    /// Should produce an empty line but set the terminal title.
    pub fn title_content<I>(s: I) -> Self
    where
        I: Into<Content<'a, S>>,
    {
        Self {
            style: Style::new(),
            content: s.into(),
            oscontrol: Some(OSControl::<S>::Title),
        }
    }

    /// Produce an ANSI string that changes the title shown
    /// by the terminal emulator. This is a const function which can only accept
    /// `&str` or `&[u8]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use procr_ansi_term::AnsiGenericString;
    /// let title_string = AnsiGenericString::title("My Title");
    /// println!("{}", title_string);
    /// ```
    /// Should produce an empty line but set the terminal title.
    pub const fn title(s: &'a S) -> Self {
        Self {
            style: Style::new(),
            content: Content::StrLike(Cow::Borrowed(s)),
            oscontrol: Some(OSControl::<S>::Title),
        }
    }

    /// Produce an ANSI string that changes the title shown
    /// by the terminal emulator. This is a const function which can only accept
    /// a [`fmt::Argument`].
    ///
    /// # Examples
    ///
    /// ```
    /// use procr_ansi_term::AnsiGenericString;
    /// let title_string = AnsiGenericString::title("My Title");
    /// println!("{}", title_string);
    /// ```
    /// Should produce an empty line but set the terminal title.
    pub const fn title_fmt_arg(s: fmt::Arguments<'a>) -> Self {
        Self {
            style: Style::new(),
            content: Content::FmtArgs(s),
            oscontrol: Some(OSControl::<S>::Title),
        }
    }

    //
    // Annotations (OSC sequences that do more than wrap)
    //

    /// Cause the styled ANSI string to link to the given URL
    ///
    /// # Examples
    ///
    /// ```
    /// use procr_ansi_term::Color::Red;
    ///
    /// let link_string = Red.paint("a red string").hyperlink_content(String::from("https://www.example.com"));
    /// println!("{}", link_string);
    /// ```
    /// Should show a red-painted string which, on terminals
    /// that support it, is a clickable hyperlink.
    pub fn hyperlink_content<I>(mut self, url: I) -> Self
    where
        I: Into<Content<'a, S>>,
    {
        self.oscontrol = Some(OSControl::Link { url: url.into() });
        self
    }

    /// Cause the styled ANSI string to link to the given URL. This is a const
    /// fn which can only accept `&str` or `&[u8]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use procr_ansi_term::Color::Red;
    ///
    /// let link_string = Red.paint("a red string").hyperlink("https://www.example.com");
    /// println!("{}", link_string);
    /// ```
    /// Should show a red-painted string which, on terminals
    /// that support it, is a clickable hyperlink.
    pub fn hyperlink(self, url: &'a S) -> Self {
        Self {
            style: self.style,
            content: self.content,
            oscontrol: Some(OSControl::Link {
                url: Content::StrLike(Cow::Borrowed(url)),
            }),
        }
    }

    /// Get the url content for this string's oscontrol.
    pub const fn url_string(&self) -> Option<&Content<'a, S>> {
        if let Some(osc) = &self.oscontrol {
            match osc {
                OSControl::Title => {}
                OSControl::Link { url } => {
                    return Some(url);
                }
            }
        }
        None
    }
}

/// A set of `AnsiGenericStrings`s collected together, in order to be
/// written with a minimum of control characters.
pub struct AnsiGenericStrings<'a, S: 'a + ToOwned + ?Sized> {
    strings: Cow<'a, [AnsiGenericString<'a, S>]>,
    style_updates: RefCell<Cow<'a, [StyleUpdate]>>,
}

impl<'a, S: 'a + ToOwned + ?Sized> From<AnsiGenericString<'a, S>> for AnsiGenericStrings<'a, S> {
    fn from(value: AnsiGenericString<'a, S>) -> Self {
        let style = value.style;
        Self {
            strings: Cow::Owned(vec![value]),
            style_updates: RefCell::new(Cow::Owned(vec![StyleUpdate {
                style_delta: StyleDelta::ExtraStyles(style),
                begins_at: 0,
            }])),
        }
    }
}

impl<'a, S: 'a + ToOwned + ?Sized> Clone for AnsiGenericStrings<'a, S> {
    fn clone(&self) -> Self {
        Self {
            style_updates: RefCell::new(self.style_updates.borrow_mut().clone()),
            strings: self.strings.clone(),
        }
    }
}

/// We manually implement [`Debug`](fmt::Debug) so that it is specifically only
/// implemented when `S` also implements `Debug`.
impl<'a, S: 'a + ToOwned + ?Sized> Debug for AnsiGenericStrings<'a, S>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnsiGenericStrings")
            .field("strings", &self.strings)
            .field("style_updates", &self.style_updates.borrow_mut())
            .finish()
    }
}

impl<'a, S: 'a + ToOwned + ?Sized> AnsiGenericStrings<'a, S> {
    pub const fn new(strings: &'a [AnsiGenericString<'a, S>]) -> Self {
        Self {
            strings: Cow::Borrowed(strings),
            style_updates: RefCell::new(Cow::Borrowed(&[])),
        }
    }
    /// Create empty sequence with the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            strings: Vec::with_capacity(capacity).into(),
            style_updates: RefCell::new(Vec::with_capacity(capacity).into()),
        }
    }

    /// Iterate over the underlying generic strings.
    pub fn iter(&self) -> impl Iterator<Item = &'_ AnsiGenericString<'a, S>> {
        self.strings.iter()
    }

    fn calculate_style_updates(&self) {
        let mut style_updates = Vec::with_capacity(self.strings.len());
        for (ix, string) in self.strings.iter().enumerate() {
            Self::push_style_into(&mut style_updates, string.style, ix);
        }
        *self.style_updates.borrow_mut() = Cow::Owned(style_updates);
    }

    /// Get the style updates required to build this string.
    ///
    /// If they are not yet computed, they will be computed, otherwise the cached updates will be returned.
    fn style_updates(&self) -> Ref<'_, Cow<'_, [StyleUpdate]>> {
        if self.strings.len() != self.style_updates.borrow().len() {
            self.calculate_style_updates();
        }
        self.style_updates.borrow()
    }

    /// Get mutable access to the style updates required to build this string.
    ///
    /// If they are not yet computed, they will be computed, otherwise the cached updates will be returned.
    fn style_updates_mut(&self) -> RefMut<'_, Cow<'a, [StyleUpdate]>> {
        if self.strings.len() != self.style_updates.borrow().len() {
            self.calculate_style_updates();
        }
        self.style_updates.borrow_mut()
    }

    /// Update specific generic strings.
    ///
    /// Depending on where the updates are made, not all style deltas will be
    /// re-computed.
    pub fn update_strings(
        &mut self,
        updates: impl IntoIterator<Item = (usize, AnsiGenericString<'a, S>)>,
    ) -> Self {
        let mut updates: Vec<(usize, AnsiGenericString<'a, S>)> = updates.into_iter().collect();

        if updates.is_empty() {
            return self.clone();
        }

        // Now we know updates are not empty.
        updates.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        let min_changed_ix = updates.first().unwrap().0;

        let mut new_strings = self.strings.to_vec();
        let original_len = new_strings.len();

        for (u_ix, u) in updates.into_iter() {
            if u_ix < original_len {
                new_strings[u_ix] = u;
            } else {
                new_strings.push(u);
            }
        }

        if min_changed_ix < original_len {
            let unchanged_existing = &self.style_updates()[0..min_changed_ix];
            let mut new_style_updates = Vec::with_capacity(new_strings.len());
            new_style_updates.extend(unchanged_existing);

            for (ix, style) in new_strings[min_changed_ix..]
                .iter()
                .map(|s| s.style)
                .enumerate()
            {
                Self::push_style_into(&mut new_style_updates, style, ix + min_changed_ix)
            }

            Self {
                strings: Cow::Owned(new_strings),
                style_updates: RefCell::new(Cow::Owned(new_style_updates)),
            }
        } else {
            Self::from_iter(new_strings)
        }
    }

    /// Rebase a nested string onto a parent's style. This is effectively an
    /// "OR" operation.
    pub fn rebase_on(self, base: Style) -> Self {
        for update in self.style_updates_mut().to_mut().iter_mut() {
            update.style_delta = match update.style_delta {
                StyleDelta::ExtraStyles(style) => StyleDelta::ExtraStyles(if style.reset_prefix {
                    style.rebase_on(base)
                } else {
                    style
                }),
                StyleDelta::Empty => StyleDelta::Empty,
            };
        }
        self
    }

    /// Push given generic string into this [`AnsiGenericStrings`] instance.
    #[inline]
    pub fn push(&mut self, s: AnsiGenericString<'a, S>) {
        self.strings.to_mut().push(s.clone());
        self.push_style(*s.style_ref(), self.strings.len() - 1);
    }

    #[inline]
    fn push_style_into(
        existing_style_updates: &mut Vec<StyleUpdate>,
        next: Style,
        begins_at: usize,
    ) {
        let command = existing_style_updates
            .last()
            .copied()
            .unwrap_or_default()
            .style_delta
            .delta_next(next);

        existing_style_updates.push(StyleUpdate {
            begins_at,
            style_delta: command,
        });
    }

    #[inline]
    fn push_style(&self, next: Style, begins_at: usize) {
        Self::push_style_into(self.style_updates.borrow_mut().to_mut(), next, begins_at)
    }

    fn write_iter(&self) -> WriteIter<'_, 'a, S> {
        WriteIter {
            style_iter: StyleIter {
                cursor: 0,
                instructions: self.style_updates.borrow(),
                next_update: None,
                current: None,
            },
            content_iter: ContentIter {
                cursor: 0,
                strings: &self.strings,
            },
        }
    }
}

/// Iterator over the minimal styles (see [`StyleDelta`]) of an [`AnsiGenericStrings`] sequence.
pub struct StyleIter<'b> {
    cursor: usize,
    instructions: Ref<'b, Cow<'b, [StyleUpdate]>>,
    next_update: Option<StyleUpdate>,
    current: Option<StyleUpdate>,
}

/// The [`StyleDelta`] to be applied before the contents of the string at
/// position `begin_at`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct StyleUpdate {
    style_delta: StyleDelta,
    begins_at: usize,
}

impl<'b> StyleIter<'b> {
    fn get_next_update(&mut self) {
        self.cursor += 1;
        self.next_update = self.instructions.get(self.cursor).copied();
    }
}

impl<'b> Iterator for StyleIter<'b> {
    type Item = StyleDelta;

    fn next(&mut self) -> Option<Self::Item> {
        match (self.current, self.next_update) {
            (None, None) => {
                self.current = self.instructions.get(self.cursor).copied();
                self.get_next_update();
                self.current
            }
            (current, Some(next_update)) => {
                if self.cursor < next_update.begins_at {
                    current
                } else {
                    self.current = self.next_update.take();
                    self.get_next_update();
                    self.current
                }
            }
            (Some(current), None) => current.into(),
        }
        .map(|u| u.style_delta)
    }
}

/// An iterator over the contents in an [`AnsiGenericStrings`] sequence.
pub struct ContentIter<'b, 'a, S: 'a + ToOwned + ?Sized> {
    cursor: usize,
    strings: &'b [AnsiGenericString<'a, S>],
}

impl<'b, 'a, S: 'a + ToOwned + ?Sized> Iterator for ContentIter<'b, 'a, S> {
    type Item = (Content<'a, S>, Option<OSControl<'a, S>>);

    fn next(&mut self) -> Option<Self::Item> {
        let r = self
            .strings
            .get(self.cursor)
            .map(|s| (s.content.clone(), s.oscontrol.clone()));
        if r.is_some() {
            self.cursor += 1;
        }
        r
    }
}

/// An iterator over the data required to write out an [`AnsiGenericStrings`]
/// sequence to an [`AnyWrite`] implementor.
pub struct WriteIter<'b, 'a, S: 'a + ToOwned + ?Sized> {
    style_iter: StyleIter<'b>,
    content_iter: ContentIter<'b, 'a, S>,
}

impl<'b, 'a, S: 'a + ToOwned + ?Sized> Iterator for WriteIter<'b, 'a, S> {
    type Item = (StyleDelta, Content<'a, S>, Option<OSControl<'a, S>>);

    fn next(&mut self) -> Option<Self::Item> {
        let (content, oscontrol) = self.content_iter.next()?;
        let update_command = self.style_iter.next().unwrap_or_default();
        Some((update_command, content, oscontrol))
    }
}

impl<'a, S: 'a + ToOwned + ?Sized> FromIterator<AnsiGenericString<'a, S>>
    for AnsiGenericStrings<'a, S>
{
    fn from_iter<Iterable: IntoIterator<Item = AnsiGenericString<'a, S>>>(iter: Iterable) -> Self {
        let iter = iter.into_iter();
        let (lower, upper) = iter.size_hint();
        let count = upper.unwrap_or(lower);
        let mut ansi_strings = AnsiGenericStrings::with_capacity(count);
        for s in iter {
            ansi_strings.push(s);
        }
        ansi_strings
    }
}

/// A set of `AnsiString`s collected together, in order to be written with a
/// minimum of control characters.
pub type AnsiStrings<'a> = AnsiGenericStrings<'a, str>;

/// A function to construct an `AnsiStrings` instance.
#[allow(non_snake_case)]
pub fn AnsiStrings<'a>(arg: impl IntoIterator<Item = AnsiString<'a>>) -> AnsiStrings<'a> {
    AnsiGenericStrings::from_iter(arg)
}

/// A set of `AnsiByteString`s collected together, in order to be
/// written with a minimum of control characters.
pub type AnsiByteStrings<'a> = AnsiGenericStrings<'a, [u8]>;

/// A function to construct an `AnsiByteStrings` instance.
#[allow(non_snake_case)]
pub fn AnsiByteStrings<'a>(
    arg: impl IntoIterator<Item = AnsiByteString<'a>>,
) -> AnsiByteStrings<'a> {
    AnsiGenericStrings::from_iter(arg)
}

// ---- paint functions ----
impl Style {
    /// Paints the given content with this style, returning an ANSI string.
    ///
    /// ```
    /// use procr_ansi_term::Style;
    /// use procr_ansi_term::Color;
    ///
    /// println!("{}", Style::new().fg(Color::Blue).paint("nice!"));
    /// ```
    #[inline]
    #[must_use]
    pub fn paint<'a, I, S: 'a + ToOwned + ?Sized>(self, input: I) -> AnsiGenericString<'a, S>
    where
        I: Into<Content<'a, S>>,
    {
        AnsiGenericString {
            content: match input.into() {
                x @ Content::GenericStrings(_) => x.with_context(self),
                x => x,
            },
            style: self,
            oscontrol: None,
        }
    }
}

impl Color {
    /// Paints the given text with this color, returning an ANSI string.
    /// This is a short-cut so you don’t have to use `Blue.as_fg()` just
    /// to get blue text.
    ///
    /// ```
    /// use procr_ansi_term::Color::Blue;
    /// println!("{}", Blue.paint("da ba dee"));
    /// ```
    #[inline]
    #[must_use]
    pub fn paint<'a, I, S: 'a + ToOwned + ?Sized>(self, input: I) -> AnsiGenericString<'a, S>
    where
        I: Into<Content<'a, S>>,
    {
        self.as_fg().paint(input)
    }
}

// ---- writers for individual ANSI strings ----

impl<'a> fmt::Display for AnsiString<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.write_to_any(fmt_write!(f))
    }
}

impl<'a> AnsiByteString<'a> {
    /// Write an `AnsiByteString` to an `io::Write`.  This writes the escape
    /// sequences for the associated `Style` around the bytes.
    pub fn write_to<W: io::Write>(&self, w: &mut W) -> io::Result<()> {
        self.write_to_any(io_write!(w))
    }
}

impl<'a, S: 'a + ToOwned + ?Sized> AnsiGenericString<'a, S> {
    /// Rebase this style on a `base` style (effective an "OR" operation).
    /// Useful for ANSI strings nested inside other ANSI strings.
    pub fn rebase_on(mut self, base: Style) -> Self {
        self.style = self.style.rebase_on(base);
        self
    }
    /// Write only the part of the generic string which lies within its styling
    /// prefix and suffix: its `content` and `oscontrol`.
    pub fn write_inner<W: AnyWrite + ?Sized>(
        content: &Content<'a, S>,
        oscontrol: &Option<OSControl<'a, S>>,
        w: &mut W,
    ) -> WriteResult<W::Error>
    where
        S: StrLike<'a, W>,
        str: StrLike<'a, W>,
    {
        match oscontrol {
            Some(OSControl::Link { url: u, .. }) => {
                write_str!(w, "\x1B]8;;")?;
                u.write_to(w)?;
                write_str!(w, "\x1B\x5C")?;
                content.write_to(w)?;
                write_str!(w, "\x1B]8;;\x1B\x5C")
            }
            Some(OSControl::Title) => {
                write_str!(w, "\x1B]2;")?;
                content.write_to(w)?;
                write_str!(w, "\x1B\x5C")
            }
            None => content.write_to(w),
        }
    }

    /// Write this generic string to the given `AnyWrite` implementor.
    pub fn write_to_any<W: AnyWrite + ?Sized>(&self, w: &mut W) -> WriteResult<W::Error>
    where
        S: StrLike<'a, W>,
        str: StrLike<'a, W>,
    {
        write_fmt!(w, "{}", self.style.prefix())?;
        Self::write_inner(&self.content, &self.oscontrol, w)?;
        write_fmt!(w, "{}", self.style.suffix())
    }
}

// ---- writers for combined ANSI strings ----

impl<'a> fmt::Display for AnsiStrings<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.write_to_any(fmt_write!(f))
    }
}

impl<'a> AnsiByteStrings<'a> {
    /// Write `AnsiByteStrings` to an `io::Write`.  This writes the minimal
    /// escape sequences for the associated `Style`s around each set of
    /// bytes.
    pub fn write_to<W: io::Write>(&self, w: &mut W) -> io::Result<()> {
        self.write_to_any(io_write!(w))
    }
}

impl<'a, S: 'a + ToOwned + ?Sized> AnsiGenericStrings<'a, S> {
    /// Write this sequence to the given [`AnyWrite`] implementor.
    pub fn write_to_any<W: AnyWrite + ?Sized>(&self, w: &mut W) -> WriteResult<W::Error>
    where
        S: StrLike<'a, W>,
        str: StrLike<'a, W>,
    {
        let mut last_is_plain = true;

        for (style_command, content, oscontrol) in self.write_iter() {
            match style_command {
                StyleDelta::ExtraStyles(style) => {
                    style.write_prefix(w)?;
                    last_is_plain = style.has_no_styling();
                }
                StyleDelta::Empty => {}
            }
            AnsiGenericString::write_inner(&content, &oscontrol, w)?;
        }

        if last_is_plain {
            dbg!(last_is_plain);
            Ok(())
        } else {
            w.write_str(RESET.as_ref())
        }
    }
}

// ---- tests ----

#[cfg(test)]
mod tests {
    pub use super::super::{AnsiGenericString, AnsiStrings};
    use crate::assert_required;
    pub use crate::style::Color::*;
    pub use crate::style::Style;

    #[test]
    fn no_control_codes_for_plain() {
        let one = Style::default().paint("one");
        let two = Style::default().paint("two");
        let output = AnsiStrings([one, two]).to_string();
        assert_eq!(output, "onetwo");
    }

    #[test]
    fn title_solo() {
        let unstyled = AnsiGenericString::title("hello");

        let joined = AnsiStrings([unstyled.clone()]).to_string();
        let expected = "\x1B]2;hello\x1B\\";
        assert_required!(joined, expected);
    }

    #[test]
    fn title_pre_plain() {
        let unstyled = AnsiGenericString::title("hello");
        let after = Style::default().paint(" After is Plain.");

        // does not introduce spurious SGR codes (reset or otherwise) adjacent
        // to plain strings
        let joined = AnsiStrings([unstyled.clone(), after.clone()]).to_string();
        let expected = format!("{}{}", unstyled, after);
        assert_required!(joined, expected);
    }

    #[test]
    fn title_post_plain() {
        let unstyled = AnsiGenericString::title("hello");
        let before = Style::default().paint("Before is Plain. ");

        // does not introduce spurious SGR codes (reset or otherwise) adjacent
        // to plain strings
        let joined = AnsiStrings([before.clone(), unstyled.clone()]).to_string();
        let expected = format!("{}{}", before.clone(), unstyled);
        assert_required!(joined, expected);
    }

    #[test]
    fn title_middle_plain() {
        let unstyled = AnsiGenericString::title("hello");
        let after = Style::default().paint(" After is Plain.");
        let before = Style::default().paint("Before is Plain. ");

        // does not introduce spurious SGR codes (reset or otherwise) adjacent
        // to plain strings
        let joined = AnsiStrings([before.clone(), unstyled.clone(), after.clone()]).to_string();
        let expected = format!("{}{}{}", before, unstyled, after);
        assert_required!(joined, expected);
    }

    #[test]
    fn title_pre_styled() {
        let unstyled = AnsiGenericString::title("hello");
        let after_g = Green.paint(" After is Green.");

        // Check that RESET does not follow unstyled
        let joined = AnsiStrings([unstyled.clone(), after_g.clone()]).to_string();
        let expected = format!("{}{}", unstyled, {
            format_args!(
                "{}{}{}",
                after_g.style.prefix(),
                after_g.content.to_string(),
                after_g.style.suffix()
            )
        });
        assert_required!(joined, expected);
    }

    #[test]
    fn title_post_styled() {
        let unstyled = AnsiGenericString::title("hello");
        let before_g = Green.paint("Before is Green.");

        // Check that reset precedes unstyled, but does not follow it
        let joined = AnsiStrings([before_g.clone(), unstyled.clone()]).to_string();
        let expected = format!(
            "{}{}",
            format_args!(
                "{}{}{}",
                before_g.style.prefix().to_string(),
                before_g.content.to_string(),
                before_g.style.suffix().to_string()
            ),
            unstyled
        );
        assert_required!(joined, expected);
    }

    #[test]
    fn title_middle_styled() {
        let unstyled = AnsiGenericString::title("hello");
        let before_g = Green.paint("Before is Green.");
        let after_g = Green.paint(" After is Green.");

        let joined = AnsiStrings([before_g.clone(), unstyled.clone(), after_g.clone()]).to_string();
        let expected = format!(
            "{}{}{}",
            format_args!(
                "{}{}{}",
                before_g.style.prefix().to_string(),
                before_g.content.to_string(),
                before_g.style.suffix().to_string()
            ),
            unstyled,
            format_args!(
                "{}{}{}",
                after_g.style.prefix(),
                after_g.content.to_string(),
                after_g.style.suffix()
            )
        );
        assert_required!(joined, expected);
    }

    #[test]
    fn hyperlink() {
        let styled = Red
            .paint("Link to example.com.")
            .hyperlink_content("https://example.com");
        assert_eq!(
            styled.to_string(),
            "\x1B[31m\x1B]8;;https://example.com\x1B\\Link to example.com.\x1B]8;;\x1B\\\x1B[0m"
        );
    }

    #[test]
    fn hperlinks_link_only() {
        let link = Blue
            .underline()
            .paint("Link to example.com.")
            .hyperlink_content("https://example.com");
        dbg!("link: {:?}", &link);
        // Assemble with link by itself
        let joined = AnsiStrings([link.clone()]).to_string();
        #[cfg(feature = "gnu_legacy")]
        assert_eq!(joined, format!("\x1B[04;34m\x1B]8;;https://example.com\x1B\\Link to example.com.\x1B]8;;\x1B\\\x1B[0m"));
        #[cfg(not(feature = "gnu_legacy"))]
        assert_eq!(joined, format!("\x1B[4;34m\x1B]8;;https://example.com\x1B\\Link to example.com.\x1B]8;;\x1B\\\x1B[0m"));
    }

    #[test]
    fn hyperlinks_link_first() {
        let link = Blue
            .underline()
            .paint("Link to example.com.")
            .hyperlink_content("https://example.com");
        dbg!("link: {:?}", &link);
        let after = Green.paint(" After link.");
        // Assemble with link first
        let joined = AnsiStrings([link.clone(), after.clone()]).to_string();
        #[cfg(feature = "gnu_legacy")]
        assert_eq!(joined, format!("\x1B[04;34m\x1B]8;;https://example.com\x1B\\Link to example.com.\x1B]8;;\x1B\\\x1B[0m\x1B[32m After link.\x1B[0m"));
        #[cfg(not(feature = "gnu_legacy"))]
        assert_eq!(joined, format!("\x1B[4;34m\x1B]8;;https://example.com\x1B\\Link to example.com.\x1B]8;;\x1B\\\x1B[0m\x1B[32m After link.\x1B[0m"));
    }

    #[test]
    fn hyperlinks_link_last() {
        let before = Green.paint("Before link. ");
        let link = Blue
            .underline()
            .paint("Link to example.com.")
            .hyperlink_content("https://example.com");
        dbg!("link: {:?}", &link);
        // Assemble with link at the end
        let joined = AnsiStrings([before.clone(), link.clone()]).to_string();
        #[cfg(feature = "gnu_legacy")]
        assert_eq!(joined, format!("\x1B[32mBefore link. \x1B[04;34m\x1B]8;;https://example.com\x1B\\Link to example.com.\x1B]8;;\x1B\\\x1B[0m"));
        #[cfg(not(feature = "gnu_legacy"))]
        assert_eq!(joined, format!("\x1B[32mBefore link. \x1B[4;34m\x1B]8;;https://example.com\x1B\\Link to example.com.\x1B]8;;\x1B\\\x1B[0m"));
    }

    #[test]
    fn hyperlinks_link_in_middle() {
        let before = Green.paint("Before link. ");
        let link = Blue
            .underline()
            .paint("Link to example.com.")
            .hyperlink_content("https://example.com");
        dbg!("link: {:?}", &link);
        let after = Green.paint(" After link.");
        dbg!("link: {:?}", &link);
        // Assemble with link in the middle
        let joined = AnsiStrings([before.clone(), link.clone(), after.clone()]).to_string();
        #[cfg(feature = "gnu_legacy")]
        assert_eq!(joined, format!("\x1B[32mBefore link. \x1B[04;34m\x1B]8;;https://example.com\x1B\\Link to example.com.\x1B]8;;\x1B\\\x1B[0m\x1B[32m After link.\x1B[0m"));
        #[cfg(not(feature = "gnu_legacy"))]
        assert_eq!(joined, format!("\x1B[32mBefore link. \x1B[4;34m\x1B]8;;https://example.com\x1B\\Link to example.com.\x1B]8;;\x1B\\\x1B[0m\x1B[32m After link.\x1B[0m"));
    }
}
