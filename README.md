# procr-ansi-term
#### [Documentation](https://docs.rs/procr_ansi_term/)

## What is this crate useful for?

Styling data that will be eventually rendered by a terminal capable of interpreting ANSI style codes.

## Should I use this crate?

Short answer: no. [`owo_colors`](https://docs.rs/owo-colors/latest/owo_colors/) does everything this crate does, but more rationally, and more effectively.

## History

[`nu-ansi-term`](https://github.com/nushell/nu-ansi-term) is used by [`tracing_subscriber`](https://github.com/tokio-rs/tracing/tree/master/tracing-subscriber) (for what may be considered [historical reasons](https://github.com/tokio-rs/tracing/pull/2040)), and was a copy of `ansi_term` but with `Colour` changed to `Color` and various colors added. `procr-ansi-term`, born in a time when I didn't know better, adds functionality to `nu-ansi-term` which allows ANSI formatting of `format_args!`, and also allows various ANSI strings/format args to be nested in styling, with a primitive parent-child inheritance of said styling.
