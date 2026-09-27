//! The manifest, read into the catalogue ZENITH builds everything from.
//!
//! The manifest is the `data` of `solar.manifest`, laid out by section 8 of SOLAR's
//! contract. ZENITH reads it leniently in one direction and strictly in the other: a member
//! it does not know is kept and shown under its own name, so that something SOLAR adds is
//! visible before ZENITH learns it, and a layout whose major version it does not know is
//! refused, as section 10 of the contract requires of every consumer.

use std::fmt;

use serde_json::Value;

use crate::schema::Schema;
use crate::schema_view::View;

/// The major version of the manifest layout this ZENITH reads.
pub const LAYOUT_MAJOR: u64 = 2;

/// The name of the API that cancels a call, section 9 of SOLAR's contract.
pub const CANCEL_API: &str = "solar.cancel";

/// Every API a SOLAR build answers to, as its manifest describes them.
#[derive(Debug, Clone)]
pub struct Catalogue {
    /// `solar_version`, the build that wrote the manifest.
    pub solar_version: Option<String>,
    /// `protocol`.
    pub protocol: Option<String>,
    /// `schema_version`, the layout of the manifest.
    pub schema_version: String,
    /// `schema_dialect`, the JSON Schema dialect of every schema in it.
    pub schema_dialect: Option<String>,
    /// `$defs`, the definitions the schemas share.
    pub definitions: Value,
    /// The APIs, in the order the manifest lists them.
    pub apis: Vec<Api>,
    /// Members of the manifest ZENITH does not know, kept to be shown.
    pub other: Vec<(String, Value)>,
}

/// One API, as its manifest entry describes it.
#[derive(Debug, Clone)]
pub struct Api {
    /// `name`.
    pub name: String,
    /// `version`, the API's own semantic version.
    pub version: Option<String>,
    /// `summary`, one line.
    pub summary: Option<String>,
    /// `description`, prose.
    pub description: Option<String>,
    /// `errors`, what it declares it may return beyond dispatch.
    pub errors: Vec<DeclaredError>,
    /// `side_effects`.
    pub side_effects: Vec<String>,
    /// `idempotent`.
    pub idempotent: Option<bool>,
    /// `stability`.
    pub stability: Option<String>,
    /// `since`, the SOLAR version it first appeared in.
    pub since: Option<String>,
    /// `timeout_ms`, SOLAR's budget for one call.
    pub timeout_ms: Option<u64>,
    /// `params_schema`, as the manifest has it.
    pub params_schema: Value,
    /// `output_schema`, as the manifest has it.
    pub output_schema: Value,
    /// `examples`.
    pub examples: Vec<Example>,
    /// Members of the entry ZENITH does not know, kept to be shown.
    pub other: Vec<(String, Value)>,
    /// Members the contract requires that the entry does not have.
    pub missing: Vec<&'static str>,
    params: Schema,
    output: Schema,
}

/// A `{status, reason}` pair an API declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredError {
    /// The canonical status.
    pub status: String,
    /// The finer reason.
    pub reason: String,
}

/// An example of an API: parameters, and what the response's `data` should be.
#[derive(Debug, Clone)]
pub struct Example {
    /// `name`.
    pub name: String,
    /// `description`.
    pub description: Option<String>,
    /// `params`, which is what a person running the example sends.
    pub params: Value,
    /// `response`, the expected `data`.
    pub response: Value,
    /// `match`, how the two are compared.
    pub matching: Matching,
}

/// How an example's response is compared with what really came back, section 8.2 of
/// SOLAR's contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Matching {
    /// Deep equality.
    Exact,
    /// Every member the example names must match; the rest is ignored.
    Subset,
    /// A rule ZENITH does not know, which it cannot check.
    Unknown(String),
}

/// Why a manifest could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// The `data` of `solar.manifest` was not an object.
    NotAnObject,
    /// There was no `schema_version`.
    NoLayoutVersion,
    /// The `schema_version` has a major version ZENITH does not read.
    Layout {
        /// What the manifest says.
        found: String,
    },
    /// There was no `apis` array.
    NoApis,
    /// An entry of `apis` was not an object with a `name`.
    BadEntry {
        /// Its position.
        index: usize,
    },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAnObject => formatter.write_str("The manifest is not a JSON object."),
            Self::NoLayoutVersion => {
                formatter.write_str("The manifest has no schema_version, so its layout is unknown.")
            }
            Self::Layout { found } => write!(
                formatter,
                "The manifest is in layout {found}, and ZENITH {} reads layout {LAYOUT_MAJOR}.",
                crate::VERSION
            ),
            Self::NoApis => formatter.write_str("The manifest has no apis array."),
            Self::BadEntry { index } => write!(
                formatter,
                "Entry {index} of the manifest's apis is not an object with a name."
            ),
        }
    }
}

impl std::error::Error for ManifestError {}

/// The major version of a semantic version, `2` of `2.0.0`.
#[must_use]
pub fn major(version: &str) -> Option<u64> {
    version.split('.').next()?.trim().parse().ok()
}

impl Catalogue {
    /// Reads the `data` of a `solar.manifest` response.
    ///
    /// # Errors
    ///
    /// [`ManifestError`] when the data is not a manifest of layout 2: not an object, no
    /// `schema_version`, a major version other than 2, no `apis`, or an entry without a
    /// name.
    pub fn from_data(data: &Value) -> Result<Self, ManifestError> {
        let Value::Object(root) = data else {
            return Err(ManifestError::NotAnObject);
        };
        let schema_version = root
            .get("schema_version")
            .and_then(Value::as_str)
            .ok_or(ManifestError::NoLayoutVersion)?
            .to_owned();
        if major(&schema_version) != Some(LAYOUT_MAJOR) {
            return Err(ManifestError::Layout {
                found: schema_version,
            });
        }
        let definitions = root
            .get("$defs")
            .cloned()
            .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
        let schema_dialect = string(root.get("schema_dialect"));
        let entries = root
            .get("apis")
            .and_then(Value::as_array)
            .ok_or(ManifestError::NoApis)?;
        let mut apis = Vec::with_capacity(entries.len());
        for (index, entry) in entries.iter().enumerate() {
            let api = Api::from_entry(entry, &definitions, schema_dialect.as_deref())
                .ok_or(ManifestError::BadEntry { index })?;
            apis.push(api);
        }
        let known = [
            "$defs",
            "apis",
            "protocol",
            "schema_dialect",
            "schema_version",
            "solar_version",
        ];
        let other = root
            .iter()
            .filter(|(key, _)| !known.contains(&key.as_str()))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        Ok(Self {
            solar_version: string(root.get("solar_version")),
            protocol: string(root.get("protocol")),
            schema_version,
            schema_dialect,
            definitions,
            apis,
            other,
        })
    }

    /// The API called `name`.
    #[must_use]
    pub fn api(&self, name: &str) -> Option<&Api> {
        self.apis.iter().find(|api| api.name == name)
    }

    /// The API names, in the manifest's order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.apis.iter().map(|api| api.name.as_str())
    }

    /// What this SOLAR offers beyond calls, as far as the manifest can say.
    #[must_use]
    pub fn capabilities(&self) -> Capabilities {
        let cancel = self.api(CANCEL_API).filter(|api| {
            let view = View::new(api.params());
            view.properties(&view.at(&crate::pointer::Pointer::root()))
                .iter()
                .any(|property| property.name == "id")
        });
        Capabilities {
            cancel: cancel.is_some(),
        }
    }
}

/// What a SOLAR build offers beyond calls, as its manifest declares it.
///
/// Batches are not here: the manifest does not say whether a build accepts them, which
/// `docs/OPEN_QUESTIONS.md` records as something ZENITH needed and did not find.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Capabilities {
    /// `solar.cancel` is in the manifest and takes an `id`, as section 9 of SOLAR's
    /// contract describes it.
    pub cancel: bool,
}

impl Api {
    fn from_entry(entry: &Value, definitions: &Value, dialect: Option<&str>) -> Option<Self> {
        let Value::Object(map) = entry else {
            return None;
        };
        let name = map.get("name")?.as_str()?.to_owned();
        let mut missing = Vec::new();
        for required in [
            "version",
            "summary",
            "description",
            "errors",
            "side_effects",
            "idempotent",
            "stability",
            "since",
            "timeout_ms",
            "params_schema",
            "output_schema",
            "examples",
        ] {
            if !map.contains_key(required) {
                missing.push(required);
            }
        }
        let params_schema = map
            .get("params_schema")
            .cloned()
            .unwrap_or(Value::Bool(true));
        let output_schema = map
            .get("output_schema")
            .cloned()
            .unwrap_or(Value::Bool(true));
        let errors = map
            .get("errors")
            .and_then(Value::as_array)
            .map(|errors| {
                errors
                    .iter()
                    .map(|error| DeclaredError {
                        status: string(error.get("status")).unwrap_or_default(),
                        reason: string(error.get("reason")).unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let side_effects = map
            .get("side_effects")
            .and_then(Value::as_array)
            .map(|effects| {
                effects
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let examples = map
            .get("examples")
            .and_then(Value::as_array)
            .map(|examples| examples.iter().filter_map(Example::from_value).collect())
            .unwrap_or_default();
        let known = [
            "name",
            "version",
            "summary",
            "description",
            "errors",
            "side_effects",
            "idempotent",
            "stability",
            "since",
            "timeout_ms",
            "params_schema",
            "output_schema",
            "examples",
        ];
        let other = map
            .iter()
            .filter(|(key, _)| !known.contains(&key.as_str()))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        Some(Self {
            params: Schema::from_fragment(&params_schema, definitions, dialect),
            output: Schema::from_fragment(&output_schema, definitions, dialect),
            name,
            version: string(map.get("version")),
            summary: string(map.get("summary")),
            description: string(map.get("description")),
            errors,
            side_effects,
            idempotent: map.get("idempotent").and_then(Value::as_bool),
            stability: string(map.get("stability")),
            since: string(map.get("since")),
            timeout_ms: map.get("timeout_ms").and_then(Value::as_u64),
            params_schema,
            output_schema,
            examples,
            other,
            missing,
        })
    }

    /// The parameter schema, made whole with the manifest's definitions.
    #[must_use]
    pub fn params(&self) -> &Schema {
        &self.params
    }

    /// The output schema, made whole with the manifest's definitions.
    #[must_use]
    pub fn output(&self) -> &Schema {
        &self.output
    }
}

impl Example {
    fn from_value(value: &Value) -> Option<Self> {
        let Value::Object(map) = value else {
            return None;
        };
        let matching = match map.get("match").and_then(Value::as_str) {
            Some("exact") => Matching::Exact,
            Some("subset") => Matching::Subset,
            Some(other) => Matching::Unknown(other.to_owned()),
            None => Matching::Unknown(String::new()),
        };
        Some(Self {
            name: string(map.get("name")).unwrap_or_default(),
            description: string(map.get("description")),
            params: map
                .get("params")
                .cloned()
                .unwrap_or_else(|| Value::Object(serde_json::Map::new())),
            response: map.get("response").cloned().unwrap_or(Value::Null),
            matching,
        })
    }
}

fn string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> Value {
        serde_json::from_str(include_str!("../tests/fixtures/solar-0.1.0-manifest.json")).unwrap()
    }

    #[test]
    fn a_real_manifest_becomes_a_catalogue_of_its_apis() {
        let catalogue = Catalogue::from_data(&fixture()).unwrap();
        let names: Vec<&str> = catalogue.names().collect();
        assert_eq!(
            names,
            vec![
                "solar.describe",
                "solar.manifest",
                "solar.ping",
                "solar.version",
                "system.info"
            ]
        );
        let ping = catalogue.api("solar.ping").unwrap();
        assert_eq!(ping.timeout_ms, Some(1000));
        assert_eq!(ping.examples.len(), 2);
        assert_eq!(ping.examples[0].matching, Matching::Exact);
        assert!(ping.missing.is_empty());
        assert!(ping.other.is_empty());
        assert!(catalogue.other.is_empty());
    }

    #[test]
    fn the_solar_of_this_stage_offers_no_cancellation() {
        let catalogue = Catalogue::from_data(&fixture()).unwrap();
        assert!(!catalogue.capabilities().cancel);
    }

    #[test]
    fn cancellation_is_detected_when_the_manifest_has_solar_cancel_with_an_id() {
        let mut manifest = fixture();
        manifest["apis"].as_array_mut().unwrap().push(json!({
            "name": "solar.cancel",
            "params_schema": {"type": "object", "properties": {"id": {}}, "required": ["id"]},
        }));
        let catalogue = Catalogue::from_data(&manifest).unwrap();
        assert!(catalogue.capabilities().cancel);
        let cancel = catalogue.api("solar.cancel").unwrap();
        assert!(cancel.missing.contains(&"timeout_ms"));
    }

    #[test]
    fn a_layout_whose_major_differs_is_refused_with_both_versions() {
        let error =
            Catalogue::from_data(&json!({"schema_version": "3.0.0", "apis": []})).unwrap_err();
        assert_eq!(
            error,
            ManifestError::Layout {
                found: "3.0.0".to_owned()
            }
        );
        let sentence = error.to_string();
        assert!(sentence.contains("3.0.0") && sentence.contains("layout 2"));
    }

    #[test]
    fn a_newer_minor_is_read_and_its_new_members_are_kept() {
        let catalogue = Catalogue::from_data(&json!({
            "schema_version": "2.4.0",
            "batch": {"max_elements": 64},
            "apis": [{"name": "a.b", "cost": "cheap"}]
        }))
        .unwrap();
        assert_eq!(
            catalogue.other,
            vec![("batch".to_owned(), json!({"max_elements": 64}))]
        );
        assert_eq!(
            catalogue.apis[0].other,
            vec![("cost".to_owned(), json!("cheap"))]
        );
    }

    #[test]
    fn what_is_not_a_manifest_is_refused_with_the_reason() {
        assert_eq!(
            Catalogue::from_data(&json!([])).unwrap_err(),
            ManifestError::NotAnObject
        );
        assert_eq!(
            Catalogue::from_data(&json!({"apis": []})).unwrap_err(),
            ManifestError::NoLayoutVersion
        );
        assert_eq!(
            Catalogue::from_data(&json!({"schema_version": "2.0.0"})).unwrap_err(),
            ManifestError::NoApis
        );
        assert_eq!(
            Catalogue::from_data(&json!({"schema_version": "2.0.0", "apis": [{"x": 1}]}))
                .unwrap_err(),
            ManifestError::BadEntry { index: 0 }
        );
    }
}
