//! JSON pointers, RFC 6901, kept as typed segments.
//!
//! SOLAR names the place of a problem with a pointer into `params`, such as `/tools/0`,
//! and ZENITH's own validator does the same, so that an error found before sending and an
//! error SOLAR returns point at the parameters in the same notation.

use std::fmt;

/// One step of a pointer: a member of an object or an element of an array.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Segment {
    /// A member of an object, by name.
    Key(String),
    /// An element of an array, by position.
    Index(usize),
}

/// A JSON pointer. The empty pointer is the whole document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Pointer(Vec<Segment>);

impl Pointer {
    /// The pointer to the whole document.
    #[must_use]
    pub fn root() -> Self {
        Self(Vec::new())
    }

    /// Whether this is the pointer to the whole document.
    #[must_use]
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    /// The segments, from the outside in.
    #[must_use]
    pub fn segments(&self) -> &[Segment] {
        &self.0
    }

    /// This pointer with a member name appended.
    #[must_use]
    pub fn key(&self, name: &str) -> Self {
        let mut next = self.clone();
        next.0.push(Segment::Key(name.to_owned()));
        next
    }

    /// This pointer with an array position appended.
    #[must_use]
    pub fn index(&self, position: usize) -> Self {
        let mut next = self.clone();
        next.0.push(Segment::Index(position));
        next
    }

    /// The last segment, when there is one.
    #[must_use]
    pub fn last(&self) -> Option<&Segment> {
        self.0.last()
    }

    /// Parses the text form, `/a/0/b~1c`. Every segment is read as a key, because the
    /// text form does not say which ones are positions; [`Pointer::find`] accepts a key
    /// made of digits as a position when it meets an array.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        if text.is_empty() {
            return Some(Self::root());
        }
        let rest = text.strip_prefix('/')?;
        let segments = rest
            .split('/')
            .map(|raw| Segment::Key(raw.replace("~1", "/").replace("~0", "~")))
            .collect();
        Some(Self(segments))
    }

    /// The value this pointer names inside `document`, if there is one.
    #[must_use]
    pub fn find<'a>(&self, document: &'a serde_json::Value) -> Option<&'a serde_json::Value> {
        let mut current = document;
        for segment in &self.0 {
            current = match (segment, current) {
                (Segment::Key(name), serde_json::Value::Object(map)) => map.get(name)?,
                (Segment::Index(position), serde_json::Value::Array(items)) => {
                    items.get(*position)?
                }
                (Segment::Key(name), serde_json::Value::Array(items)) => {
                    items.get(name.parse::<usize>().ok()?)?
                }
                _ => return None,
            };
        }
        Some(current)
    }
}

impl fmt::Display for Pointer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for segment in &self.0 {
            match segment {
                Segment::Key(name) => {
                    write!(formatter, "/{}", name.replace('~', "~0").replace('/', "~1"))?;
                }
                Segment::Index(position) => write!(formatter, "/{position}")?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_root_is_written_as_the_empty_string() {
        assert_eq!(Pointer::root().to_string(), "");
        assert!(Pointer::root().is_root());
    }

    #[test]
    fn keys_and_positions_are_written_with_the_two_escapes() {
        let pointer = Pointer::root().key("a/b").index(3).key("c~d");
        assert_eq!(pointer.to_string(), "/a~1b/3/c~0d");
    }

    #[test]
    fn a_parsed_pointer_finds_the_value_it_names() {
        let document = json!({"tools": [{"name": "x"}, {"name": "y"}], "a/b": 1});
        let pointer = Pointer::parse("/tools/1/name").unwrap();
        assert_eq!(pointer.find(&document), Some(&json!("y")));
        assert_eq!(
            Pointer::parse("/a~1b").unwrap().find(&document),
            Some(&json!(1))
        );
        assert_eq!(Pointer::parse("/tools/9").unwrap().find(&document), None);
    }

    #[test]
    fn text_that_does_not_start_with_a_slash_is_not_a_pointer() {
        assert_eq!(Pointer::parse("tools"), None);
        assert_eq!(Pointer::parse(""), Some(Pointer::root()));
    }
}
