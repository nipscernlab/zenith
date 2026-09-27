//! The SOLAR client of ZENITH.
//!
//! Everything ZENITH knows about SOLAR goes through this crate: finding the `solar`
//! binary, starting `solar serve --stdio`, writing requests and reading responses one
//! line at a time, and making sense of the manifest. It has no terminal code, so the
//! tests can drive it against a real SOLAR on its own.
//!
//! It depends on no SOLAR crate and reads no SOLAR source. What it knows of SOLAR is
//! SOLAR's contract, `docs/CONTRACT.md` in the SOLAR repository, and what SOLAR says at
//! run time.
//!
//! | Module | What it answers |
//! | ------ | --------------- |
//! | [`locate`] | Where `solar` is, and on Windows the copy that runs |
//! | [`connection`] | The child process and the threads that read and write its pipes |
//! | [`calls`] | Which waiting call a response answers |
//! | [`envelope`] | What a response says, and whether its shape keeps the contract |
//! | [`handshake`] | Whether this SOLAR speaks the protocol and the layout ZENITH knows |
//! | [`manifest`] | The catalogue of APIs |
//! | [`schema`], [`schema_view`] | Validation, and schemas read as descriptions |
//! | [`json_text`], [`pointer`](mod@pointer) | JSON as typed text, and places inside it |
//! | [`example`] | Whether a response matches the example that produced it |

/// The version of this build, which is ZENITH's version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod calls;
pub mod connection;
pub mod envelope;
pub mod example;
pub mod handshake;
pub mod json_text;
pub mod locate;
pub mod manifest;
pub mod pointer;
pub mod schema;
pub mod schema_view;
