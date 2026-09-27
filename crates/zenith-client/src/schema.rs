//! Validation of parameters against a JSON Schema 2020-12 before they are sent.
//!
//! This is ZENITH's own validator, not the `jsonschema` crate, which took the release
//! binary from 775 168 bytes to 4 598 272 when it was tried: most of that is regular
//! expressions and format checkers that SOLAR's schemas do not use. The tests hold this
//! validator to the verdicts of `jsonschema`, which is a development dependency only.
//!
//! Every assertion of the 2020-12 vocabularies is checked, including `unevaluatedProperties`
//! and `unevaluatedItems`, with `$ref` resolved inside the schema document. Three things
//! are not: `pattern` and `patternProperties`, which need regular expressions, and any
//! `$ref` that leaves the document. When a schema uses one of them, the report says so in
//! [`Report::unchecked`], and the part of the instance they govern is treated as valid.
//! ZENITH tests SOLAR, and it must never refuse what SOLAR would accept, so what it cannot
//! check it lets through and says it did.
//!
//! `format` is an annotation in 2020-12 unless a validator opts in, and it is not checked
//! here, which is also what SOLAR's own contract tests do.

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashSet};
use std::fmt;

use serde_json::{Map, Number, Value};

use crate::pointer::Pointer;

/// How many `$ref` a validation follows in a chain before it stops, so that a schema that
/// refers to itself without consuming the instance cannot loop.
const MAX_REFERENCE_DEPTH: usize = 64;

/// A schema made whole: the fragment of a manifest with the manifest's `$defs` put back
/// around it, which is what section 8.1 of SOLAR's contract says a consumer does when it
/// needs a schema on its own.
#[derive(Debug, Clone)]
pub struct Schema {
    document: Value,
}

impl Schema {
    /// Wraps a fragment with the dialect and the shared definitions.
    #[must_use]
    pub fn from_fragment(fragment: &Value, definitions: &Value, dialect: Option<&str>) -> Self {
        let document = match fragment {
            Value::Object(map) => {
                let mut whole = map.clone();
                if let Some(dialect) = dialect {
                    whole
                        .entry("$schema")
                        .or_insert_with(|| Value::String(dialect.to_owned()));
                }
                if !whole.contains_key("$defs") {
                    whole.insert("$defs".to_owned(), definitions.clone());
                }
                Value::Object(whole)
            }
            other => other.clone(),
        };
        Self { document }
    }

    /// A schema that is already whole.
    #[must_use]
    pub fn from_document(document: Value) -> Self {
        Self { document }
    }

    /// The whole document.
    #[must_use]
    pub fn document(&self) -> &Value {
        &self.document
    }

    /// Validates `instance` and reports every failure, and every keyword that was not
    /// checked.
    #[must_use]
    pub fn validate(&self, instance: &Value) -> Report {
        let mut run = Run {
            document: &self.document,
            unchecked: BTreeSet::new(),
        };
        let mut failures = Vec::new();
        run.validate(
            &self.document,
            instance,
            &Pointer::root(),
            "",
            0,
            &mut failures,
        );
        Report {
            failures,
            unchecked: run.unchecked,
        }
    }
}

/// What a validation found.
#[derive(Debug, Clone, Default)]
pub struct Report {
    /// Every failure, in the order the schema was read.
    pub failures: Vec<Failure>,
    /// The keywords the schema uses that this validator does not check.
    pub unchecked: BTreeSet<String>,
}

impl Report {
    /// Whether the instance passed everything that was checked.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.failures.is_empty()
    }
}

/// One failure: where in the instance, which keyword of the schema, and what was wrong.
#[derive(Debug, Clone, PartialEq)]
pub struct Failure {
    /// The place in the instance, as a pointer.
    pub at: Pointer,
    /// Where in the schema the keyword that failed is, as a pointer into the document.
    pub keyword_path: String,
    /// What was wrong.
    pub kind: FailureKind,
}

/// What was wrong, in terms a message can be written from.
#[derive(Debug, Clone, PartialEq)]
pub enum FailureKind {
    /// `type`.
    Type {
        /// The types the schema allows.
        expected: Vec<String>,
        /// The type that arrived.
        received: &'static str,
    },
    /// `const`.
    Const {
        /// The only value allowed.
        expected: Value,
    },
    /// `enum`.
    Enum {
        /// The values allowed.
        allowed: Vec<Value>,
    },
    /// `multipleOf`.
    NotMultipleOf {
        /// What the value must be a multiple of.
        divisor: Number,
    },
    /// `minimum` or `exclusiveMinimum`.
    TooSmall {
        /// The limit.
        limit: Number,
        /// Whether the limit itself is refused.
        exclusive: bool,
    },
    /// `maximum` or `exclusiveMaximum`.
    TooLarge {
        /// The limit.
        limit: Number,
        /// Whether the limit itself is refused.
        exclusive: bool,
    },
    /// `minLength`, in characters.
    TooShort {
        /// The fewest characters allowed.
        limit: u64,
    },
    /// `maxLength`, in characters.
    TooLong {
        /// The most characters allowed.
        limit: u64,
    },
    /// `minItems`.
    TooFewItems {
        /// The fewest elements allowed.
        limit: u64,
    },
    /// `maxItems`.
    TooManyItems {
        /// The most elements allowed.
        limit: u64,
    },
    /// `uniqueItems`.
    NotUnique {
        /// The first of two equal elements.
        first: usize,
        /// The second.
        second: usize,
    },
    /// `contains`, with `minContains`.
    ContainsTooFew {
        /// How many elements matched.
        found: usize,
        /// How many had to.
        limit: u64,
    },
    /// `maxContains`.
    ContainsTooMany {
        /// How many elements matched.
        found: usize,
        /// How many may.
        limit: u64,
    },
    /// `required`.
    Missing {
        /// The member that is not there.
        property: String,
    },
    /// `additionalProperties` refused a member.
    NotAllowed {
        /// The member.
        property: String,
        /// The members the schema declares, when it declares any.
        declared: Vec<String>,
        /// The declared member closest to it, when one is close.
        suggestion: Option<String>,
    },
    /// `unevaluatedProperties` refused a member.
    NotEvaluated {
        /// The member.
        property: String,
    },
    /// `items: false` or `unevaluatedItems` refused an element.
    ItemNotAllowed {
        /// Its position.
        index: usize,
    },
    /// `minProperties`.
    TooFewProperties {
        /// The fewest members allowed.
        limit: u64,
    },
    /// `maxProperties`.
    TooManyProperties {
        /// The most members allowed.
        limit: u64,
    },
    /// `dependentRequired`.
    DependentMissing {
        /// The member that is present.
        property: String,
        /// The member it requires, which is not.
        requires: String,
    },
    /// `propertyNames` refused a name.
    BadPropertyName {
        /// The name.
        name: String,
    },
    /// `anyOf` or `oneOf` matched no branch.
    NoBranch {
        /// How many branches there are.
        branches: usize,
        /// The failures of the branch that came closest.
        closest: Vec<Failure>,
    },
    /// `oneOf` matched more than one branch.
    SeveralBranches {
        /// The positions of the branches that matched.
        matched: Vec<usize>,
    },
    /// `not`.
    Not,
    /// The schema `false`, which nothing satisfies.
    Nothing,
}

impl Failure {
    /// What the schema expected at this place, in words.
    #[must_use]
    pub fn expected(&self) -> String {
        match &self.kind {
            FailureKind::Type { expected, .. } => join_types(expected),
            FailureKind::Const { expected } => compact(expected),
            FailureKind::Enum { allowed } => format!(
                "one of: {}",
                allowed.iter().map(compact).collect::<Vec<_>>().join(", ")
            ),
            FailureKind::NotMultipleOf { divisor } => format!("a multiple of {divisor}"),
            FailureKind::TooSmall { limit, exclusive } => {
                format!(
                    "{} {limit}",
                    if *exclusive { "more than" } else { "at least" }
                )
            }
            FailureKind::TooLarge { limit, exclusive } => {
                format!(
                    "{} {limit}",
                    if *exclusive { "less than" } else { "at most" }
                )
            }
            FailureKind::TooShort { limit } => format!("at least {}", count(*limit, "character")),
            FailureKind::TooLong { limit } => format!("at most {}", count(*limit, "character")),
            FailureKind::TooFewItems { limit } => format!("at least {}", count(*limit, "element")),
            FailureKind::TooManyItems { limit } => format!("at most {}", count(*limit, "element")),
            FailureKind::NotUnique { .. } => "elements that are all different".to_owned(),
            FailureKind::ContainsTooFew { limit, .. } => {
                format!("at least {} matching the schema", count(*limit, "element"))
            }
            FailureKind::ContainsTooMany { limit, .. } => {
                format!("at most {} matching the schema", count(*limit, "element"))
            }
            FailureKind::Missing { property } => format!("a member {property}"),
            FailureKind::NotAllowed { declared, .. } => {
                if declared.is_empty() {
                    "no members".to_owned()
                } else {
                    format!("one of: {}", declared.join(", "))
                }
            }
            FailureKind::NotEvaluated { .. } => "only the members the schema declares".to_owned(),
            FailureKind::ItemNotAllowed { .. } => "no element at this position".to_owned(),
            FailureKind::TooFewProperties { limit } => {
                format!("at least {}", count(*limit, "member"))
            }
            FailureKind::TooManyProperties { limit } => {
                format!("at most {}", count(*limit, "member"))
            }
            FailureKind::DependentMissing { requires, .. } => format!("a member {requires}"),
            FailureKind::BadPropertyName { .. } => "a name the schema allows".to_owned(),
            FailureKind::NoBranch { branches, .. } => {
                format!("a value matching one of {branches} alternatives")
            }
            FailureKind::SeveralBranches { .. } => {
                "a value matching exactly one alternative".to_owned()
            }
            FailureKind::Not => "a value the schema does not describe".to_owned(),
            FailureKind::Nothing => "nothing: no value is allowed here".to_owned(),
        }
    }

    /// One sentence saying what was wrong.
    #[must_use]
    pub fn sentence(&self) -> String {
        let place = if self.at.is_root() {
            "The parameters".to_owned()
        } else {
            self.at.to_string()
        };
        match &self.kind {
            FailureKind::Type { received, .. } => format!(
                "{place} should be {}, and {received} arrived.",
                self.expected()
            ),
            FailureKind::Missing { property } => {
                format!("{place} must have {property}, which is required.")
            }
            FailureKind::NotAllowed {
                property,
                suggestion,
                ..
            } => match suggestion {
                Some(close) => format!("There is no parameter {property}. Did you mean {close}?"),
                None => format!("There is no parameter {property}."),
            },
            FailureKind::NotEvaluated { property } => {
                format!("There is no parameter {property}.")
            }
            FailureKind::DependentMissing { property, requires } => {
                format!("{place} has {property}, which needs {requires} as well.")
            }
            FailureKind::NotUnique { first, second } => {
                format!("{place} has the same value at {first} and at {second}.")
            }
            FailureKind::ItemNotAllowed { index } => {
                format!("{place} has an element at {index}, where none is allowed.")
            }
            FailureKind::SeveralBranches { matched } => format!(
                "{place} matches {} alternatives, and must match exactly one.",
                matched.len()
            ),
            FailureKind::BadPropertyName { name } => {
                format!("{place} has a member named {name}, which the schema does not allow.")
            }
            _ => format!("{place} should be {}.", self.expected()),
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.sentence())
    }
}

fn count(amount: u64, noun: &str) -> String {
    if amount == 1 {
        format!("1 {noun}")
    } else {
        format!("{amount} {noun}s")
    }
}

fn join_types(types: &[String]) -> String {
    let words: Vec<String> = types.iter().map(|name| article(name)).collect();
    match words.as_slice() {
        [] => "nothing".to_owned(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
    }
}

/// The name of a JSON type with its article, as a sentence needs it.
#[must_use]
pub fn article(type_name: &str) -> String {
    match type_name {
        "null" => "null".to_owned(),
        "object" | "array" | "integer" => format!("an {type_name}"),
        other => format!("a {other}"),
    }
}

/// The type of a value, with its article, as the `received` side of a message.
#[must_use]
pub fn received_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(number) if is_integer(number) => "an integer",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// A value on one line, cut at 60 characters, for a message.
#[must_use]
pub fn compact(value: &Value) -> String {
    let text = value.to_string();
    if text.chars().count() > 60 {
        let cut: String = text.chars().take(57).collect();
        format!("{cut}...")
    } else {
        text
    }
}

/// What a schema evaluated of an instance, which `unevaluatedProperties` and
/// `unevaluatedItems` need to know.
#[derive(Debug, Default)]
struct Evaluated {
    properties: HashSet<String>,
    items: HashSet<usize>,
    all_items: bool,
    all_properties: bool,
}

impl Evaluated {
    fn merge(&mut self, other: Evaluated) {
        self.properties.extend(other.properties);
        self.items.extend(other.items);
        self.all_items |= other.all_items;
        self.all_properties |= other.all_properties;
    }
}

struct Run<'a> {
    document: &'a Value,
    unchecked: BTreeSet<String>,
}

impl<'a> Run<'a> {
    #[allow(
        clippy::too_many_lines,
        reason = "one function per vocabulary would scatter the evaluation order, which the \
                  unevaluated keywords depend on, across several functions"
    )]
    fn validate(
        &mut self,
        schema: &'a Value,
        instance: &Value,
        at: &Pointer,
        path: &str,
        depth: usize,
        out: &mut Vec<Failure>,
    ) -> Evaluated {
        let mut evaluated = Evaluated::default();
        let map = match schema {
            Value::Bool(false) => {
                out.push(failure(at, path, FailureKind::Nothing));
                return evaluated;
            }
            Value::Object(map) => map,
            // `true`, which allows everything, or not a schema at all. SOLAR's contract
            // tests compile every schema, so the second would be SOLAR's bug, and ZENITH
            // lets the value through rather than guess.
            _ => return evaluated,
        };

        // The reference first, so that what it evaluates counts for the keywords below.
        if let Some(Value::String(reference)) = map.get("$ref") {
            if let Some(target) = self.resolve(reference) {
                if depth >= MAX_REFERENCE_DEPTH {
                    self.unchecked
                        .insert(format!("$ref nested deeper than {MAX_REFERENCE_DEPTH}"));
                } else {
                    let inner = self.validate(target, instance, at, reference, depth + 1, out);
                    evaluated.merge(inner);
                }
            } else {
                self.unchecked.insert(format!("$ref to {reference}"));
            }
        }
        for keyword in ["$dynamicRef", "$recursiveRef"] {
            if map.contains_key(keyword) {
                self.unchecked.insert(keyword.to_owned());
            }
        }

        Self::check_type(map, instance, at, path, out);
        if let Some(expected) = map.get("const")
            && !json_equal(expected, instance)
        {
            out.push(failure(
                at,
                &join(path, "const"),
                FailureKind::Const {
                    expected: expected.clone(),
                },
            ));
        }
        if let Some(Value::Array(allowed)) = map.get("enum")
            && !allowed
                .iter()
                .any(|candidate| json_equal(candidate, instance))
        {
            out.push(failure(
                at,
                &join(path, "enum"),
                FailureKind::Enum {
                    allowed: allowed.clone(),
                },
            ));
        }

        match instance {
            Value::Number(number) => check_number(map, number, at, path, out),
            Value::String(text) => self.check_string(map, text, at, path, out),
            Value::Array(items) => {
                let inner = self.check_array(map, items, at, path, depth, out);
                evaluated.merge(inner);
            }
            Value::Object(members) => {
                let inner = self.check_object(map, members, at, path, depth, out);
                evaluated.merge(inner);
            }
            Value::Null | Value::Bool(_) => {}
        }

        // The applicators that apply several schemas to the same place.
        if let Some(Value::Array(branches)) = map.get("allOf") {
            for (index, branch) in branches.iter().enumerate() {
                let inner = self.validate(
                    branch,
                    instance,
                    at,
                    &format!("{}/{index}", join(path, "allOf")),
                    depth,
                    out,
                );
                evaluated.merge(inner);
            }
        }
        if let Some(Value::Array(branches)) = map.get("anyOf") {
            let outcome = self.branches(branches, instance, at, &join(path, "anyOf"), depth);
            if outcome.matched.is_empty() {
                out.push(failure(
                    at,
                    &join(path, "anyOf"),
                    FailureKind::NoBranch {
                        branches: branches.len(),
                        closest: outcome.closest,
                    },
                ));
            }
            for inner in outcome.evaluated {
                evaluated.merge(inner);
            }
        }
        if let Some(Value::Array(branches)) = map.get("oneOf") {
            let outcome = self.branches(branches, instance, at, &join(path, "oneOf"), depth);
            match outcome.matched.len() {
                0 => out.push(failure(
                    at,
                    &join(path, "oneOf"),
                    FailureKind::NoBranch {
                        branches: branches.len(),
                        closest: outcome.closest,
                    },
                )),
                1 => {
                    for inner in outcome.evaluated {
                        evaluated.merge(inner);
                    }
                }
                _ => out.push(failure(
                    at,
                    &join(path, "oneOf"),
                    FailureKind::SeveralBranches {
                        matched: outcome.matched,
                    },
                )),
            }
        }
        if let Some(negated) = map.get("not") {
            let mut ignored = Vec::new();
            self.validate(
                negated,
                instance,
                at,
                &join(path, "not"),
                depth,
                &mut ignored,
            );
            if ignored.is_empty() {
                out.push(failure(at, &join(path, "not"), FailureKind::Not));
            }
        }
        if let Some(condition) = map.get("if") {
            let mut condition_failures = Vec::new();
            let condition_evaluated = self.validate(
                condition,
                instance,
                at,
                &join(path, "if"),
                depth,
                &mut condition_failures,
            );
            if condition_failures.is_empty() {
                evaluated.merge(condition_evaluated);
                if let Some(then) = map.get("then") {
                    let inner = self.validate(then, instance, at, &join(path, "then"), depth, out);
                    evaluated.merge(inner);
                }
            } else if let Some(otherwise) = map.get("else") {
                let inner = self.validate(otherwise, instance, at, &join(path, "else"), depth, out);
                evaluated.merge(inner);
            }
        }

        // Last, because they depend on everything evaluated above at this place.
        if let Value::Object(members) = instance {
            self.check_unevaluated_properties(map, members, at, path, depth, &mut evaluated, out);
        }
        if let Value::Array(items) = instance {
            self.check_unevaluated_items(map, items, at, path, depth, &mut evaluated, out);
        }
        evaluated
    }

    fn resolve(&self, reference: &str) -> Option<&'a Value> {
        let fragment = reference.strip_prefix('#')?;
        if fragment.is_empty() {
            return Some(self.document);
        }
        let pointer = Pointer::parse(&percent_decode(fragment)?)?;
        pointer.find(self.document)
    }

    fn check_type(
        map: &Map<String, Value>,
        instance: &Value,
        at: &Pointer,
        path: &str,
        out: &mut Vec<Failure>,
    ) {
        let Some(declared) = map.get("type") else {
            return;
        };
        let expected: Vec<String> = match declared {
            Value::String(name) => vec![name.clone()],
            Value::Array(names) => names
                .iter()
                .filter_map(|name| name.as_str().map(str::to_owned))
                .collect(),
            _ => return,
        };
        if !expected.iter().any(|name| has_type(instance, name)) {
            out.push(failure(
                at,
                &join(path, "type"),
                FailureKind::Type {
                    expected,
                    received: received_type(instance),
                },
            ));
        }
    }

    fn check_string(
        &mut self,
        map: &Map<String, Value>,
        text: &str,
        at: &Pointer,
        path: &str,
        out: &mut Vec<Failure>,
    ) {
        let length = text.chars().count() as u64;
        if let Some(limit) = map.get("minLength").and_then(Value::as_u64)
            && length < limit
        {
            out.push(failure(
                at,
                &join(path, "minLength"),
                FailureKind::TooShort { limit },
            ));
        }
        if let Some(limit) = map.get("maxLength").and_then(Value::as_u64)
            && length > limit
        {
            out.push(failure(
                at,
                &join(path, "maxLength"),
                FailureKind::TooLong { limit },
            ));
        }
        if map.contains_key("pattern") {
            self.unchecked.insert("pattern".to_owned());
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "prefixItems, items and contains share the set of evaluated positions"
    )]
    fn check_array(
        &mut self,
        map: &'a Map<String, Value>,
        items: &[Value],
        at: &Pointer,
        path: &str,
        depth: usize,
        out: &mut Vec<Failure>,
    ) -> Evaluated {
        let mut evaluated = Evaluated::default();
        let length = items.len() as u64;
        if let Some(limit) = map.get("minItems").and_then(Value::as_u64)
            && length < limit
        {
            out.push(failure(
                at,
                &join(path, "minItems"),
                FailureKind::TooFewItems { limit },
            ));
        }
        if let Some(limit) = map.get("maxItems").and_then(Value::as_u64)
            && length > limit
        {
            out.push(failure(
                at,
                &join(path, "maxItems"),
                FailureKind::TooManyItems { limit },
            ));
        }
        if map.get("uniqueItems") == Some(&Value::Bool(true)) {
            'outer: for (first, left) in items.iter().enumerate() {
                for (offset, right) in items[first + 1..].iter().enumerate() {
                    if json_equal(left, right) {
                        out.push(failure(
                            at,
                            &join(path, "uniqueItems"),
                            FailureKind::NotUnique {
                                first,
                                second: first + 1 + offset,
                            },
                        ));
                        break 'outer;
                    }
                }
            }
        }
        let mut prefix_length = 0;
        if let Some(Value::Array(prefix)) = map.get("prefixItems") {
            prefix_length = prefix.len();
            for (index, (item, item_schema)) in items.iter().zip(prefix).enumerate() {
                self.validate(
                    item_schema,
                    item,
                    &at.index(index),
                    &format!("{}/{index}", join(path, "prefixItems")),
                    depth,
                    out,
                );
                evaluated.items.insert(index);
            }
        }
        if let Some(rest_schema) = map.get("items") {
            for (index, item) in items.iter().enumerate().skip(prefix_length) {
                if rest_schema == &Value::Bool(false) {
                    out.push(failure(
                        &at.index(index),
                        &join(path, "items"),
                        FailureKind::ItemNotAllowed { index },
                    ));
                } else {
                    self.validate(
                        rest_schema,
                        item,
                        &at.index(index),
                        &join(path, "items"),
                        depth,
                        out,
                    );
                }
            }
            evaluated.all_items = true;
        }
        if let Some(contained) = map.get("contains") {
            let mut found = 0;
            for (index, item) in items.iter().enumerate() {
                let mut ignored = Vec::new();
                self.validate(
                    contained,
                    item,
                    &at.index(index),
                    &join(path, "contains"),
                    depth,
                    &mut ignored,
                );
                if ignored.is_empty() {
                    found += 1;
                    evaluated.items.insert(index);
                }
            }
            let minimum = map.get("minContains").and_then(Value::as_u64).unwrap_or(1);
            if (found as u64) < minimum {
                out.push(failure(
                    at,
                    &join(path, "contains"),
                    FailureKind::ContainsTooFew {
                        found,
                        limit: minimum,
                    },
                ));
            }
            if let Some(limit) = map.get("maxContains").and_then(Value::as_u64)
                && found as u64 > limit
            {
                out.push(failure(
                    at,
                    &join(path, "maxContains"),
                    FailureKind::ContainsTooMany { found, limit },
                ));
            }
        }
        evaluated
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the object keywords share the set of evaluated members, and splitting \
                  them would pass that set through several functions for no clarity"
    )]
    fn check_object(
        &mut self,
        map: &'a Map<String, Value>,
        members: &Map<String, Value>,
        at: &Pointer,
        path: &str,
        depth: usize,
        out: &mut Vec<Failure>,
    ) -> Evaluated {
        let mut evaluated = Evaluated::default();
        let count_of_members = members.len() as u64;
        if let Some(limit) = map.get("minProperties").and_then(Value::as_u64)
            && count_of_members < limit
        {
            out.push(failure(
                at,
                &join(path, "minProperties"),
                FailureKind::TooFewProperties { limit },
            ));
        }
        if let Some(limit) = map.get("maxProperties").and_then(Value::as_u64)
            && count_of_members > limit
        {
            out.push(failure(
                at,
                &join(path, "maxProperties"),
                FailureKind::TooManyProperties { limit },
            ));
        }
        if let Some(Value::Array(required)) = map.get("required") {
            for name in required.iter().filter_map(Value::as_str) {
                if !members.contains_key(name) {
                    out.push(failure(
                        at,
                        &join(path, "required"),
                        FailureKind::Missing {
                            property: name.to_owned(),
                        },
                    ));
                }
            }
        }
        if let Some(Value::Object(dependencies)) = map.get("dependentRequired") {
            for (name, needed) in dependencies {
                if !members.contains_key(name) {
                    continue;
                }
                for requires in needed
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                {
                    if !members.contains_key(requires) {
                        out.push(failure(
                            at,
                            &join(path, "dependentRequired"),
                            FailureKind::DependentMissing {
                                property: name.clone(),
                                requires: requires.to_owned(),
                            },
                        ));
                    }
                }
            }
        }
        if let Some(names_schema) = map.get("propertyNames") {
            for name in members.keys() {
                let mut name_failures = Vec::new();
                self.validate(
                    names_schema,
                    &Value::String(name.clone()),
                    at,
                    &join(path, "propertyNames"),
                    depth,
                    &mut name_failures,
                );
                if !name_failures.is_empty() {
                    out.push(failure(
                        at,
                        &join(path, "propertyNames"),
                        FailureKind::BadPropertyName { name: name.clone() },
                    ));
                }
            }
        }
        let declared = map.get("properties").and_then(Value::as_object);
        if let Some(properties) = declared {
            for (name, property_schema) in properties {
                if let Some(value) = members.get(name) {
                    self.validate(
                        property_schema,
                        value,
                        &at.key(name),
                        &format!("{}/{}", join(path, "properties"), escape(name)),
                        depth,
                        out,
                    );
                    evaluated.properties.insert(name.clone());
                }
            }
        }
        let has_patterns = map.contains_key("patternProperties");
        if has_patterns {
            // Without regular expressions nobody can say which members the patterns
            // match, so every member is taken as evaluated and nothing after this can
            // refuse one. That is the side that never refuses what SOLAR accepts.
            self.unchecked.insert("patternProperties".to_owned());
            evaluated.all_properties = true;
        }
        if let Some(additional) = map.get("additionalProperties") {
            if !has_patterns {
                let declared_names: Vec<String> = declared
                    .map(|properties| properties.keys().cloned().collect())
                    .unwrap_or_default();
                for (name, value) in members {
                    if declared.is_some_and(|properties| properties.contains_key(name)) {
                        continue;
                    }
                    if additional == &Value::Bool(false) {
                        out.push(failure(
                            &at.key(name),
                            &join(path, "additionalProperties"),
                            FailureKind::NotAllowed {
                                property: name.clone(),
                                suggestion: closest(name, &declared_names),
                                declared: declared_names.clone(),
                            },
                        ));
                    } else {
                        self.validate(
                            additional,
                            value,
                            &at.key(name),
                            &join(path, "additionalProperties"),
                            depth,
                            out,
                        );
                    }
                }
            }
            evaluated.all_properties = true;
        }
        if let Some(Value::Object(dependent)) = map.get("dependentSchemas") {
            for (name, dependent_schema) in dependent {
                if members.contains_key(name) {
                    let inner = self.validate(
                        dependent_schema,
                        &Value::Object(members.clone()),
                        at,
                        &format!("{}/{}", join(path, "dependentSchemas"), escape(name)),
                        depth,
                        out,
                    );
                    evaluated.merge(inner);
                }
            }
        }
        evaluated
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the evaluation state of one place in the instance, passed as it is"
    )]
    fn check_unevaluated_properties(
        &mut self,
        map: &'a Map<String, Value>,
        members: &Map<String, Value>,
        at: &Pointer,
        path: &str,
        depth: usize,
        evaluated: &mut Evaluated,
        out: &mut Vec<Failure>,
    ) {
        let Some(unevaluated) = map.get("unevaluatedProperties") else {
            return;
        };
        if !evaluated.all_properties {
            for (name, value) in members {
                if evaluated.properties.contains(name) {
                    continue;
                }
                if unevaluated == &Value::Bool(false) {
                    out.push(failure(
                        &at.key(name),
                        &join(path, "unevaluatedProperties"),
                        FailureKind::NotEvaluated {
                            property: name.clone(),
                        },
                    ));
                } else {
                    self.validate(
                        unevaluated,
                        value,
                        &at.key(name),
                        &join(path, "unevaluatedProperties"),
                        depth,
                        out,
                    );
                }
            }
        }
        evaluated.all_properties = true;
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the evaluation state of one place in the instance, passed as it is"
    )]
    fn check_unevaluated_items(
        &mut self,
        map: &'a Map<String, Value>,
        items: &[Value],
        at: &Pointer,
        path: &str,
        depth: usize,
        evaluated: &mut Evaluated,
        out: &mut Vec<Failure>,
    ) {
        let Some(unevaluated) = map.get("unevaluatedItems") else {
            return;
        };
        if !evaluated.all_items {
            for (index, item) in items.iter().enumerate() {
                if evaluated.items.contains(&index) {
                    continue;
                }
                if unevaluated == &Value::Bool(false) {
                    out.push(failure(
                        &at.index(index),
                        &join(path, "unevaluatedItems"),
                        FailureKind::ItemNotAllowed { index },
                    ));
                } else {
                    self.validate(
                        unevaluated,
                        item,
                        &at.index(index),
                        &join(path, "unevaluatedItems"),
                        depth,
                        out,
                    );
                }
            }
        }
        evaluated.all_items = true;
    }

    fn branches(
        &mut self,
        branches: &'a [Value],
        instance: &Value,
        at: &Pointer,
        path: &str,
        depth: usize,
    ) -> Branches {
        let mut matched = Vec::new();
        let mut evaluated = Vec::new();
        let mut closest: Option<Vec<Failure>> = None;
        for (index, branch) in branches.iter().enumerate() {
            let mut branch_failures = Vec::new();
            let inner = self.validate(
                branch,
                instance,
                at,
                &format!("{path}/{index}"),
                depth,
                &mut branch_failures,
            );
            if branch_failures.is_empty() {
                matched.push(index);
                evaluated.push(inner);
            } else if closest
                .as_ref()
                .is_none_or(|best| closer(&branch_failures, best, at))
            {
                closest = Some(branch_failures);
            }
        }
        Branches {
            matched,
            evaluated,
            closest: closest.unwrap_or_default(),
        }
    }
}

struct Branches {
    matched: Vec<usize>,
    evaluated: Vec<Evaluated>,
    closest: Vec<Failure>,
}

/// Whether one branch's failures are closer to a match than another's. A branch whose
/// failures are all inside the value, so that its type at least was right, is closer than
/// one that failed at the value itself; after that, fewer failures are closer.
fn closer(candidate: &[Failure], best: &[Failure], at: &Pointer) -> bool {
    let deep = |failures: &[Failure]| {
        failures
            .iter()
            .all(|failure| failure.at.segments().len() > at.segments().len())
    };
    match (deep(candidate), deep(best)) {
        (true, false) => true,
        (false, true) => false,
        _ => candidate.len() < best.len(),
    }
}

fn failure(at: &Pointer, keyword_path: &str, kind: FailureKind) -> Failure {
    Failure {
        at: at.clone(),
        keyword_path: keyword_path.to_owned(),
        kind,
    }
}

fn join(path: &str, keyword: &str) -> String {
    format!("{path}/{keyword}")
}

fn escape(name: &str) -> String {
    name.replace('~', "~0").replace('/', "~1")
}

/// Decodes the percent escapes a URI fragment may carry, such as `%25` for `%`.
fn percent_decode(fragment: &str) -> Option<String> {
    let bytes = fragment.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            let digits = fragment.get(at + 1..at + 3)?;
            decoded.push(u8::from_str_radix(digits, 16).ok()?);
            at += 3;
        } else {
            decoded.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn has_type(instance: &Value, name: &str) -> bool {
    match (name, instance) {
        ("null", Value::Null)
        | ("boolean", Value::Bool(_))
        | ("object", Value::Object(_))
        | ("array", Value::Array(_))
        | ("string", Value::String(_))
        | ("number", Value::Number(_)) => true,
        ("integer", Value::Number(number)) => is_integer(number),
        _ => false,
    }
}

/// Whether a number is an integer in the sense of JSON Schema, where `1.0` is one.
#[must_use]
pub fn is_integer(number: &Number) -> bool {
    number.is_i64()
        || number.is_u64()
        || number
            .as_f64()
            .is_some_and(|float| float.is_finite() && float.fract() == 0.0)
}

fn check_number(
    map: &Map<String, Value>,
    number: &Number,
    at: &Pointer,
    path: &str,
    out: &mut Vec<Failure>,
) {
    let bound = |keyword: &str| map.get(keyword).and_then(Value::as_number);
    if let Some(limit) = bound("minimum")
        && compare_numbers(number, limit) == Ordering::Less
    {
        out.push(failure(
            at,
            &join(path, "minimum"),
            FailureKind::TooSmall {
                limit: limit.clone(),
                exclusive: false,
            },
        ));
    }
    if let Some(limit) = bound("exclusiveMinimum")
        && compare_numbers(number, limit) != Ordering::Greater
    {
        out.push(failure(
            at,
            &join(path, "exclusiveMinimum"),
            FailureKind::TooSmall {
                limit: limit.clone(),
                exclusive: true,
            },
        ));
    }
    if let Some(limit) = bound("maximum")
        && compare_numbers(number, limit) == Ordering::Greater
    {
        out.push(failure(
            at,
            &join(path, "maximum"),
            FailureKind::TooLarge {
                limit: limit.clone(),
                exclusive: false,
            },
        ));
    }
    if let Some(limit) = bound("exclusiveMaximum")
        && compare_numbers(number, limit) != Ordering::Less
    {
        out.push(failure(
            at,
            &join(path, "exclusiveMaximum"),
            FailureKind::TooLarge {
                limit: limit.clone(),
                exclusive: true,
            },
        ));
    }
    if let Some(divisor) = bound("multipleOf")
        && !is_multiple_of(number, divisor)
    {
        out.push(failure(
            at,
            &join(path, "multipleOf"),
            FailureKind::NotMultipleOf {
                divisor: divisor.clone(),
            },
        ));
    }
}

/// Compares two numbers by value, exactly when both are integers.
#[must_use]
pub fn compare_numbers(left: &Number, right: &Number) -> Ordering {
    if let (Some(a), Some(b)) = (as_i128(left), as_i128(right)) {
        return a.cmp(&b);
    }
    if let (Some(a), Some(b)) = (Decimal::of(left), Decimal::of(right)) {
        a.compare(&b)
    } else {
        let a = left.as_f64().unwrap_or(f64::NAN);
        let b = right.as_f64().unwrap_or(f64::NAN);
        a.partial_cmp(&b).unwrap_or(Ordering::Equal)
    }
}

fn as_i128(number: &Number) -> Option<i128> {
    number
        .as_i64()
        .map(i128::from)
        .or_else(|| number.as_u64().map(i128::from))
}

/// Equality in the sense of JSON Schema: numbers are equal by value, so `1` equals `1.0`,
/// and objects are equal whatever the order of their members.
#[must_use]
pub fn json_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => compare_numbers(a, b) == Ordering::Equal,
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| json_equal(x, y))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(key, value)| b.get(key).is_some_and(|other| json_equal(value, other)))
        }
        _ => left == right,
    }
}

/// A number written in decimal, `mantissa × 10^exponent`, read from the text `serde_json`
/// gives it, so that `0.3` is exactly three tenths rather than the nearest binary
/// fraction. That is what makes `0.3` a multiple of `0.1`, as the specification requires.
#[derive(Debug, Clone, Copy)]
struct Decimal {
    mantissa: i128,
    exponent: i32,
}

impl Decimal {
    fn of(number: &Number) -> Option<Self> {
        let text = number.to_string();
        let (base, exponent) = match text.find(['e', 'E']) {
            Some(at) => (&text[..at], text[at + 1..].parse::<i32>().ok()?),
            None => (text.as_str(), 0),
        };
        let (whole, fraction) = base.split_once('.').unwrap_or((base, ""));
        let digits = format!("{whole}{fraction}");
        let mantissa = digits.parse::<i128>().ok()?;
        let exponent = exponent.checked_sub(i32::try_from(fraction.len()).ok()?)?;
        Some(Self { mantissa, exponent })
    }

    /// Both numbers with the same exponent, when that fits in 128 bits.
    fn aligned(self, other: Self) -> Option<(i128, i128)> {
        let exponent = self.exponent.min(other.exponent);
        let scale = |decimal: Self| -> Option<i128> {
            let shift = u32::try_from(decimal.exponent - exponent).ok()?;
            decimal.mantissa.checked_mul(10_i128.checked_pow(shift)?)
        };
        Some((scale(self)?, scale(other)?))
    }

    fn compare(self, other: &Self) -> Ordering {
        if let Some((a, b)) = self.aligned(*other) {
            a.cmp(&b)
        } else {
            let approximate = |decimal: Self| {
                #[allow(
                    clippy::cast_precision_loss,
                    reason = "only reached when the exact comparison overflows 128 bits"
                )]
                let mantissa = decimal.mantissa as f64;
                mantissa * 10_f64.powi(decimal.exponent)
            };
            approximate(self)
                .partial_cmp(&approximate(*other))
                .unwrap_or(Ordering::Equal)
        }
    }
}

fn is_multiple_of(number: &Number, divisor: &Number) -> bool {
    if let (Some(value), Some(by)) = (Decimal::of(number), Decimal::of(divisor))
        && let Some((value, by)) = value.aligned(by)
    {
        return by != 0 && value % by == 0;
    }
    match (number.as_f64(), divisor.as_f64()) {
        (Some(value), Some(by)) if by != 0.0 => {
            let quotient = value / by;
            quotient.is_finite() && quotient.fract() == 0.0
        }
        _ => true,
    }
}

/// The declared name closest to `name` by edit distance, when it is close enough to be a
/// typing mistake: a distance of at most three and smaller than the name itself. It is the
/// rule SOLAR's contract gives for suggesting a method, so that ZENITH suggests the same
/// names SOLAR would.
#[must_use]
pub fn closest(name: &str, candidates: &[String]) -> Option<String> {
    candidates
        .iter()
        .map(|candidate| (edit_distance(name, candidate), candidate))
        .filter(|(distance, candidate)| *distance <= 3 && *distance < candidate.chars().count())
        .min_by(|(a, x), (b, y)| a.cmp(b).then_with(|| x.cmp(y)))
        .map(|(_, candidate)| candidate.clone())
}

/// The Levenshtein distance between two strings, counted in characters.
#[must_use]
pub fn edit_distance(left: &str, right: &str) -> usize {
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0; right.len() + 1];
    for (i, a) in left.chars().enumerate() {
        current[0] = i + 1;
        for (j, b) in right.iter().enumerate() {
            let substitution = previous[j] + usize::from(a != *b);
            current[j + 1] = substitution.min(previous[j + 1] + 1).min(current[j] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn check(schema: &Value, instance: &Value) -> Report {
        Schema::from_document(schema.clone()).validate(instance)
    }

    fn valid(schema: &Value, instance: &Value) -> bool {
        check(schema, instance).is_valid()
    }

    #[test]
    fn the_ping_schema_accepts_what_solar_accepts() {
        let schema = json!({
            "additionalProperties": false,
            "properties": {"message": {"default": null, "type": ["string", "null"]}},
            "type": "object"
        });
        assert!(valid(&schema, &json!({})));
        assert!(valid(&schema, &json!({"message": "hi"})));
        assert!(valid(&schema, &json!({"message": null})));
        assert!(!valid(&schema, &json!({"message": 3})));
        assert!(!valid(&schema, &json!([])));
    }

    #[test]
    fn an_unknown_parameter_is_reported_with_the_closest_declared_name() {
        let schema = json!({
            "additionalProperties": false,
            "properties": {"message": {"type": "string"}},
            "type": "object"
        });
        let report = check(&schema, &json!({"mesage": "hi"}));
        assert_eq!(report.failures.len(), 1);
        let failure = &report.failures[0];
        assert_eq!(failure.at.to_string(), "/mesage");
        assert_eq!(
            failure.kind,
            FailureKind::NotAllowed {
                property: "mesage".to_owned(),
                declared: vec!["message".to_owned()],
                suggestion: Some("message".to_owned()),
            }
        );
        assert_eq!(
            failure.sentence(),
            "There is no parameter mesage. Did you mean message?"
        );
    }

    #[test]
    fn a_missing_required_parameter_is_reported_at_the_object() {
        let schema = json!({"type": "object", "required": ["api"], "properties": {"api": {}}});
        let report = check(&schema, &json!({}));
        assert_eq!(report.failures[0].at, Pointer::root());
        assert_eq!(
            report.failures[0].sentence(),
            "The parameters must have api, which is required."
        );
    }

    #[test]
    fn references_are_resolved_into_the_shared_definitions() {
        let definitions = json!({"Level": {"enum": ["off", "trace"]}});
        let fragment = json!({"properties": {"level": {"$ref": "#/$defs/Level"}}});
        let schema = Schema::from_fragment(&fragment, &definitions, None);
        assert!(schema.validate(&json!({"level": "trace"})).is_valid());
        let report = schema.validate(&json!({"level": "loud"}));
        assert_eq!(report.failures[0].at.to_string(), "/level");
        assert!(report.unchecked.is_empty());
    }

    #[test]
    fn a_reference_that_leaves_the_document_is_reported_as_unchecked_and_passes() {
        let schema = json!({"$ref": "https://example.org/schema.json"});
        let report = check(&schema, &json!(1));
        assert!(report.is_valid());
        assert!(
            report
                .unchecked
                .contains("$ref to https://example.org/schema.json")
        );
    }

    #[test]
    fn a_reference_to_itself_stops_instead_of_looping() {
        let schema = json!({"$defs": {"loop": {"$ref": "#/$defs/loop"}}, "$ref": "#/$defs/loop"});
        let report = check(&schema, &json!(1));
        assert!(report.is_valid());
        assert!(
            report
                .unchecked
                .iter()
                .any(|keyword| keyword.contains("deeper"))
        );
    }

    #[test]
    fn pattern_is_not_checked_and_says_so() {
        let schema = json!({"type": "string", "pattern": "^a$"});
        let report = check(&schema, &json!("b"));
        assert!(report.is_valid());
        assert!(report.unchecked.contains("pattern"));
    }

    #[test]
    fn integers_are_numbers_with_no_fraction_whatever_their_spelling() {
        let schema = json!({"type": "integer"});
        assert!(valid(&schema, &json!(1)));
        assert!(valid(&schema, &json!(1.0)));
        assert!(!valid(&schema, &json!(1.5)));
        assert!(!valid(&schema, &json!("1")));
    }

    #[test]
    fn bounds_compare_by_value_and_exclusive_bounds_refuse_the_limit() {
        assert!(valid(&json!({"minimum": 0}), &json!(0)));
        assert!(!valid(&json!({"exclusiveMinimum": 0}), &json!(0)));
        assert!(!valid(&json!({"maximum": 10}), &json!(10.5)));
        assert!(valid(
            &json!({"maximum": 18_446_744_073_709_551_615_u64}),
            &json!(u64::MAX)
        ));
        assert!(!valid(&json!({"maximum": -1}), &json!(u64::MAX)));
    }

    #[test]
    fn multiple_of_is_exact_for_decimal_fractions() {
        assert!(valid(&json!({"multipleOf": 0.1}), &json!(0.3)));
        assert!(!valid(&json!({"multipleOf": 0.1}), &json!(0.35)));
        assert!(valid(&json!({"multipleOf": 3}), &json!(9)));
        assert!(!valid(&json!({"multipleOf": 3}), &json!(10)));
    }

    #[test]
    fn lengths_are_counted_in_characters_not_bytes() {
        assert!(valid(&json!({"maxLength": 2}), &json!("çé")));
        assert!(!valid(&json!({"minLength": 3}), &json!("çé")));
    }

    #[test]
    fn enum_and_const_compare_numbers_by_value() {
        assert!(valid(&json!({"enum": [1, "a"]}), &json!(1.0)));
        assert!(valid(&json!({"const": {"a": [1]}}), &json!({"a": [1.0]})));
        assert!(!valid(&json!({"const": null}), &json!(false)));
    }

    #[test]
    fn arrays_check_their_prefix_their_rest_and_what_they_contain() {
        let schema = json!({
            "prefixItems": [{"type": "string"}],
            "items": {"type": "integer"},
            "contains": {"const": 2},
            "maxContains": 1,
            "uniqueItems": true
        });
        assert!(valid(&schema, &json!(["a", 1, 2])));
        assert!(!valid(&schema, &json!([1, 2])));
        assert!(!valid(&schema, &json!(["a", 2, 2])));
        assert!(!valid(&schema, &json!(["a", 1])));
        assert!(!valid(&json!({"items": false}), &json!([1])));
    }

    #[test]
    fn one_of_refuses_both_none_and_several() {
        let schema = json!({"oneOf": [{"type": "integer"}, {"minimum": 0}]});
        assert!(valid(&schema, &json!(-1)));
        assert!(valid(&schema, &json!(0.5)));
        assert!(!valid(&schema, &json!(1)));
        assert!(!valid(&schema, &json!(-0.5)));
    }

    #[test]
    fn the_closest_branch_of_an_any_of_is_the_one_whose_type_was_right() {
        let schema = json!({
            "anyOf": [
                {"type": "object", "properties": {"a": {"type": "string"}}},
                {"type": "null"}
            ]
        });
        let report = check(&schema, &json!({"a": 1}));
        let FailureKind::NoBranch { closest, .. } = &report.failures[0].kind else {
            panic!("expected an anyOf failure");
        };
        assert_eq!(closest[0].at.to_string(), "/a");
    }

    #[test]
    fn if_then_else_applies_the_branch_the_condition_chose() {
        let schema = json!({
            "if": {"properties": {"kind": {"const": "a"}}},
            "then": {"required": ["a"]},
            "else": {"required": ["b"]}
        });
        assert!(valid(&schema, &json!({"kind": "a", "a": 1})));
        assert!(!valid(&schema, &json!({"kind": "a", "b": 1})));
        assert!(valid(&schema, &json!({"kind": "z", "b": 1})));
    }

    #[test]
    fn unevaluated_properties_see_what_the_subschemas_evaluated() {
        let schema = json!({
            "allOf": [{"properties": {"a": {}}}],
            "properties": {"b": {}},
            "unevaluatedProperties": false
        });
        assert!(valid(&schema, &json!({"a": 1, "b": 2})));
        let report = check(&schema, &json!({"a": 1, "c": 3}));
        assert_eq!(report.failures[0].at.to_string(), "/c");
    }

    #[test]
    fn a_failed_branch_evaluates_nothing() {
        let schema = json!({
            "anyOf": [{"properties": {"a": {"type": "string"}}, "required": ["a"]}, true],
            "unevaluatedProperties": false
        });
        assert!(valid(&schema, &json!({"a": "x"})));
        assert!(!valid(&schema, &json!({"a": 1})));
    }

    #[test]
    fn dependent_keywords_apply_only_when_their_member_is_there() {
        let schema = json!({
            "dependentRequired": {"a": ["b"]},
            "dependentSchemas": {"c": {"required": ["d"]}}
        });
        assert!(valid(&schema, &json!({})));
        assert!(!valid(&schema, &json!({"a": 1})));
        assert!(!valid(&schema, &json!({"c": 1})));
        assert!(valid(&schema, &json!({"c": 1, "d": 2})));
    }

    #[test]
    fn property_names_and_counts_are_checked() {
        let schema =
            json!({"propertyNames": {"maxLength": 2}, "minProperties": 1, "maxProperties": 2});
        assert!(valid(&schema, &json!({"ab": 1})));
        assert!(!valid(&schema, &json!({"abc": 1})));
        assert!(!valid(&schema, &json!({})));
        assert!(!valid(&schema, &json!({"a": 1, "b": 2, "c": 3})));
    }

    #[test]
    fn the_schemas_true_and_false_accept_everything_and_nothing() {
        assert!(valid(&json!(true), &json!({"any": "thing"})));
        assert!(!valid(&json!(false), &json!(null)));
        assert!(!valid(&json!({"not": {}}), &json!(1)));
    }

    #[test]
    fn a_reference_with_an_escaped_name_is_found() {
        let schema = json!({"$defs": {"a/b": {"type": "string"}, "c%d": {"type": "null"}},
                            "properties": {"x": {"$ref": "#/$defs/a~1b"}, "y": {"$ref": "#/$defs/c%25d"}}});
        assert!(valid(&schema, &json!({"x": "s", "y": null})));
        assert!(!valid(&schema, &json!({"x": 1})));
    }

    #[test]
    fn the_edit_distance_suggests_only_what_is_close() {
        let names = vec!["message".to_owned(), "api".to_owned()];
        assert_eq!(closest("mesage", &names), Some("message".to_owned()));
        assert_eq!(closest("xyz", &names), None);
        assert_eq!(edit_distance("kitten", "sitting"), 3);
    }

    #[test]
    fn messages_name_the_place_what_was_expected_and_what_arrived() {
        let schema = json!({"properties": {"message": {"type": ["string", "null"]}}});
        let report = check(&schema, &json!({"message": 3}));
        assert_eq!(
            report.failures[0].sentence(),
            "/message should be a string or null, and an integer arrived."
        );
    }
}
