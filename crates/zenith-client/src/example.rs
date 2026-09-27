//! Comparing the response of an example with what came back, by the rules of section 8.2
//! of SOLAR's contract.
//!
//! `exact` is deep equality. `subset` asks that every member the example names exists in
//! the real output and matches; members it does not name are ignored; arrays have the
//! same length and match element by element. In both, the string `"$any"` matches any
//! value at its position, as long as the position exists.

use serde_json::Value;

use crate::manifest::Matching;
use crate::pointer::Pointer;
use crate::schema::{compact, json_equal};

/// The string that matches anything, section 8.2.
pub const ANY: &str = "$any";

/// The first place where the real output differs from the example.
#[derive(Debug, Clone, PartialEq)]
pub struct Difference {
    /// Where, in the output.
    pub at: Pointer,
    /// What happened there.
    pub kind: DifferenceKind,
}

/// How the output differs at one place.
#[derive(Debug, Clone, PartialEq)]
pub enum DifferenceKind {
    /// The example names a member the output does not have.
    Missing {
        /// What the example expected there.
        expected: Value,
    },
    /// The output has a member the example does not name, which `exact` refuses.
    Extra {
        /// What the output has there.
        received: Value,
    },
    /// The two values differ.
    Different {
        /// The example's value.
        expected: Value,
        /// The output's value.
        received: Value,
    },
    /// Two arrays differ in length.
    Length {
        /// The example's length.
        expected: usize,
        /// The output's length.
        received: usize,
    },
    /// The example's rule is one ZENITH does not know, so nothing can be said.
    UnknownRule {
        /// The rule.
        rule: String,
    },
}

impl Difference {
    /// One sentence, for the screen.
    #[must_use]
    pub fn sentence(&self) -> String {
        let place = if self.at.is_root() {
            "the output".to_owned()
        } else {
            self.at.to_string()
        };
        match &self.kind {
            DifferenceKind::Missing { expected } => {
                format!(
                    "differs at {place}: expected {}, and it is absent",
                    compact(expected)
                )
            }
            DifferenceKind::Extra { received } => format!(
                "differs at {place}: {} is there, and the example does not name it",
                compact(received)
            ),
            DifferenceKind::Different { expected, received } => format!(
                "differs at {place}: expected {}, received {}",
                compact(expected),
                compact(received)
            ),
            DifferenceKind::Length { expected, received } => {
                format!("differs at {place}: expected {expected} elements, received {received}")
            }
            DifferenceKind::UnknownRule { rule } => {
                format!("cannot be compared: the example's rule {rule:?} is not one ZENITH knows")
            }
        }
    }
}

#[allow(
    clippy::result_large_err,
    reason = "a difference is returned once per comparison, and boxing it would buy nothing"
)]
/// Compares the example's `response` with the real `data`.
///
/// # Errors
///
/// The first [`Difference`] found, in the order the example is written.
pub fn compare(expected: &Value, received: &Value, matching: &Matching) -> Result<(), Difference> {
    let subset = match matching {
        Matching::Exact => false,
        Matching::Subset => true,
        Matching::Unknown(rule) => {
            return Err(Difference {
                at: Pointer::root(),
                kind: DifferenceKind::UnknownRule { rule: rule.clone() },
            });
        }
    };
    walk(expected, received, subset, &Pointer::root())
}

#[allow(
    clippy::result_large_err,
    reason = "a difference is returned once per comparison, and boxing it would buy nothing"
)]
fn walk(expected: &Value, received: &Value, subset: bool, at: &Pointer) -> Result<(), Difference> {
    if expected.as_str() == Some(ANY) {
        return Ok(());
    }
    match (expected, received) {
        (Value::Object(want), Value::Object(have)) => {
            for (key, value) in want {
                match have.get(key) {
                    Some(actual) => walk(value, actual, subset, &at.key(key))?,
                    None => {
                        return Err(Difference {
                            at: at.key(key),
                            kind: DifferenceKind::Missing {
                                expected: value.clone(),
                            },
                        });
                    }
                }
            }
            if !subset
                && let Some((key, value)) = have.iter().find(|(key, _)| !want.contains_key(*key))
            {
                return Err(Difference {
                    at: at.key(key),
                    kind: DifferenceKind::Extra {
                        received: value.clone(),
                    },
                });
            }
            Ok(())
        }
        (Value::Array(want), Value::Array(have)) => {
            if want.len() != have.len() {
                return Err(Difference {
                    at: at.clone(),
                    kind: DifferenceKind::Length {
                        expected: want.len(),
                        received: have.len(),
                    },
                });
            }
            for (index, (value, actual)) in want.iter().zip(have).enumerate() {
                walk(value, actual, subset, &at.index(index))?;
            }
            Ok(())
        }
        _ if json_equal(expected, received) => Ok(()),
        _ => Err(Difference {
            at: at.clone(),
            kind: DifferenceKind::Different {
                expected: expected.clone(),
                received: received.clone(),
            },
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn any_matches_every_value_that_is_there() {
        let expected = json!({"echo": null, "pong": true, "received_at": "$any"});
        let received = json!({"echo": null, "pong": true, "received_at": "2026-09-27T02:58Z"});
        assert_eq!(compare(&expected, &received, &Matching::Exact), Ok(()));
    }

    #[test]
    fn any_does_not_match_a_member_that_is_absent() {
        let expected = json!({"received_at": "$any"});
        let difference = compare(&expected, &json!({}), &Matching::Subset).unwrap_err();
        assert_eq!(difference.at.to_string(), "/received_at");
    }

    #[test]
    fn exact_refuses_a_member_the_example_does_not_name_and_subset_ignores_it() {
        let expected = json!({"pong": true});
        let received = json!({"pong": true, "echo": null});
        assert!(compare(&expected, &received, &Matching::Subset).is_ok());
        let difference = compare(&expected, &received, &Matching::Exact).unwrap_err();
        assert_eq!(
            difference.sentence(),
            "differs at /echo: null is there, and the example does not name it"
        );
    }

    #[test]
    fn subset_goes_into_nested_objects_and_compares_arrays_element_by_element() {
        let expected = json!({"apis": [{"name": "solar.ping", "version": "$any"}]});
        let received = json!({"apis": [{"name": "solar.ping", "version": "1.0.0", "x": 1}]});
        assert!(compare(&expected, &received, &Matching::Subset).is_ok());
        let longer = json!({"apis": [{"name": "solar.ping"}, {"name": "b"}]});
        let difference = compare(&expected, &longer, &Matching::Subset).unwrap_err();
        assert_eq!(
            difference.kind,
            DifferenceKind::Length {
                expected: 1,
                received: 2
            }
        );
    }

    #[test]
    fn a_different_value_is_reported_with_both_sides() {
        let difference = compare(
            &json!({"echo": "hi"}),
            &json!({"echo": null}),
            &Matching::Exact,
        )
        .unwrap_err();
        assert_eq!(
            difference.sentence(),
            "differs at /echo: expected \"hi\", received null"
        );
    }

    #[test]
    fn a_rule_zenith_does_not_know_cannot_be_compared() {
        let rule = Matching::Unknown("fuzzy".to_owned());
        let difference = compare(&json!({}), &json!({}), &rule).unwrap_err();
        assert!(difference.sentence().contains("fuzzy"));
    }
}
