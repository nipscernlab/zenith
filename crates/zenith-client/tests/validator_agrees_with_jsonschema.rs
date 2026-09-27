//! ZENITH's validator gives the verdict of the `jsonschema` crate.
//!
//! `jsonschema` passes the official JSON Schema test suite, and it is a development
//! dependency here, so it costs the shipped binary nothing. These properties generate
//! schemas from every keyword ZENITH's validator checks, and values to try against them,
//! and require the two validators to agree on every one. Then they do the same for every
//! schema of a real SOLAR manifest.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test reports failure by panicking"
)]

use proptest::prelude::*;
use serde_json::{Map, Value, json};
use zenith_client::schema::Schema;

fn reference_verdict(schema: &Value, instance: &Value) -> Option<bool> {
    let validator = jsonschema::draft202012::options()
        .should_validate_formats(false)
        .build(&with_object_ifs(schema))
        .ok()?;
    Some(validator.is_valid(instance))
}

/// The schema with every boolean `if` written as the object schema it stands for, `true`
/// as `{}` and `false` as `{"not": {}}`, which the specification makes the same schema.
///
/// `jsonschema` 0.58.1 drops the annotations of `then` and `else` when `if` is a boolean,
/// so that `unevaluatedItems` and `unevaluatedProperties` see as unevaluated what they
/// evaluated. On 27 September 2026 it judged `{"if": false, "else": {"prefixItems":
/// [true]}, "unevaluatedItems": false}` to refuse `[[1]]`, and the same schema with
/// `{"not": {}}` for `if` to accept it; ZENITH's validator and the Python `jsonschema`
/// 4.26.0 accept it both ways. The names this file generates are never `if`, so every
/// `if` it finds is the keyword.
fn with_object_ifs(schema: &Value) -> Value {
    match schema {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    let value = match (key.as_str(), value) {
                        ("if", Value::Bool(true)) => json!({}),
                        ("if", Value::Bool(false)) => json!({"not": {}}),
                        _ => with_object_ifs(value),
                    };
                    (key.clone(), value)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(with_object_ifs).collect()),
        other => other.clone(),
    }
}

fn zenith_verdict(schema: &Value, instance: &Value) -> bool {
    Schema::from_document(schema.clone())
        .validate(instance)
        .is_valid()
}

const NAMES: [&str; 4] = ["a", "b", "c", "d"];

fn name() -> impl Strategy<Value = String> {
    prop::sample::select(NAMES.to_vec()).prop_map(str::to_owned)
}

fn number() -> impl Strategy<Value = Value> {
    prop_oneof![
        (-3_i64..6).prop_map(|integer| json!(integer)),
        prop::sample::select(vec![0.5, 1.5, -0.5, 2.0, 0.3, 0.1, 1.0])
            .prop_map(|float| json!(float)),
    ]
}

fn instance() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        number(),
        prop::sample::select(vec!["", "a", "ab", "abc", "é", "ça"])
            .prop_map(|text| Value::String(text.to_owned())),
    ];
    leaf.prop_recursive(3, 24, 4, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(Value::Array),
            prop::collection::btree_map(name(), inner, 0..4)
                .prop_map(|members| Value::Object(members.into_iter().collect())),
        ]
    })
}

fn type_name() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec![
        "null", "boolean", "object", "array", "number", "string", "integer",
    ])
}

fn leaf_schema(with_references: bool) -> impl Strategy<Value = Value> {
    let reference = if with_references {
        Just(json!({"$ref": "#/$defs/x"})).boxed()
    } else {
        Just(json!({})).boxed()
    };
    prop_oneof![
        reference,
        Just(json!(true)),
        Just(json!(false)),
        Just(json!({})),
        type_name().prop_map(|name| json!({"type": name})),
        // The meta-schema requires the names in a list of types to be different.
        (type_name(), type_name())
            .prop_filter("two different types", |(a, b)| a != b)
            .prop_map(|(a, b)| json!({"type": [a, b]})),
        instance().prop_map(|value| json!({"const": value})),
        prop::collection::vec(instance(), 1..4).prop_map(|values| json!({"enum": values})),
        number().prop_map(|limit| json!({"minimum": limit})),
        number().prop_map(|limit| json!({"maximum": limit})),
        number().prop_map(|limit| json!({"exclusiveMinimum": limit})),
        number().prop_map(|limit| json!({"exclusiveMaximum": limit})),
        prop::sample::select(vec![json!(0.5), json!(2), json!(3), json!(0.1), json!(1.5)])
            .prop_map(|divisor| json!({"multipleOf": divisor})),
        (0_u64..4).prop_map(|limit| json!({"minLength": limit})),
        (0_u64..4).prop_map(|limit| json!({"maxLength": limit})),
        (0_u64..4).prop_map(|limit| json!({"minItems": limit})),
        (0_u64..4).prop_map(|limit| json!({"maxItems": limit})),
        any::<bool>().prop_map(|unique| json!({"uniqueItems": unique})),
        prop::collection::btree_set(name(), 0..3).prop_map(|names| json!({"required": names})),
        (0_u64..4).prop_map(|limit| json!({"minProperties": limit})),
        (0_u64..4).prop_map(|limit| json!({"maxProperties": limit})),
        (name(), prop::collection::btree_set(name(), 1..3))
            .prop_map(|(key, needed)| json!({"dependentRequired": {key: needed}})),
    ]
}

fn schema_of(with_references: bool) -> impl Strategy<Value = Value> {
    leaf_schema(with_references).prop_recursive(3, 32, 4, |inner| {
        prop_oneof![
            prop::collection::btree_map(name(), inner.clone(), 1..3)
                .prop_map(|properties| json!({"properties": properties})),
            (
                prop::collection::btree_map(name(), inner.clone(), 0..3),
                inner.clone()
            )
                .prop_map(|(properties, additional)| json!({
                    "properties": properties,
                    "additionalProperties": additional
                })),
            inner.clone().prop_map(|items| json!({"items": items})),
            (prop::collection::vec(inner.clone(), 1..3), inner.clone())
                .prop_map(|(prefix, rest)| json!({"prefixItems": prefix, "items": rest})),
            (inner.clone(), 0_u64..3, prop::option::of(0_u64..3)).prop_map(
                |(contains, minimum, maximum)| {
                    let mut schema = json!({"contains": contains, "minContains": minimum});
                    if let Some(maximum) = maximum {
                        schema["maxContains"] = json!(maximum);
                    }
                    schema
                }
            ),
            prop::collection::vec(inner.clone(), 1..3).prop_map(|all| json!({"allOf": all})),
            prop::collection::vec(inner.clone(), 1..3).prop_map(|any| json!({"anyOf": any})),
            prop::collection::vec(inner.clone(), 1..3).prop_map(|one| json!({"oneOf": one})),
            inner.clone().prop_map(|negated| json!({"not": negated})),
            (inner.clone(), inner.clone(), inner.clone()).prop_map(
                |(condition, then, otherwise)| json!({
                    "if": condition, "then": then, "else": otherwise
                })
            ),
            (name(), inner.clone())
                .prop_map(|(key, dependent)| json!({"dependentSchemas": {key: dependent}})),
            inner
                .clone()
                .prop_map(|names| json!({"propertyNames": names})),
            (inner.clone(), inner.clone()).prop_map(|(applied, unevaluated)| json!({
                "allOf": [applied],
                "unevaluatedProperties": unevaluated
            })),
            (inner.clone(), inner.clone()).prop_map(|(applied, unevaluated)| json!({
                "anyOf": [applied],
                "unevaluatedItems": unevaluated
            })),
            (inner.clone(), inner).prop_map(|(mut left, right)| {
                merge(&mut left, &right);
                left
            }),
        ]
    })
}

/// A whole schema: a definition at the root, which any subschema may refer to as
/// `#/$defs/x`, and a body built from every keyword. The definition itself refers to
/// nothing, so a reference can never loop without consuming the instance.
fn schema() -> impl Strategy<Value = Value> {
    (schema_of(false), schema_of(true)).prop_map(|(definition, body)| {
        let mut whole = json!({"$defs": {"x": definition}});
        match body {
            Value::Object(_) => merge(&mut whole, &body),
            other => whole["allOf"] = json!([other]),
        }
        whole
    })
}

/// The keywords of `right` added to `left`, when both are objects, keeping `left`'s when
/// both have the same keyword.
fn merge(left: &mut Value, right: &Value) {
    if let (Value::Object(target), Value::Object(source)) = (left, right) {
        for (keyword, value) in source {
            target
                .entry(keyword.clone())
                .or_insert_with(|| value.clone());
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 4096,
        ..ProptestConfig::default()
    })]

    #[test]
    fn a_generated_schema_gets_the_same_verdict_from_both_validators(
        schema in schema(),
        instance in instance(),
    ) {
        let Some(expected) = reference_verdict(&schema, &instance) else {
            return Ok(());
        };
        prop_assert_eq!(
            zenith_verdict(&schema, &instance),
            expected,
            "schema {} with instance {}",
            schema,
            instance
        );
    }
}

fn manifest() -> Value {
    let text = include_str!("fixtures/solar-0.1.0-manifest.json");
    serde_json::from_str(text).unwrap()
}

/// Every schema in the manifest, made whole the way section 8.1 of SOLAR's contract
/// says, with its name.
fn manifest_schemas() -> Vec<(String, Value)> {
    let manifest = manifest();
    let definitions = manifest["$defs"].clone();
    let dialect = manifest["schema_dialect"].as_str().map(str::to_owned);
    let mut schemas = Vec::new();
    for api in manifest["apis"].as_array().unwrap() {
        let name = api["name"].as_str().unwrap();
        for which in ["params_schema", "output_schema"] {
            let whole = Schema::from_fragment(&api[which], &definitions, dialect.as_deref());
            schemas.push((format!("{name} {which}"), whole.document().clone()));
        }
    }
    for (name, definition) in definitions.as_object().unwrap() {
        let whole = Schema::from_fragment(definition, &definitions, dialect.as_deref());
        schemas.push((format!("$defs/{name}"), whole.document().clone()));
    }
    schemas
}

fn object_like_the_manifest() -> impl Strategy<Value = Value> {
    let keys = prop::sample::select(vec![
        "api", "message", "echo", "pong", "name", "status", "reason", "version", "extra",
    ]);
    prop::collection::btree_map(keys.prop_map(str::to_owned), instance(), 0..4).prop_map(
        |members| {
            let map: Map<String, Value> = members.into_iter().collect();
            Value::Object(map)
        },
    )
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 512,
        ..ProptestConfig::default()
    })]

    #[test]
    fn every_schema_of_a_real_manifest_gets_the_same_verdict_from_both_validators(
        instance in prop_oneof![instance(), object_like_the_manifest()],
    ) {
        for (name, schema) in manifest_schemas() {
            let expected = reference_verdict(&schema, &instance)
                .unwrap_or_else(|| panic!("jsonschema refused the schema {name}"));
            prop_assert_eq!(
                zenith_verdict(&schema, &instance),
                expected,
                "{} with instance {}",
                name,
                instance
            );
        }
    }
}

#[test]
fn every_example_of_a_real_manifest_is_valid_for_its_api() {
    let manifest = manifest();
    let definitions = manifest["$defs"].clone();
    for api in manifest["apis"].as_array().unwrap() {
        let params = Schema::from_fragment(&api["params_schema"], &definitions, None);
        for example in api["examples"].as_array().unwrap() {
            let report = params.validate(&example["params"]);
            assert!(
                report.is_valid(),
                "{} example {}: {:?}",
                api["name"],
                example["name"],
                report.failures
            );
        }
    }
}

#[test]
fn nothing_in_a_real_manifest_goes_unchecked() {
    let manifest = manifest();
    for (name, schema) in manifest_schemas() {
        let report = Schema::from_document(schema).validate(&json!({}));
        assert!(
            report.unchecked.is_empty(),
            "{name} uses {:?}",
            report.unchecked
        );
    }
    drop(manifest);
}

#[test]
fn the_generated_schemas_are_ones_the_reference_accepts_and_both_verdicts_occur() {
    use proptest::strategy::ValueTree;
    use proptest::test_runner::TestRunner;
    let mut runner = TestRunner::deterministic();
    let (mut built, mut valid, mut invalid) = (0, 0, 0);
    for _ in 0..2000 {
        let schema = schema().new_tree(&mut runner).unwrap().current();
        let instance = instance().new_tree(&mut runner).unwrap().current();
        match reference_verdict(&schema, &instance) {
            Some(true) => {
                built += 1;
                valid += 1;
            }
            Some(false) => {
                built += 1;
                invalid += 1;
            }
            None => {}
        }
    }
    // A generator whose schemas the reference refuses, or whose cases all pass or all
    // fail, would make the agreement above say nothing.
    assert!(
        built >= 1900,
        "only {built} of 2000 generated schemas were accepted"
    );
    assert!(
        valid >= 300 && invalid >= 300,
        "{valid} valid and {invalid} invalid"
    );
}
