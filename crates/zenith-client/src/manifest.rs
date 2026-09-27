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

/// The name of the API that changes SOLAR's log level while it runs, section 2 of SOLAR's
/// contract.
pub const SET_LOG_LEVEL_API: &str = "solar.set_log_level";

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
    /// `capabilities`, what the protocol accepts, section 8.2 of SOLAR's contract. A
    /// manifest from before layout 2.1.0 does not have it.
    pub declared: Option<Declared>,
    /// Members of the manifest ZENITH does not know, kept to be shown.
    pub other: Vec<(String, Value)>,
}

/// What a manifest declares the protocol accepts, its `capabilities`, read as section 8.2
/// of SOLAR's contract lays them out. A member ZENITH does not know, or one it knows in a
/// shape it does not, is kept in `other` rather than dropped.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Declared {
    /// `batch`.
    pub batch: Option<BatchDeclared>,
    /// `cancellation`.
    pub cancellation: Option<CancellationDeclared>,
    /// `notifications.accepted`.
    pub notifications: Option<bool>,
    /// `limits`, every number by its name, in the manifest's order.
    pub limits: Vec<(String, u64)>,
    /// Members ZENITH does not know, kept to be shown.
    pub other: Vec<(String, Value)>,
}

/// `capabilities.batch`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatchDeclared {
    /// Whether a line holding an array of requests is answered rather than refused.
    pub accepted: bool,
    /// The most elements one batch may hold.
    pub max_elements: Option<u64>,
    /// Whether the responses come back in the order of the requests.
    pub ordered: Option<bool>,
}

/// `capabilities.cancellation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancellationDeclared {
    /// Whether a call in flight can be asked to stop.
    pub accepted: bool,
    /// The API that does the asking, `None` when cancellation is not accepted.
    pub method: Option<String>,
}

impl Declared {
    /// Reads `capabilities`, or `None` when it is not an object.
    #[must_use]
    pub fn from_value(value: &Value) -> Option<Self> {
        let Value::Object(map) = value else {
            return None;
        };
        let mut declared = Self::default();
        for (key, value) in map {
            let read = match key.as_str() {
                "batch" => BatchDeclared::from_value(value)
                    .map(|batch| declared.batch = Some(batch))
                    .is_some(),
                "cancellation" => CancellationDeclared::from_value(value)
                    .map(|cancellation| declared.cancellation = Some(cancellation))
                    .is_some(),
                "notifications" => value
                    .get("accepted")
                    .and_then(Value::as_bool)
                    .map(|accepted| declared.notifications = Some(accepted))
                    .is_some(),
                "limits" => match value {
                    Value::Object(limits) => {
                        for (name, limit) in limits {
                            match limit.as_u64() {
                                Some(number) => declared.limits.push((name.clone(), number)),
                                None => declared
                                    .other
                                    .push((format!("limits.{name}"), limit.clone())),
                            }
                        }
                        true
                    }
                    _ => false,
                },
                _ => false,
            };
            if !read {
                declared.other.push((key.clone(), value.clone()));
            }
        }
        Some(declared)
    }

    /// The limit called `name`, such as `max_request_bytes`.
    #[must_use]
    pub fn limit(&self, name: &str) -> Option<u64> {
        self.limits
            .iter()
            .find(|(limit, _)| limit == name)
            .map(|(_, number)| *number)
    }
}

impl BatchDeclared {
    fn from_value(value: &Value) -> Option<Self> {
        Some(Self {
            accepted: value.get("accepted")?.as_bool()?,
            max_elements: value.get("max_elements").and_then(Value::as_u64),
            ordered: value.get("ordered").and_then(Value::as_bool),
        })
    }
}

impl CancellationDeclared {
    fn from_value(value: &Value) -> Option<Self> {
        Some(Self {
            accepted: value.get("accepted")?.as_bool()?,
            method: string(value.get("method")),
        })
    }
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
    /// `max_output_bytes`, the largest response the API may produce, section 8.3. A SOLAR
    /// from before section 8.3 does not declare it, under the same manifest layout, so its
    /// absence is not a breach of the contract.
    pub max_output_bytes: Option<u64>,
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
        let declared = root.get("capabilities").and_then(Declared::from_value);
        let known = [
            "$defs",
            "apis",
            "protocol",
            "schema_dialect",
            "schema_version",
            "solar_version",
        ];
        // `capabilities` in a shape ZENITH cannot read is kept, as any unknown member is.
        let read =
            |key: &str| known.contains(&key) || (key == "capabilities" && declared.is_some());
        let other = root
            .iter()
            .filter(|(key, _)| !read(key))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        Ok(Self {
            solar_version: string(root.get("solar_version")),
            protocol: string(root.get("protocol")),
            schema_version,
            schema_dialect,
            definitions,
            apis,
            declared,
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

    /// What this SOLAR offers beyond calls: what its manifest declares, and, for a manifest
    /// that declares nothing, what its APIs show.
    #[must_use]
    pub fn capabilities(&self) -> Capabilities {
        let declared = self.declared.as_ref();
        let cancel = match declared.and_then(|declared| declared.cancellation.as_ref()) {
            Some(cancellation) if cancellation.accepted => Some(
                cancellation
                    .method
                    .clone()
                    .unwrap_or_else(|| CANCEL_API.to_owned()),
            ),
            Some(_) => None,
            None => self
                .api(CANCEL_API)
                .filter(|api| takes(api, "id"))
                .map(|api| api.name.clone()),
        };
        let batches = match declared.and_then(|declared| declared.batch) {
            None => Batches::Unknown,
            Some(batch) if !batch.accepted => Batches::Refused,
            Some(batch) => Batches::Accepted {
                max_elements: batch.max_elements,
                ordered: batch.ordered,
            },
        };
        Capabilities {
            cancel,
            batches,
            log_level: self
                .api(SET_LOG_LEVEL_API)
                .is_some_and(|api| takes(api, "level")),
            max_request_bytes: declared.and_then(|declared| declared.limit("max_request_bytes")),
            declared: declared.is_some(),
        }
    }
}

/// Whether an API's parameters have a member called `name`.
fn takes(api: &Api, name: &str) -> bool {
    let view = View::new(api.params());
    view.properties(&view.at(&crate::pointer::Pointer::root()))
        .iter()
        .any(|property| property.name == name)
}

/// What a SOLAR build offers beyond calls. A manifest of layout 2.1.0 or later declares it
/// in `capabilities`; for an older one, ZENITH reads what it can from the APIs and leaves
/// the rest unknown.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Capabilities {
    /// The API that cancels a call in flight: the one the manifest declares, or, when it
    /// declares nothing, `solar.cancel` if it is in the manifest and takes an `id`.
    pub cancel: Option<String>,
    /// Whether a line holding an array of requests is answered.
    pub batches: Batches,
    /// `solar.set_log_level` is in the manifest and takes a `level`, so SOLAR's log level
    /// can change while it runs.
    pub log_level: bool,
    /// The longest request line SOLAR reads, in bytes, without its newline, when the
    /// manifest declares it.
    pub max_request_bytes: Option<u64>,
    /// Whether the manifest declares its capabilities at all.
    pub declared: bool,
}

/// Whether SOLAR answers a batch, section 3.2 of SOLAR's contract.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Batches {
    /// The manifest does not say, as one from before layout 2.1.0 does not.
    #[default]
    Unknown,
    /// A batch is refused.
    Refused,
    /// A batch is answered.
    Accepted {
        /// The most elements one may hold, when the manifest says.
        max_elements: Option<u64>,
        /// Whether the responses come back in the order of the requests, when it says.
        ordered: Option<bool>,
    },
}

/// The members section 8 of SOLAR's contract requires of every API entry.
const REQUIRED: [&str; 12] = [
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

/// The members ZENITH also knows: the name, and one that a SOLAR from before its section of
/// the contract does not have.
const ALSO_KNOWN: [&str; 2] = ["name", "max_output_bytes"];

impl Api {
    fn from_entry(entry: &Value, definitions: &Value, dialect: Option<&str>) -> Option<Self> {
        let Value::Object(map) = entry else {
            return None;
        };
        let name = map.get("name")?.as_str()?.to_owned();
        let missing = REQUIRED
            .into_iter()
            .filter(|required| !map.contains_key(*required))
            .collect();
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
        let other = map
            .iter()
            .filter(|(key, _)| {
                !REQUIRED.contains(&key.as_str()) && !ALSO_KNOWN.contains(&key.as_str())
            })
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
            max_output_bytes: map.get("max_output_bytes").and_then(Value::as_u64),
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
        assert_eq!(ping.max_output_bytes, None);
        assert_eq!(ping.examples.len(), 2);
        assert_eq!(ping.examples[0].matching, Matching::Exact);
        assert!(ping.missing.is_empty());
        assert!(ping.other.is_empty());
        assert!(catalogue.other.is_empty());
    }

    #[test]
    fn the_largest_response_is_read_where_an_api_declares_it() {
        let mut manifest = fixture();
        manifest["apis"][2]["max_output_bytes"] = json!(8_388_608);
        let catalogue = Catalogue::from_data(&manifest).unwrap();
        let ping = catalogue.api("solar.ping").unwrap();
        assert_eq!(ping.max_output_bytes, Some(8_388_608));
        assert!(ping.other.is_empty() && ping.missing.is_empty());
    }

    fn fixture_0_3_0() -> Value {
        serde_json::from_str(include_str!("../tests/fixtures/solar-0.3.0-manifest.json")).unwrap()
    }

    #[test]
    fn solar_0_1_0_offers_no_cancellation_and_declares_nothing() {
        let catalogue = Catalogue::from_data(&fixture()).unwrap();
        assert_eq!(catalogue.declared, None);
        assert_eq!(
            catalogue.capabilities(),
            Capabilities {
                cancel: None,
                batches: Batches::Unknown,
                log_level: false,
                max_request_bytes: None,
                declared: false,
            }
        );
    }

    #[test]
    fn solar_0_3_0_declares_its_capabilities_and_they_are_read() {
        let catalogue = Catalogue::from_data(&fixture_0_3_0()).unwrap();
        assert_eq!(catalogue.schema_version, "2.1.0");
        assert!(catalogue.other.is_empty(), "{:?}", catalogue.other);
        let declared = catalogue.declared.clone().unwrap();
        assert_eq!(
            declared.batch,
            Some(BatchDeclared {
                accepted: true,
                max_elements: Some(64),
                ordered: Some(true),
            })
        );
        assert_eq!(
            declared.cancellation,
            Some(CancellationDeclared {
                accepted: true,
                method: Some("solar.cancel".to_owned()),
            })
        );
        assert_eq!(declared.notifications, Some(false));
        assert_eq!(declared.limit("max_request_bytes"), Some(16_777_216));
        assert_eq!(declared.limit("max_queued_requests"), Some(256));
        assert_eq!(declared.limit("max_thoughts"), None);
        assert_eq!(declared.limits.len(), 6);
        assert!(declared.other.is_empty(), "{:?}", declared.other);
        assert_eq!(
            catalogue.capabilities(),
            Capabilities {
                cancel: Some("solar.cancel".to_owned()),
                batches: Batches::Accepted {
                    max_elements: Some(64),
                    ordered: Some(true),
                },
                log_level: true,
                max_request_bytes: Some(16_777_216),
                declared: true,
            }
        );
    }

    #[test]
    fn what_the_manifest_declares_wins_over_what_its_apis_suggest() {
        let mut manifest = fixture_0_3_0();
        manifest["capabilities"]["cancellation"] = json!({"accepted": false, "method": null});
        manifest["capabilities"]["batch"] = json!({"accepted": false});
        let capabilities = Catalogue::from_data(&manifest).unwrap().capabilities();
        assert_eq!(capabilities.cancel, None);
        assert_eq!(capabilities.batches, Batches::Refused);
        manifest["capabilities"]["cancellation"] =
            json!({"accepted": true, "method": "solar.stop"});
        let capabilities = Catalogue::from_data(&manifest).unwrap().capabilities();
        assert_eq!(capabilities.cancel.as_deref(), Some("solar.stop"));
        manifest["capabilities"]["cancellation"] = json!({"accepted": true, "method": null});
        let capabilities = Catalogue::from_data(&manifest).unwrap().capabilities();
        assert_eq!(capabilities.cancel.as_deref(), Some("solar.cancel"));
    }

    #[test]
    fn capabilities_that_leave_cancellation_out_fall_back_to_the_apis() {
        let mut manifest = fixture_0_3_0();
        manifest["capabilities"]
            .as_object_mut()
            .unwrap()
            .remove("cancellation");
        let capabilities = Catalogue::from_data(&manifest).unwrap().capabilities();
        assert_eq!(capabilities.cancel.as_deref(), Some("solar.cancel"));
        assert!(capabilities.declared);
    }

    #[test]
    fn capabilities_in_a_shape_zenith_does_not_know_are_kept_and_not_guessed() {
        let manifest = json!({
            "schema_version": "2.3.0",
            "apis": [],
            "capabilities": {
                "batch": {"max_elements": 64},
                "cancellation": "yes",
                "notifications": {},
                "limits": {"max_request_bytes": 100, "max_thoughts": "many"},
                "streaming": {"accepted": true}
            }
        });
        let catalogue = Catalogue::from_data(&manifest).unwrap();
        let declared = catalogue.declared.clone().unwrap();
        assert_eq!(declared.batch, None);
        assert_eq!(declared.cancellation, None);
        assert_eq!(declared.notifications, None);
        assert_eq!(declared.limits, vec![("max_request_bytes".to_owned(), 100)]);
        let kept: Vec<&str> = declared.other.iter().map(|(key, _)| key.as_str()).collect();
        assert_eq!(
            kept,
            vec![
                "batch",
                "cancellation",
                "notifications",
                "limits.max_thoughts",
                "streaming"
            ]
        );
        let capabilities = catalogue.capabilities();
        assert_eq!(capabilities.batches, Batches::Unknown);
        assert_eq!(capabilities.cancel, None);
        assert_eq!(capabilities.max_request_bytes, Some(100));
        // A capabilities member that is not an object is kept whole, as any unknown member.
        let catalogue = Catalogue::from_data(&json!({
            "schema_version": "2.1.0", "apis": [], "capabilities": true
        }))
        .unwrap();
        assert_eq!(catalogue.declared, None);
        assert_eq!(
            catalogue.other,
            vec![("capabilities".to_owned(), json!(true))]
        );
        assert!(!catalogue.capabilities().declared);
    }

    #[test]
    fn the_log_level_can_be_set_only_through_an_api_that_takes_a_level() {
        let mut manifest = fixture();
        manifest["apis"].as_array_mut().unwrap().push(json!({
            "name": "solar.set_log_level",
            "params_schema": {"type": "object", "properties": {"verbosity": {}}},
        }));
        assert!(
            !Catalogue::from_data(&manifest)
                .unwrap()
                .capabilities()
                .log_level
        );
        manifest["apis"][5]["params_schema"]["properties"] = json!({"level": {}});
        assert!(
            Catalogue::from_data(&manifest)
                .unwrap()
                .capabilities()
                .log_level
        );
    }

    #[test]
    fn cancellation_is_detected_when_the_manifest_has_solar_cancel_with_an_id() {
        let mut manifest = fixture();
        manifest["apis"].as_array_mut().unwrap().push(json!({
            "name": "solar.cancel",
            "params_schema": {"type": "object", "properties": {"id": {}}, "required": ["id"]},
        }));
        let catalogue = Catalogue::from_data(&manifest).unwrap();
        assert_eq!(
            catalogue.capabilities().cancel.as_deref(),
            Some("solar.cancel")
        );
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
