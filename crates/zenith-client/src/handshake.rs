//! What ZENITH checks when it connects: that it speaks the protocol SOLAR speaks and reads
//! the manifest layout SOLAR writes.
//!
//! The two versions are what lets ZENITH and SOLAR evolve apart. When either differs from
//! what this ZENITH knows, the connection stops with both versions named, rather than
//! failing in some stranger way later.

use std::fmt;

use serde_json::Value;

use crate::manifest::{LAYOUT_MAJOR, major};

/// The protocol this ZENITH speaks.
pub const PROTOCOL: &str = "solar/1";

/// What `solar.version` says about the build that answered.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerInfo {
    /// `solar_version`.
    pub solar_version: String,
    /// `protocol`.
    pub protocol: String,
    /// `manifest_schema_version`.
    pub manifest_schema_version: String,
    /// `build`, as SOLAR reports it.
    pub build: Value,
}

/// Why the handshake stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandshakeError {
    /// `solar.version` did not answer with the members the handshake needs.
    Malformed {
        /// What was missing.
        what: String,
    },
    /// SOLAR speaks another protocol.
    Protocol {
        /// The SOLAR that answered.
        solar_version: String,
        /// Its protocol.
        theirs: String,
    },
    /// SOLAR writes its manifest in a layout with another major version.
    Layout {
        /// The SOLAR that answered.
        solar_version: String,
        /// Its layout.
        theirs: String,
    },
}

impl fmt::Display for HandshakeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let zenith = crate::VERSION;
        match self {
            Self::Malformed { what } => write!(
                formatter,
                "SOLAR answered solar.version without {what}, so ZENITH cannot tell which \
                 protocol it speaks."
            ),
            Self::Protocol {
                solar_version,
                theirs,
            } => write!(
                formatter,
                "SOLAR {solar_version} speaks {theirs}, and ZENITH {zenith} speaks {PROTOCOL}. \
                 Use a ZENITH that speaks {theirs}, or a SOLAR that speaks {PROTOCOL}."
            ),
            Self::Layout {
                solar_version,
                theirs,
            } => write!(
                formatter,
                "SOLAR {solar_version} writes its manifest in layout {theirs}, and ZENITH \
                 {zenith} reads layout {LAYOUT_MAJOR}. Use a ZENITH that reads layout {theirs}, \
                 or a SOLAR whose manifest is in layout {LAYOUT_MAJOR}."
            ),
        }
    }
}

impl std::error::Error for HandshakeError {}

/// Reads the `data` of `solar.version` and checks the protocol, then the layout.
///
/// # Errors
///
/// [`HandshakeError`] naming the first check that failed, with both versions.
pub fn check_version(data: &Value) -> Result<ServerInfo, HandshakeError> {
    let text = |key: &str| {
        data.get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| HandshakeError::Malformed {
                what: key.to_owned(),
            })
    };
    let info = ServerInfo {
        solar_version: text("solar_version")?,
        protocol: text("protocol")?,
        manifest_schema_version: text("manifest_schema_version")?,
        build: data.get("build").cloned().unwrap_or(Value::Null),
    };
    if info.protocol != PROTOCOL {
        return Err(HandshakeError::Protocol {
            solar_version: info.solar_version,
            theirs: info.protocol,
        });
    }
    if major(&info.manifest_schema_version) != Some(LAYOUT_MAJOR) {
        return Err(HandshakeError::Layout {
            solar_version: info.solar_version,
            theirs: info.manifest_schema_version,
        });
    }
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn version(protocol: &str, layout: &str) -> Value {
        json!({
            "solar_version": "0.3.0",
            "protocol": protocol,
            "manifest_schema_version": layout,
            "build": {"profile": "release"}
        })
    }

    #[test]
    fn the_solar_of_this_stage_is_accepted() {
        let info = check_version(&version("solar/1", "2.0.0")).unwrap();
        assert_eq!(info.solar_version, "0.3.0");
        assert_eq!(info.build["profile"], json!("release"));
    }

    #[test]
    fn another_protocol_is_refused_with_both_versions() {
        let error = check_version(&version("solar/2", "2.0.0")).unwrap_err();
        let sentence = error.to_string();
        assert!(sentence.contains("SOLAR 0.3.0 speaks solar/2"));
        assert!(sentence.contains(&format!("ZENITH {} speaks solar/1", crate::VERSION)));
    }

    #[test]
    fn another_layout_is_refused_with_both_versions_and_a_newer_minor_is_not() {
        let error = check_version(&version("solar/1", "3.0.0")).unwrap_err();
        assert!(error.to_string().contains("layout 3.0.0"));
        assert!(check_version(&version("solar/1", "2.7.0")).is_ok());
    }

    #[test]
    fn a_version_without_its_members_is_malformed_and_says_which() {
        let error = check_version(&json!({"solar_version": "0.1.0"})).unwrap_err();
        assert_eq!(
            error,
            HandshakeError::Malformed {
                what: "protocol".to_owned()
            }
        );
    }
}
