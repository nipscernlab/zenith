//! Reading a schema as a description, for completion and for parameter forms.
//!
//! Validation asks whether a value fits a schema. Completion and forms ask something else:
//! at this place, which members may go, of which types, with which values allowed, and
//! what does each one mean. The answers are gathered through `$ref`, `allOf`, `anyOf` and
//! `oneOf`, because a schema generated from a Rust type spreads a single object over
//! several of them.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::pointer::{Pointer, Segment};
use crate::schema::{Schema, compact};

/// How many schemas deep the gathering goes, through references and branches, before it
/// stops. Generated schemas are shallow; a schema that refers to itself is not.
const MAX_EXPANSION_DEPTH: usize = 16;

/// A schema document read for what it describes.
#[derive(Debug, Clone, Copy)]
pub struct View<'a> {
    document: &'a Value,
}

/// A member an object may have.
#[derive(Debug, Clone)]
pub struct Property<'a> {
    /// Its name.
    pub name: String,
    /// Whether the object must have it.
    pub required: bool,
    /// Every schema that describes it.
    pub schemas: Vec<&'a Value>,
}

/// A value that may go at a place, for completion.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// The value as JSON text, ready to insert.
    pub text: String,
    /// What the schema says about it, when it says anything.
    pub description: Option<String>,
}

/// What a place in a schema describes, in the terms a person reads.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Description {
    /// The types allowed, in the order the schema names them, `null` included.
    pub types: Vec<String>,
    /// The values allowed, when the schema lists them with `enum` or `const`.
    pub allowed: Vec<Value>,
    /// The first `description` found.
    pub description: Option<String>,
    /// The first `default` found.
    pub default: Option<Value>,
    /// The first `format` found, which is an annotation, shown as a hint.
    pub format: Option<String>,
    /// The bounds, in words, such as `at least 0`.
    pub bounds: Vec<String>,
}

impl Description {
    /// Whether `null` is one of the types allowed.
    #[must_use]
    pub fn nullable(&self) -> bool {
        self.types.iter().any(|name| name == "null") || self.allowed.iter().any(Value::is_null)
    }

    /// The types without `null`.
    #[must_use]
    pub fn non_null_types(&self) -> Vec<&str> {
        self.types
            .iter()
            .map(String::as_str)
            .filter(|name| *name != "null")
            .collect()
    }

    /// The type in words: `string or null`, `one of: "a", "b"`, `integer, at least 0`.
    #[must_use]
    pub fn summary(&self) -> String {
        let mut text = if self.allowed.is_empty() {
            match self.types.as_slice() {
                [] => "any value".to_owned(),
                [one] => one.clone(),
                [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
            }
        } else {
            format!(
                "one of: {}",
                self.allowed
                    .iter()
                    .map(compact)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        if !self.bounds.is_empty() {
            text.push_str(", ");
            text.push_str(&self.bounds.join(", "));
        }
        text
    }
}

/// How a form lets a person fill in one parameter.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldKind {
    /// Text, sent as a JSON string.
    Text,
    /// A whole number.
    Integer,
    /// Any number.
    Number,
    /// `true` or `false`.
    Boolean,
    /// One of a fixed list of values.
    Choice(Vec<Value>),
    /// Anything else, typed as JSON.
    Json,
}

impl<'a> View<'a> {
    /// A view of a whole schema.
    #[must_use]
    pub fn new(schema: &'a Schema) -> Self {
        Self {
            document: schema.document(),
        }
    }

    /// A view of a document that is already whole.
    #[must_use]
    pub fn of_document(document: &'a Value) -> Self {
        Self { document }
    }

    /// Every schema that describes the value at `path`.
    #[must_use]
    pub fn at(&self, path: &Pointer) -> Vec<&'a Value> {
        let mut current = self.expand(self.document);
        for segment in path.segments() {
            let mut next = Vec::new();
            for schema in &current {
                let Value::Object(map) = schema else { continue };
                match segment {
                    Segment::Key(name) => {
                        if let Some(property) = map
                            .get("properties")
                            .and_then(|properties| properties.get(name))
                        {
                            next.push(property);
                        } else if let Some(additional @ Value::Object(_)) =
                            map.get("additionalProperties")
                        {
                            next.push(additional);
                        }
                    }
                    Segment::Index(position) => {
                        let prefix = map.get("prefixItems").and_then(Value::as_array);
                        if let Some(item) = prefix.and_then(|items| items.get(*position)) {
                            next.push(item);
                        } else if let Some(items @ Value::Object(_)) = map.get("items") {
                            next.push(items);
                        }
                    }
                }
            }
            current = next
                .into_iter()
                .flat_map(|schema| self.expand(schema))
                .collect();
        }
        current
    }

    /// The members the object described by `schemas` may have, in the order the schema
    /// declares them.
    #[must_use]
    pub fn properties(&self, schemas: &[&'a Value]) -> Vec<Property<'a>> {
        let mut order: Vec<String> = Vec::new();
        let mut found: BTreeMap<String, Property<'a>> = BTreeMap::new();
        let mut required: Vec<String> = Vec::new();
        for schema in schemas {
            let Value::Object(map) = schema else { continue };
            if let Some(Value::Array(names)) = map.get("required") {
                required.extend(names.iter().filter_map(Value::as_str).map(str::to_owned));
            }
            if let Some(Value::Object(properties)) = map.get("properties") {
                for (name, property_schema) in properties {
                    let entry = found.entry(name.clone()).or_insert_with(|| {
                        order.push(name.clone());
                        Property {
                            name: name.clone(),
                            required: false,
                            schemas: Vec::new(),
                        }
                    });
                    entry.schemas.extend(self.expand(property_schema));
                }
            }
        }
        order
            .into_iter()
            .filter_map(|name| found.remove(&name))
            .map(|mut property| {
                property.required = required.contains(&property.name);
                property
            })
            .collect()
    }

    /// What `schemas` describe, together.
    #[must_use]
    pub fn describe(&self, schemas: &[&'a Value]) -> Description {
        let mut description = Description::default();
        for schema in schemas {
            let Value::Object(map) = schema else { continue };
            match map.get("type") {
                Some(Value::String(name)) => push_unique(&mut description.types, name),
                Some(Value::Array(names)) => {
                    for name in names.iter().filter_map(Value::as_str) {
                        push_unique(&mut description.types, name);
                    }
                }
                _ => {}
            }
            if let Some(value) = map.get("const") {
                push_unique_value(&mut description.allowed, value);
            }
            if let Some(Value::Array(values)) = map.get("enum") {
                for value in values {
                    push_unique_value(&mut description.allowed, value);
                }
            }
            if description.description.is_none() {
                description.description = map
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
            }
            if description.default.is_none() {
                description.default = map.get("default").cloned();
            }
            if description.format.is_none() {
                description.format = map.get("format").and_then(Value::as_str).map(str::to_owned);
            }
            for (keyword, words) in [
                ("minimum", "at least"),
                ("exclusiveMinimum", "more than"),
                ("maximum", "at most"),
                ("exclusiveMaximum", "less than"),
                ("minLength", "at least characters:"),
                ("maxLength", "at most characters:"),
            ] {
                if let Some(limit) = map.get(keyword) {
                    let bound = match words.strip_suffix(" characters:") {
                        Some(prefix) => format!("{prefix} {limit} characters"),
                        None => format!("{words} {limit}"),
                    };
                    if !description.bounds.contains(&bound) {
                        description.bounds.push(bound);
                    }
                }
            }
        }
        // A list of constants says more than the type they share.
        if !description.allowed.is_empty() {
            description.types.retain(|name| name == "null");
        }
        description
    }

    /// The values completion offers at a place: the allowed values, `true` and `false` for
    /// a boolean, and `null` when the type admits it, with the default first.
    #[must_use]
    pub fn value_candidates(&self, schemas: &[&'a Value]) -> Vec<Candidate> {
        let description = self.describe(schemas);
        let mut candidates: Vec<Candidate> = Vec::new();
        let mut push = |value: &Value, text: Option<String>| {
            let json = value.to_string();
            if !candidates.iter().any(|candidate| candidate.text == json) {
                candidates.push(Candidate {
                    text: json,
                    description: text,
                });
            }
        };
        if let Some(default) = &description.default {
            push(default, Some("the default".to_owned()));
        }
        for schema in schemas {
            if let Some(value) = schema.get("const") {
                push(
                    value,
                    schema
                        .get("description")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                );
            }
        }
        for value in &description.allowed {
            push(value, None);
        }
        if description.types.iter().any(|name| name == "boolean") {
            push(&Value::Bool(true), None);
            push(&Value::Bool(false), None);
        }
        if description.nullable() {
            push(&Value::Null, None);
        }
        candidates
    }

    /// How a form should let a person fill in a value described by `schemas`.
    #[must_use]
    pub fn field_kind(&self, schemas: &[&'a Value]) -> FieldKind {
        let description = self.describe(schemas);
        let non_null: Vec<Value> = description
            .allowed
            .iter()
            .filter(|value| !value.is_null())
            .cloned()
            .collect();
        if !non_null.is_empty() {
            return FieldKind::Choice(non_null);
        }
        match description.non_null_types().as_slice() {
            ["string"] => FieldKind::Text,
            ["integer"] => FieldKind::Integer,
            ["number"] | ["integer", "number"] | ["number", "integer"] => FieldKind::Number,
            ["boolean"] => FieldKind::Boolean,
            _ => FieldKind::Json,
        }
    }

    /// The schema and every schema it stands for: the target of its `$ref` and the
    /// branches of its `allOf`, `anyOf` and `oneOf`, recursively.
    fn expand(self, schema: &'a Value) -> Vec<&'a Value> {
        let mut out = Vec::new();
        self.expand_into(schema, &mut out, 0);
        out
    }

    fn expand_into(self, schema: &'a Value, out: &mut Vec<&'a Value>, depth: usize) {
        if depth > MAX_EXPANSION_DEPTH || out.iter().any(|seen| std::ptr::eq(*seen, schema)) {
            return;
        }
        out.push(schema);
        let Value::Object(map) = schema else { return };
        if let Some(target) = map
            .get("$ref")
            .and_then(Value::as_str)
            .and_then(|reference| self.resolve(reference))
        {
            self.expand_into(target, out, depth + 1);
        }
        for keyword in ["allOf", "anyOf", "oneOf"] {
            if let Some(Value::Array(branches)) = map.get(keyword) {
                for branch in branches {
                    self.expand_into(branch, out, depth + 1);
                }
            }
        }
    }

    fn resolve(self, reference: &str) -> Option<&'a Value> {
        let fragment = reference.strip_prefix('#')?;
        if fragment.is_empty() {
            return Some(self.document);
        }
        Pointer::parse(fragment)?.find(self.document)
    }
}

fn push_unique(list: &mut Vec<String>, name: &str) {
    if !list.iter().any(|present| present == name) {
        list.push(name.to_owned());
    }
}

fn push_unique_value(list: &mut Vec<Value>, value: &Value) {
    if !list.contains(value) {
        list.push(value.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema(document: Value) -> Schema {
        Schema::from_document(document)
    }

    #[test]
    fn properties_are_listed_in_declaration_order_with_whether_they_are_required() {
        let whole = schema(json!({
            "type": "object",
            "required": ["api"],
            "properties": {"api": {"type": "string"}, "depth": {"type": "integer"}}
        }));
        let view = View::new(&whole);
        let properties = view.properties(&view.at(&Pointer::root()));
        let names: Vec<(&str, bool)> = properties
            .iter()
            .map(|property| (property.name.as_str(), property.required))
            .collect();
        assert_eq!(names, vec![("api", true), ("depth", false)]);
    }

    #[test]
    fn properties_are_gathered_through_references_and_branches() {
        let whole = schema(json!({
            "$defs": {"Base": {"properties": {"a": {"type": "string"}}, "required": ["a"]}},
            "allOf": [{"$ref": "#/$defs/Base"}],
            "anyOf": [{"properties": {"b": {"type": "boolean"}}}]
        }));
        let view = View::new(&whole);
        let properties = view.properties(&view.at(&Pointer::root()));
        let names: Vec<&str> = properties
            .iter()
            .map(|property| property.name.as_str())
            .collect();
        assert_eq!(names, vec!["a", "b"]);
        assert!(properties[0].required);
    }

    #[test]
    fn a_nested_place_is_found_through_properties_and_items() {
        let whole = schema(json!({
            "properties": {"tools": {"type": "array", "items": {"$ref": "#/$defs/Tool"}}},
            "$defs": {"Tool": {"properties": {"name": {"type": "string", "description": "Its name."}}}}
        }));
        let view = View::new(&whole);
        let place = view.at(&Pointer::root().key("tools").index(0).key("name"));
        let description = view.describe(&place);
        assert_eq!(description.types, vec!["string".to_owned()]);
        assert_eq!(description.description.as_deref(), Some("Its name."));
    }

    #[test]
    fn a_nullable_string_is_described_and_offered_as_text() {
        let whole = schema(json!({"type": ["string", "null"], "default": null}));
        let view = View::new(&whole);
        let place = view.at(&Pointer::root());
        let description = view.describe(&place);
        assert_eq!(description.summary(), "string or null");
        assert!(description.nullable());
        assert_eq!(view.field_kind(&place), FieldKind::Text);
        let texts: Vec<String> = view
            .value_candidates(&place)
            .into_iter()
            .map(|candidate| candidate.text)
            .collect();
        assert_eq!(texts, vec!["null".to_owned()]);
    }

    #[test]
    fn constants_of_one_of_become_a_choice_with_their_descriptions() {
        let whole = schema(json!({
            "oneOf": [
                {"const": "exact", "description": "Deep equality.", "type": "string"},
                {"const": "subset", "description": "Named members only.", "type": "string"}
            ]
        }));
        let view = View::new(&whole);
        let place = view.at(&Pointer::root());
        assert_eq!(
            view.field_kind(&place),
            FieldKind::Choice(vec![json!("exact"), json!("subset")])
        );
        let candidates = view.value_candidates(&place);
        assert_eq!(candidates[0].text, "\"exact\"");
        assert_eq!(candidates[0].description.as_deref(), Some("Deep equality."));
        assert_eq!(
            view.describe(&place).summary(),
            "one of: \"exact\", \"subset\""
        );
    }

    #[test]
    fn booleans_offer_true_and_false_and_bounds_are_described() {
        let whole = schema(json!({
            "properties": {"on": {"type": "boolean"}, "count": {"type": "integer", "minimum": 0}}
        }));
        let view = View::new(&whole);
        let on = view.at(&Pointer::root().key("on"));
        let texts: Vec<String> = view
            .value_candidates(&on)
            .into_iter()
            .map(|candidate| candidate.text)
            .collect();
        assert_eq!(texts, vec!["true".to_owned(), "false".to_owned()]);
        let count = view.at(&Pointer::root().key("count"));
        assert_eq!(view.describe(&count).summary(), "integer, at least 0");
        assert_eq!(view.field_kind(&count), FieldKind::Integer);
    }

    #[test]
    fn a_schema_that_refers_to_itself_is_expanded_once() {
        let whole = schema(json!({"$ref": "#", "type": "object"}));
        let view = View::new(&whole);
        assert_eq!(view.at(&Pointer::root()).len(), 1);
    }

    fn described(document: Value) -> Description {
        let whole = schema(document);
        let view = View::new(&whole);
        view.describe(&view.at(&Pointer::root()))
    }

    fn kind(document: Value) -> FieldKind {
        let whole = schema(document);
        let view = View::new(&whole);
        view.field_kind(&view.at(&Pointer::root()))
    }

    #[test]
    fn a_list_of_values_says_more_than_their_type_but_null_is_kept() {
        let description = described(json!({"type": ["string", "null"], "enum": ["a", null]}));
        assert_eq!(description.types, ["null"]);
        assert_eq!(description.allowed, [json!("a"), json!(null)]);
        let description = described(json!({"type": "string", "enum": ["a"]}));
        assert!(description.types.is_empty());
    }

    #[test]
    fn a_field_is_a_number_or_a_boolean_by_its_types() {
        for (types, expected) in [
            (json!("number"), FieldKind::Number),
            (json!(["integer", "number"]), FieldKind::Number),
            (json!(["number", "integer"]), FieldKind::Number),
            (json!(["number", "null"]), FieldKind::Number),
            (json!("boolean"), FieldKind::Boolean),
            (json!(["boolean", "null"]), FieldKind::Boolean),
            (json!("integer"), FieldKind::Integer),
            (json!("string"), FieldKind::Text),
            (json!(["string", "boolean"]), FieldKind::Json),
        ] {
            assert_eq!(kind(json!({"type": types})), expected, "{types}");
        }
    }

    /// A schema whose description is `levels` references down from the root.
    fn chain_of_references(levels: usize) -> Value {
        let mut definitions = serde_json::Map::new();
        for level in 1..levels {
            definitions.insert(
                format!("d{level}"),
                json!({"$ref": format!("#/$defs/d{}", level + 1)}),
            );
        }
        definitions.insert(format!("d{levels}"), json!({"description": "deep"}));
        json!({"$defs": definitions, "$ref": "#/$defs/d1"})
    }

    /// A schema whose description is `levels` branches of `allOf` down from the root.
    fn nested_branches(levels: usize) -> Value {
        let mut schema = json!({"description": "deep"});
        for _ in 0..levels {
            schema = json!({"allOf": [schema]});
        }
        schema
    }

    #[test]
    fn references_and_branches_are_followed_sixteen_levels_down_and_no_further() {
        for build in [chain_of_references, nested_branches] {
            assert_eq!(
                described(build(MAX_EXPANSION_DEPTH)).description.as_deref(),
                Some("deep")
            );
            assert_eq!(described(build(MAX_EXPANSION_DEPTH + 1)).description, None);
        }
    }
}
