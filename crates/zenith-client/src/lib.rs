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
