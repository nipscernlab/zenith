//! The APIs tab: the catalogue, the examples and what they returned, and the parameter
//! form.

use std::collections::HashMap;
use std::time::Duration;

use serde_json::{Map, Value};
use zenith_client::manifest::{Api, Catalogue};
use zenith_client::pointer::Pointer;
use zenith_client::schema_view::{FieldKind, View};

use crate::editor::LineEditor;

/// What running an example produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExampleResult {
    /// Sent, not answered.
    Running,
    /// The response matches the example, by the example's rule.
    Matches {
        /// The round trip.
        round_trip: Duration,
    },
    /// The response does not match the example.
    Differs {
        /// Where and how, as a sentence.
        sentence: String,
    },
    /// SOLAR answered with an error.
    Failed {
        /// Its status.
        status: String,
        /// Its reason.
        reason: String,
    },
    /// No response came.
    Closed {
        /// Why.
        why: String,
    },
}

/// The APIs tab.
#[derive(Debug, Clone, Default)]
pub struct ApisTab {
    /// The selected API, as a position in the filtered list.
    pub selected: usize,
    /// The filter being typed or applied.
    pub filter: LineEditor,
    /// Whether the filter has the keys.
    pub filtering: bool,
    /// How far the entry is scrolled.
    pub scroll: u16,
    /// The parameter form, when it is open.
    pub form: Option<Form>,
    /// What each example returned the last time it ran, by API and example position.
    pub results: HashMap<(String, usize), ExampleResult>,
}

impl ApisTab {
    /// The APIs the filter lets through, in the manifest's order.
    #[must_use]
    pub fn visible<'a>(&self, catalogue: &'a Catalogue) -> Vec<&'a Api> {
        let filter = self.filter.text().trim();
        catalogue
            .apis
            .iter()
            .filter(|api| filter.is_empty() || api.name.contains(filter))
            .collect()
    }

    /// The selected API.
    #[must_use]
    pub fn current<'a>(&self, catalogue: &'a Catalogue) -> Option<&'a Api> {
        let visible = self.visible(catalogue);
        visible
            .get(self.selected.min(visible.len().saturating_sub(1)))
            .copied()
    }
}

/// One field of the parameter form.
#[derive(Debug, Clone)]
pub struct Field {
    /// The parameter.
    pub name: String,
    /// Whether it must be filled in.
    pub required: bool,
    /// How it is filled in.
    pub kind: FieldKind,
    /// Its type in words.
    pub summary: String,
    /// What the schema says it is for.
    pub description: Option<String>,
    /// The schema's default.
    pub default: Option<Value>,
    /// The text of a text, number or JSON field.
    pub editor: LineEditor,
    /// The value of a boolean field, `None` until one is chosen.
    pub boolean: Option<bool>,
    /// The position of the chosen value of a field with fixed values.
    pub choice: Option<usize>,
}

/// The parameter form of one API, built from its `params_schema`.
#[derive(Debug, Clone)]
pub struct Form {
    /// The API.
    pub api: String,
    /// One field per parameter, in the schema's order.
    pub fields: Vec<Field>,
    /// The field that has the keys.
    pub focus: usize,
    /// The failures of the last attempt, by field, or for the whole form.
    pub errors: Vec<(Option<usize>, String)>,
    /// The keywords ZENITH did not check the last time.
    pub unchecked: Vec<String>,
}

impl Form {
    /// The form of an API.
    #[must_use]
    pub fn for_api(api: &Api) -> Self {
        let view = View::new(api.params());
        let root = view.at(&Pointer::root());
        let fields = view
            .properties(&root)
            .into_iter()
            .map(|property| {
                let description = view.describe(&property.schemas);
                Field {
                    kind: view.field_kind(&property.schemas),
                    summary: description.summary(),
                    description: description.description.clone(),
                    default: description.default.clone(),
                    name: property.name,
                    required: property.required,
                    editor: LineEditor::new(),
                    boolean: None,
                    choice: None,
                }
            })
            .collect();
        Self {
            api: api.name.clone(),
            fields,
            focus: 0,
            errors: Vec::new(),
            unchecked: Vec::new(),
        }
    }

    /// The field with the keys.
    pub fn focused(&mut self) -> Option<&mut Field> {
        self.fields.get_mut(self.focus)
    }

    /// The parameters the fields make, leaving out every empty optional field.
    ///
    /// # Errors
    ///
    /// Each field that cannot be read, with why: a required field left empty, a number
    /// that is not one, JSON that does not parse.
    pub fn params(&self) -> Result<Value, Vec<(Option<usize>, String)>> {
        let mut params = Map::new();
        let mut errors = Vec::new();
        for (index, field) in self.fields.iter().enumerate() {
            match field.value() {
                Ok(Some(value)) => {
                    params.insert(field.name.clone(), value);
                }
                Ok(None) if field.required => {
                    errors.push((Some(index), format!("{} is required.", field.name)));
                }
                Ok(None) => {}
                Err(message) => errors.push((Some(index), message)),
            }
        }
        if errors.is_empty() {
            Ok(Value::Object(params))
        } else {
            Err(errors)
        }
    }

    /// The command the form is equivalent to, which is how a person learns the command
    /// line from the form.
    #[must_use]
    pub fn command(&self) -> Option<String> {
        let params = self.params().ok()?;
        Some(format!("/call {} {params}", self.api))
    }
}

impl Field {
    /// The value the field holds, `None` when it is empty.
    ///
    /// # Errors
    ///
    /// Why the text is not a value of the field's kind.
    pub fn value(&self) -> Result<Option<Value>, String> {
        let text = self.editor.text().trim();
        match &self.kind {
            FieldKind::Boolean => Ok(self.boolean.map(Value::Bool)),
            FieldKind::Choice(values) => {
                Ok(self.choice.and_then(|index| values.get(index).cloned()))
            }
            _ if text.is_empty() => Ok(None),
            FieldKind::Text => Ok(Some(Value::String(self.editor.text().to_owned()))),
            FieldKind::Integer => match serde_json::from_str::<Value>(text) {
                Ok(Value::Number(number)) if number.is_i64() || number.is_u64() => {
                    Ok(Some(Value::Number(number)))
                }
                _ => Err(format!(
                    "{} should be a whole number, and {text} is not.",
                    self.name
                )),
            },
            FieldKind::Number => match serde_json::from_str::<Value>(text) {
                Ok(Value::Number(number)) => Ok(Some(Value::Number(number))),
                _ => Err(format!(
                    "{} should be a number, and {text} is not.",
                    self.name
                )),
            },
            FieldKind::Json => serde_json::from_str::<Value>(text)
                .map(Some)
                .map_err(|error| format!("{} is not JSON: {error}.", self.name)),
        }
    }

    /// Moves a field with fixed values to the next or the previous one, and a boolean to
    /// its other value.
    pub fn cycle(&mut self, forward: bool) {
        match &self.kind {
            FieldKind::Choice(values) if !values.is_empty() => {
                let count = values.len();
                self.choice = Some(match (self.choice, forward) {
                    (None, true) => 0,
                    (None, false) => count - 1,
                    (Some(index), true) => (index + 1) % count,
                    (Some(index), false) => (index + count - 1) % count,
                });
            }
            FieldKind::Boolean => self.boolean = Some(!self.boolean.unwrap_or(false)),
            _ => {}
        }
    }

    /// The value as the form shows it.
    #[must_use]
    pub fn shown(&self) -> String {
        match &self.kind {
            FieldKind::Boolean => self
                .boolean
                .map_or_else(String::new, |value| value.to_string()),
            FieldKind::Choice(values) => self
                .choice
                .and_then(|index| values.get(index))
                .map_or_else(String::new, Value::to_string),
            _ => self.editor.text().to_owned(),
        }
    }

    /// Whether the field takes text, so that printable keys go into it.
    #[must_use]
    pub fn takes_text(&self) -> bool {
        !matches!(self.kind, FieldKind::Boolean | FieldKind::Choice(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn catalogue() -> Catalogue {
        let text = include_str!("../../../zenith-client/tests/fixtures/solar-0.1.0-manifest.json");
        Catalogue::from_data(&serde_json::from_str(text).unwrap()).unwrap()
    }

    #[test]
    fn the_form_of_ping_has_one_optional_text_field() {
        let catalogue = catalogue();
        let form = Form::for_api(catalogue.api("solar.ping").unwrap());
        assert_eq!(form.fields.len(), 1);
        let field = &form.fields[0];
        assert_eq!((field.name.as_str(), field.required), ("message", false));
        assert_eq!(field.kind, FieldKind::Text);
        assert_eq!(form.params(), Ok(json!({})));
        assert_eq!(form.command().as_deref(), Some("/call solar.ping {}"));
    }

    #[test]
    fn a_required_field_left_empty_is_an_error_at_that_field() {
        let catalogue = catalogue();
        let form = Form::for_api(catalogue.api("solar.describe").unwrap());
        assert_eq!(
            form.params(),
            Err(vec![(Some(0), "api is required.".to_owned())])
        );
    }

    #[test]
    fn filled_fields_make_the_parameters() {
        let catalogue = catalogue();
        let mut form = Form::for_api(catalogue.api("solar.describe").unwrap());
        form.fields[0].editor.set("solar.ping");
        assert_eq!(form.params(), Ok(json!({"api": "solar.ping"})));
    }

    #[test]
    fn a_choice_cycles_both_ways_and_a_boolean_toggles() {
        let mut field = Field {
            name: "mode".to_owned(),
            required: true,
            kind: FieldKind::Choice(vec![json!("exact"), json!("subset")]),
            summary: String::new(),
            description: None,
            default: None,
            editor: LineEditor::new(),
            boolean: None,
            choice: None,
        };
        field.cycle(false);
        assert_eq!(field.value(), Ok(Some(json!("subset"))));
        field.cycle(true);
        assert_eq!(field.shown(), "\"exact\"");
        field.kind = FieldKind::Boolean;
        field.cycle(true);
        assert_eq!(field.value(), Ok(Some(json!(true))));
    }

    #[test]
    fn numbers_that_are_not_numbers_are_refused_by_name() {
        let mut field = Field {
            name: "depth".to_owned(),
            required: false,
            kind: FieldKind::Integer,
            summary: String::new(),
            description: None,
            default: None,
            editor: LineEditor::new(),
            boolean: None,
            choice: None,
        };
        field.editor.set("1.5");
        assert!(field.value().unwrap_err().contains("whole number"));
        field.editor.set("42");
        assert_eq!(field.value(), Ok(Some(json!(42))));
    }

    #[test]
    fn the_filter_keeps_the_apis_whose_name_contains_it() {
        let catalogue = catalogue();
        let mut tab = ApisTab::default();
        tab.filter.set("solar.");
        assert_eq!(tab.visible(&catalogue).len(), 4);
        tab.filter.set("info");
        assert_eq!(tab.current(&catalogue).unwrap().name, "system.info");
    }

    fn field(kind: FieldKind) -> Field {
        Field {
            name: "f".to_owned(),
            required: false,
            kind,
            summary: String::new(),
            description: None,
            default: None,
            editor: LineEditor::new(),
            boolean: None,
            choice: None,
        }
    }

    fn typed(kind: FieldKind, text: &str) -> Result<Option<Value>, String> {
        let mut field = field(kind);
        field.editor.set(text);
        field.value()
    }

    #[test]
    fn a_whole_number_may_be_negative_or_larger_than_a_signed_one() {
        assert_eq!(typed(FieldKind::Integer, "-5"), Ok(Some(json!(-5))));
        assert_eq!(
            typed(FieldKind::Integer, "18446744073709551615"),
            Ok(Some(json!(18_446_744_073_709_551_615_u64)))
        );
        assert!(typed(FieldKind::Integer, "2.5").is_err());
        assert_eq!(typed(FieldKind::Number, "2.5"), Ok(Some(json!(2.5))));
        assert_eq!(typed(FieldKind::Number, "-7"), Ok(Some(json!(-7))));
        assert!(typed(FieldKind::Number, "seven").is_err());
    }

    #[test]
    fn a_choice_cycles_round_both_ways_from_nothing_chosen() {
        let mut choice = field(FieldKind::Choice(vec![json!("a"), json!("b"), json!("c")]));
        choice.cycle(true);
        assert_eq!(choice.choice, Some(0));
        choice.cycle(true);
        choice.cycle(true);
        assert_eq!(choice.choice, Some(2));
        choice.cycle(true);
        assert_eq!(choice.choice, Some(0), "forward from the last is the first");
        choice.cycle(false);
        assert_eq!(choice.choice, Some(2), "back from the first is the last");
        choice.cycle(false);
        assert_eq!(choice.choice, Some(1));
        let mut backwards = field(FieldKind::Choice(vec![json!(1), json!(2)]));
        backwards.cycle(false);
        assert_eq!(backwards.choice, Some(1), "back from nothing is the last");
        // A choice of nothing stays unchosen.
        let mut empty = field(FieldKind::Choice(Vec::new()));
        empty.cycle(false);
        empty.cycle(true);
        assert_eq!(empty.choice, None);
    }

    #[test]
    fn only_a_field_without_fixed_values_takes_the_keys_as_text() {
        for kind in [
            FieldKind::Text,
            FieldKind::Integer,
            FieldKind::Number,
            FieldKind::Json,
        ] {
            assert!(field(kind.clone()).takes_text(), "{kind:?}");
        }
        assert!(!field(FieldKind::Boolean).takes_text());
        assert!(!field(FieldKind::Choice(vec![json!(1)])).takes_text());
    }
}
