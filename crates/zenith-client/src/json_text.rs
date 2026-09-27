//! JSON as text a person is typing: where each part of it is, and where the cursor is.
//!
//! Two questions need the text rather than the parsed value. Validation must underline
//! the part of the line an error refers to, so it needs the byte span of every value and
//! every key. Completion must know, in a line that is not finished and not valid, whether
//! the cursor stands where a key goes or where a value goes, and inside which objects.
//! `serde_json` answers neither, so this module does, and nothing else.

use crate::pointer::{Pointer, Segment};

/// A range of bytes in the text, `start..end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// The first byte.
    pub start: usize,
    /// One past the last byte.
    pub end: usize,
}

impl Span {
    /// A span of no bytes at `at`, which is where an insertion goes.
    #[must_use]
    pub fn empty(at: usize) -> Self {
        Self { start: at, end: at }
    }
}

/// A value of a valid JSON text, with where it is.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// An object and its members, in the order they were written.
    Object {
        /// From the opening brace to the closing one, both included.
        span: Span,
        /// The members.
        members: Vec<Member>,
    },
    /// An array and its elements.
    Array {
        /// From the opening bracket to the closing one, both included.
        span: Span,
        /// The elements.
        items: Vec<Node>,
    },
    /// A string, a number, `true`, `false` or `null`.
    Scalar {
        /// The token, quotes included for a string.
        span: Span,
    },
}

/// One member of an object: its key and its value, each with its span.
#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    /// The key, with its escapes decoded.
    pub key: String,
    /// The key as written, quotes included.
    pub key_span: Span,
    /// The value.
    pub value: Node,
}

impl Node {
    /// Where this value is in the text.
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::Object { span, .. } | Self::Array { span, .. } | Self::Scalar { span } => *span,
        }
    }

    /// The node a pointer names, if the text has it.
    #[must_use]
    pub fn find(&self, pointer: &Pointer) -> Option<&Node> {
        let mut current = self;
        for segment in pointer.segments() {
            current = match (segment, current) {
                (Segment::Key(name), Self::Object { members, .. }) => {
                    &members
                        .iter()
                        .rev()
                        .find(|member| &member.key == name)?
                        .value
                }
                (Segment::Index(position), Self::Array { items, .. }) => items.get(*position)?,
                (Segment::Key(name), Self::Array { items, .. }) => {
                    items.get(name.parse::<usize>().ok()?)?
                }
                _ => return None,
            };
        }
        Some(current)
    }

    /// The member called `key` of this object, the last one when a key is repeated,
    /// which is the one `serde_json` keeps.
    #[must_use]
    pub fn member(&self, key: &str) -> Option<&Member> {
        match self {
            Self::Object { members, .. } => members.iter().rev().find(|member| member.key == key),
            _ => None,
        }
    }
}

/// How deep the parsers go before they give up, so that a line of ten thousand brackets
/// cannot overflow the stack.
const MAX_DEPTH: usize = 128;

/// The span tree of a valid JSON text, or `None` when the text is not valid JSON.
///
/// Callers parse with `serde_json` first, for the value and the error message, and come
/// here only for the spans.
#[must_use]
pub fn spans(text: &str) -> Option<Node> {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        text,
        at: 0,
    };
    parser.skip_whitespace();
    let node = parser.value(0)?;
    parser.skip_whitespace();
    (parser.at == parser.bytes.len()).then_some(node)
}

struct Parser<'a> {
    bytes: &'a [u8],
    text: &'a str,
    at: usize,
}

impl Parser<'_> {
    fn skip_whitespace(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.bytes.get(self.at) {
            self.at += 1;
        }
    }

    fn value(&mut self, depth: usize) -> Option<Node> {
        if depth > MAX_DEPTH {
            return None;
        }
        match self.bytes.get(self.at)? {
            b'{' => self.object(depth),
            b'[' => self.array(depth),
            b'"' => {
                let start = self.at;
                self.string()?;
                Some(Node::Scalar {
                    span: Span {
                        start,
                        end: self.at,
                    },
                })
            }
            b'-' | b'0'..=b'9' => self.number(),
            b't' => self.literal("true"),
            b'f' => self.literal("false"),
            b'n' => self.literal("null"),
            _ => None,
        }
    }

    fn object(&mut self, depth: usize) -> Option<Node> {
        let start = self.at;
        self.at += 1;
        let mut members = Vec::new();
        self.skip_whitespace();
        if self.bytes.get(self.at) == Some(&b'}') {
            self.at += 1;
            return Some(Node::Object {
                span: Span {
                    start,
                    end: self.at,
                },
                members,
            });
        }
        loop {
            self.skip_whitespace();
            if self.bytes.get(self.at) != Some(&b'"') {
                return None;
            }
            let key_start = self.at;
            let key = self.string()?;
            let key_span = Span {
                start: key_start,
                end: self.at,
            };
            self.skip_whitespace();
            if self.bytes.get(self.at) != Some(&b':') {
                return None;
            }
            self.at += 1;
            self.skip_whitespace();
            let value = self.value(depth + 1)?;
            members.push(Member {
                key,
                key_span,
                value,
            });
            self.skip_whitespace();
            match self.bytes.get(self.at)? {
                b',' => self.at += 1,
                b'}' => {
                    self.at += 1;
                    return Some(Node::Object {
                        span: Span {
                            start,
                            end: self.at,
                        },
                        members,
                    });
                }
                _ => return None,
            }
        }
    }

    fn array(&mut self, depth: usize) -> Option<Node> {
        let start = self.at;
        self.at += 1;
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.bytes.get(self.at) == Some(&b']') {
            self.at += 1;
            return Some(Node::Array {
                span: Span {
                    start,
                    end: self.at,
                },
                items,
            });
        }
        loop {
            self.skip_whitespace();
            items.push(self.value(depth + 1)?);
            self.skip_whitespace();
            match self.bytes.get(self.at)? {
                b',' => self.at += 1,
                b']' => {
                    self.at += 1;
                    return Some(Node::Array {
                        span: Span {
                            start,
                            end: self.at,
                        },
                        items,
                    });
                }
                _ => return None,
            }
        }
    }

    /// Reads a whole string starting at its opening quote and returns it decoded.
    fn string(&mut self) -> Option<String> {
        let (decoded, end, terminated) = read_string(self.text, self.at + 1);
        if !terminated {
            return None;
        }
        self.at = end;
        decoded
    }

    fn number(&mut self) -> Option<Node> {
        let start = self.at;
        if self.bytes.get(self.at) == Some(&b'-') {
            self.at += 1;
        }
        match self.bytes.get(self.at)? {
            b'0' => self.at += 1,
            b'1'..=b'9' => self.digits(),
            _ => return None,
        }
        if self.bytes.get(self.at) == Some(&b'.') {
            self.at += 1;
            if !self.bytes.get(self.at)?.is_ascii_digit() {
                return None;
            }
            self.digits();
        }
        if let Some(b'e' | b'E') = self.bytes.get(self.at) {
            self.at += 1;
            if let Some(b'+' | b'-') = self.bytes.get(self.at) {
                self.at += 1;
            }
            if !self.bytes.get(self.at)?.is_ascii_digit() {
                return None;
            }
            self.digits();
        }
        Some(Node::Scalar {
            span: Span {
                start,
                end: self.at,
            },
        })
    }

    fn digits(&mut self) {
        while self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) {
            self.at += 1;
        }
    }

    fn literal(&mut self, word: &str) -> Option<Node> {
        let start = self.at;
        let end = start + word.len();
        if self.bytes.get(start..end)? != word.as_bytes() {
            return None;
        }
        self.at = end;
        Some(Node::Scalar {
            span: Span { start, end },
        })
    }
}

/// Reads the content of a string whose opening quote is just before `from`.
///
/// Returns the decoded content, or `None` when an escape is malformed; the byte just past
/// the closing quote, or the end of the text; and whether the closing quote was found. An
/// escape cut off by the end of the text is dropped rather than refused, because a person
/// in the middle of typing one has not made a mistake yet.
fn read_string(text: &str, from: usize) -> (Option<String>, usize, bool) {
    let bytes = text.as_bytes();
    let mut decoded = String::new();
    let mut valid = true;
    let mut at = from;
    let mut run_start = at;
    while at < bytes.len() {
        match bytes[at] {
            b'"' => {
                decoded.push_str(&text[run_start..at]);
                return (valid.then_some(decoded), at + 1, true);
            }
            b'\\' => {
                decoded.push_str(&text[run_start..at]);
                let Some(&escape) = bytes.get(at + 1) else {
                    return (valid.then_some(decoded), bytes.len(), false);
                };
                at += 2;
                match escape {
                    b'"' => decoded.push('"'),
                    b'\\' => decoded.push('\\'),
                    b'/' => decoded.push('/'),
                    b'b' => decoded.push('\u{8}'),
                    b'f' => decoded.push('\u{c}'),
                    b'n' => decoded.push('\n'),
                    b'r' => decoded.push('\r'),
                    b't' => decoded.push('\t'),
                    b'u' => match read_unicode_escape(text, at) {
                        Escape::Char(character, next) => {
                            decoded.push(character);
                            at = next;
                        }
                        Escape::CutOff => return (valid.then_some(decoded), bytes.len(), false),
                        Escape::Invalid(next) => {
                            valid = false;
                            at = next;
                        }
                    },
                    _ => valid = false,
                }
                run_start = at;
            }
            // A raw control character is not allowed inside a JSON string.
            0x00..=0x1f => {
                valid = false;
                at += 1;
            }
            _ => at += 1,
        }
    }
    decoded.push_str(&text[run_start.min(bytes.len())..]);
    (valid.then_some(decoded), bytes.len(), false)
}

enum Escape {
    Char(char, usize),
    CutOff,
    Invalid(usize),
}

/// Reads the four hex digits after `\u`, and a second escape when the first is the high
/// half of a surrogate pair.
fn read_unicode_escape(text: &str, at: usize) -> Escape {
    let Some(first) = hex4(text, at) else {
        return if text.len() < at + 4 {
            Escape::CutOff
        } else {
            Escape::Invalid(at)
        };
    };
    let next = at + 4;
    if (0xD800..0xDC00).contains(&first) {
        if text.len() < next + 6 {
            return Escape::CutOff;
        }
        if text.get(next..next + 2) != Some("\\u") {
            return Escape::Invalid(next);
        }
        let Some(second) = hex4(text, next + 2) else {
            return Escape::Invalid(next);
        };
        if !(0xDC00..0xE000).contains(&second) {
            return Escape::Invalid(next + 6);
        }
        let combined = 0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00);
        return char::from_u32(combined).map_or(Escape::Invalid(next + 6), |character| {
            Escape::Char(character, next + 6)
        });
    }
    char::from_u32(first).map_or(Escape::Invalid(next), |character| {
        Escape::Char(character, next)
    })
}

fn hex4(text: &str, at: usize) -> Option<u32> {
    let digits = text.get(at..at + 4)?;
    if !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(digits, 16).ok()
}

/// Where the cursor stands in a JSON text that is being typed.
#[derive(Debug, Clone, PartialEq)]
pub enum Position {
    /// Nothing has been typed yet, or only spaces.
    Nothing,
    /// Where a key of an object goes.
    Key {
        /// The object the key belongs to.
        object: Pointer,
        /// What has been typed of the key so far, without its opening quote.
        prefix: String,
        /// The part of the text a completed key replaces: the partial key, with its
        /// opening quote when there is one.
        replace: Span,
        /// The keys the object already has before the cursor.
        present: Vec<String>,
    },
    /// Where a value goes.
    Value {
        /// Where the value would be.
        at: Pointer,
        /// What has been typed of the value so far, as written.
        prefix: String,
        /// The part of the text a completed value replaces.
        replace: Span,
    },
    /// Somewhere a completion makes no sense: after a finished value, inside a string
    /// that is not a key, or after something that is not JSON.
    Elsewhere,
}

enum Frame {
    Object {
        at: Pointer,
        expect: ObjectExpect,
        key: Option<String>,
        present: Vec<String>,
    },
    Array {
        at: Pointer,
        index: usize,
        expect: ArrayExpect,
    },
}

#[derive(Clone, Copy, PartialEq)]
enum ObjectExpect {
    KeyOrEnd,
    Colon,
    Value,
    CommaOrEnd,
}

#[derive(Clone, Copy, PartialEq)]
enum ArrayExpect {
    ValueOrEnd,
    CommaOrEnd,
}

struct Scanner {
    stack: Vec<Frame>,
    root_done: bool,
}

impl Scanner {
    /// Where a value would go now, if a value can go here.
    fn value_path(&self) -> Option<Pointer> {
        match self.stack.last() {
            None => (!self.root_done).then(Pointer::root),
            Some(Frame::Object {
                at,
                expect: ObjectExpect::Value,
                key: Some(key),
                ..
            }) => Some(at.key(key)),
            Some(Frame::Array {
                at,
                index,
                expect: ArrayExpect::ValueOrEnd,
            }) => Some(at.index(*index)),
            Some(_) => None,
        }
    }

    fn at_key_position(&self) -> bool {
        matches!(
            self.stack.last(),
            Some(Frame::Object {
                expect: ObjectExpect::KeyOrEnd,
                ..
            })
        )
    }

    fn value_done(&mut self) {
        match self.stack.last_mut() {
            None => self.root_done = true,
            Some(Frame::Object { expect, .. }) => *expect = ObjectExpect::CommaOrEnd,
            Some(Frame::Array { expect, .. }) => *expect = ArrayExpect::CommaOrEnd,
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "a scanner reads best as one loop over one state, with every transition in view"
)]
/// Where `cursor`, a byte offset, stands in `text`. Only the text before the cursor is
/// read, so what follows it never changes the answer.
#[must_use]
pub fn position_at(text: &str, cursor: usize) -> Position {
    let mut cursor = cursor.min(text.len());
    while !text.is_char_boundary(cursor) {
        cursor -= 1;
    }
    let before = &text[..cursor];
    let bytes = before.as_bytes();
    let mut scanner = Scanner {
        stack: Vec::new(),
        root_done: false,
    };
    let mut at = 0;
    while at < bytes.len() {
        if scanner.stack.len() > MAX_DEPTH {
            return Position::Elsewhere;
        }
        match bytes[at] {
            b' ' | b'\t' | b'\n' | b'\r' => at += 1,
            b'{' | b'[' => {
                let Some(path) = scanner.value_path() else {
                    return Position::Elsewhere;
                };
                scanner.stack.push(if bytes[at] == b'{' {
                    Frame::Object {
                        at: path,
                        expect: ObjectExpect::KeyOrEnd,
                        key: None,
                        present: Vec::new(),
                    }
                } else {
                    Frame::Array {
                        at: path,
                        index: 0,
                        expect: ArrayExpect::ValueOrEnd,
                    }
                });
                at += 1;
            }
            b'}' => {
                let closes = matches!(
                    scanner.stack.last(),
                    Some(Frame::Object {
                        expect: ObjectExpect::KeyOrEnd | ObjectExpect::CommaOrEnd,
                        ..
                    })
                );
                if !closes {
                    return Position::Elsewhere;
                }
                scanner.stack.pop();
                scanner.value_done();
                at += 1;
            }
            b']' => {
                if !matches!(scanner.stack.last(), Some(Frame::Array { .. })) {
                    return Position::Elsewhere;
                }
                scanner.stack.pop();
                scanner.value_done();
                at += 1;
            }
            b':' => {
                let Some(Frame::Object { expect, .. }) = scanner.stack.last_mut() else {
                    return Position::Elsewhere;
                };
                if *expect != ObjectExpect::Colon {
                    return Position::Elsewhere;
                }
                *expect = ObjectExpect::Value;
                at += 1;
            }
            b',' => {
                match scanner.stack.last_mut() {
                    Some(Frame::Object { expect, key, .. })
                        if *expect == ObjectExpect::CommaOrEnd =>
                    {
                        *expect = ObjectExpect::KeyOrEnd;
                        *key = None;
                    }
                    Some(Frame::Array { expect, index, .. })
                        if *expect == ArrayExpect::CommaOrEnd =>
                    {
                        *expect = ArrayExpect::ValueOrEnd;
                        *index += 1;
                    }
                    _ => return Position::Elsewhere,
                }
                at += 1;
            }
            b'"' => {
                let start = at;
                let (decoded, end, terminated) = read_string(before, at + 1);
                if !terminated {
                    let replace = Span { start, end: cursor };
                    if scanner.at_key_position() {
                        let Some(Frame::Object { at, present, .. }) = scanner.stack.last() else {
                            return Position::Elsewhere;
                        };
                        return Position::Key {
                            object: at.clone(),
                            prefix: decoded.unwrap_or_default(),
                            replace,
                            present: present.clone(),
                        };
                    }
                    return match scanner.value_path() {
                        Some(path) => Position::Value {
                            at: path,
                            prefix: before[start..].to_owned(),
                            replace,
                        },
                        None => Position::Elsewhere,
                    };
                }
                if scanner.at_key_position() {
                    let Some(Frame::Object {
                        expect,
                        key,
                        present,
                        ..
                    }) = scanner.stack.last_mut()
                    else {
                        return Position::Elsewhere;
                    };
                    let name = decoded.unwrap_or_default();
                    present.push(name.clone());
                    *key = Some(name);
                    *expect = ObjectExpect::Colon;
                } else if scanner.value_path().is_some() {
                    scanner.value_done();
                } else {
                    return Position::Elsewhere;
                }
                at = end;
            }
            _ => {
                let start = at;
                while at < bytes.len()
                    && !matches!(
                        bytes[at],
                        b' ' | b'\t'
                            | b'\n'
                            | b'\r'
                            | b'{'
                            | b'}'
                            | b'['
                            | b']'
                            | b':'
                            | b','
                            | b'"'
                    )
                {
                    at += 1;
                }
                let token = &before[start..at];
                if at == bytes.len() {
                    let replace = Span { start, end: cursor };
                    if scanner.at_key_position() {
                        let Some(Frame::Object {
                            at: object,
                            present,
                            ..
                        }) = scanner.stack.last()
                        else {
                            return Position::Elsewhere;
                        };
                        return Position::Key {
                            object: object.clone(),
                            prefix: token.to_owned(),
                            replace,
                            present: present.clone(),
                        };
                    }
                    return match scanner.value_path() {
                        Some(path) => Position::Value {
                            at: path,
                            prefix: token.to_owned(),
                            replace,
                        },
                        None => Position::Elsewhere,
                    };
                }
                if scanner.value_path().is_some() {
                    scanner.value_done();
                } else {
                    return Position::Elsewhere;
                }
            }
        }
    }
    let here = Span::empty(cursor);
    if scanner.stack.is_empty() {
        return if scanner.root_done {
            Position::Elsewhere
        } else {
            Position::Nothing
        };
    }
    if scanner.at_key_position()
        && let Some(Frame::Object { at, present, .. }) = scanner.stack.last()
    {
        return Position::Key {
            object: at.clone(),
            prefix: String::new(),
            replace: here,
            present: present.clone(),
        };
    }
    match scanner.value_path() {
        Some(path) => Position::Value {
            at: path,
            prefix: String::new(),
            replace: here,
        },
        None => Position::Elsewhere,
    }
}

/// The byte offset of the position `serde_json` reports, which counts lines and columns
/// from one. The column counts bytes, and the result is moved back to the start of the
/// character it falls in.
#[must_use]
pub fn offset_of(text: &str, line: usize, column: usize) -> usize {
    let mut offset = 0;
    for (number, content) in text.split_inclusive('\n').enumerate() {
        if number + 1 == line {
            let mut at = (offset + column.saturating_sub(1)).min(text.len());
            while !text.is_char_boundary(at) {
                at -= 1;
            }
            return at;
        }
        offset += content.len();
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slice(text: &str, span: Span) -> &str {
        &text[span.start..span.end]
    }

    #[test]
    fn the_span_tree_finds_values_and_keys_by_pointer() {
        let text = r#" {"a": 1, "b": {"c": [true, "x\"y"]}} "#;
        let tree = spans(text).unwrap();
        let pointer = Pointer::root().key("b").key("c").index(1);
        assert_eq!(
            slice(text, tree.find(&pointer).unwrap().span()),
            r#""x\"y""#
        );
        let member = tree
            .find(&Pointer::root().key("b"))
            .unwrap()
            .member("c")
            .unwrap();
        assert_eq!(slice(text, member.key_span), r#""c""#);
        assert_eq!(
            slice(text, tree.span()),
            r#"{"a": 1, "b": {"c": [true, "x\"y"]}}"#
        );
    }

    #[test]
    fn a_repeated_key_is_found_where_serde_json_keeps_it() {
        let text = r#"{"a": 1, "a": 2}"#;
        let tree = spans(text).unwrap();
        assert_eq!(
            slice(text, tree.find(&Pointer::root().key("a")).unwrap().span()),
            "2"
        );
    }

    #[test]
    fn keys_are_compared_after_their_escapes_are_decoded() {
        let text = r#"{"café": 1, "😀": 2}"#;
        let tree = spans(text).unwrap();
        assert!(tree.member("café").is_some());
        assert!(tree.member("😀").is_some());
    }

    #[test]
    fn text_that_is_not_json_has_no_span_tree() {
        for text in [
            "",
            "{",
            r#"{"a"}"#,
            "[1,]",
            "01",
            "1.",
            "tru",
            r#"{"a":1} x"#,
            "\"\u{1}\"",
        ] {
            assert_eq!(spans(text), None, "{text:?}");
        }
    }

    #[test]
    fn nesting_beyond_the_limit_is_refused_rather_than_overflowing() {
        let deep = "[".repeat(10_000) + &"]".repeat(10_000);
        assert_eq!(spans(&deep), None);
        assert_eq!(position_at(&deep, deep.len()), Position::Elsewhere);
    }

    #[test]
    fn an_empty_line_is_nothing_yet() {
        assert_eq!(position_at("", 0), Position::Nothing);
        assert_eq!(position_at("   ", 3), Position::Nothing);
    }

    #[test]
    fn after_an_opening_brace_a_key_goes() {
        let text = "{";
        assert_eq!(
            position_at(text, 1),
            Position::Key {
                object: Pointer::root(),
                prefix: String::new(),
                replace: Span::empty(1),
                present: vec![],
            }
        );
    }

    #[test]
    fn a_partial_key_is_found_with_its_quote_and_the_keys_before_it() {
        let text = r#"{"api": "x", "mes"#;
        let Position::Key {
            object,
            prefix,
            replace,
            present,
        } = position_at(text, text.len())
        else {
            panic!("not a key position");
        };
        assert_eq!(object, Pointer::root());
        assert_eq!(prefix, "mes");
        assert_eq!(slice(text, replace), "\"mes");
        assert_eq!(present, vec!["api".to_owned()]);
    }

    #[test]
    fn a_key_typed_without_quotes_is_still_a_key() {
        let text = "{mes";
        let Position::Key {
            prefix, replace, ..
        } = position_at(text, 4)
        else {
            panic!("not a key position");
        };
        assert_eq!(prefix, "mes");
        assert_eq!(slice(text, replace), "mes");
    }

    #[test]
    fn after_a_colon_a_value_goes_at_the_member_s_path() {
        let text = r#"{"outer": {"inner": "#;
        assert_eq!(
            position_at(text, text.len()),
            Position::Value {
                at: Pointer::root().key("outer").key("inner"),
                prefix: String::new(),
                replace: Span::empty(text.len()),
            }
        );
    }

    #[test]
    fn a_partial_value_is_found_as_typed() {
        let text = r#"{"flag": tr"#;
        let Position::Value {
            at,
            prefix,
            replace,
        } = position_at(text, text.len())
        else {
            panic!("not a value position");
        };
        assert_eq!(at, Pointer::root().key("flag"));
        assert_eq!(prefix, "tr");
        assert_eq!(slice(text, replace), "tr");
    }

    #[test]
    fn elements_of_an_array_are_found_by_position() {
        let text = r#"{"tools": ["a", "#;
        assert_eq!(
            position_at(text, text.len()),
            Position::Value {
                at: Pointer::root().key("tools").index(1),
                prefix: String::new(),
                replace: Span::empty(text.len()),
            }
        );
    }

    #[test]
    fn after_a_finished_value_or_inside_a_value_string_nothing_is_completed() {
        assert_eq!(position_at(r#"{"a": 1 "#, 8), Position::Elsewhere);
        assert_eq!(position_at(r#"{"a": "x" "#, 10), Position::Elsewhere);
        assert_eq!(position_at("{}", 2), Position::Elsewhere);
        assert_eq!(position_at(r#"{"a" "#, 5), Position::Elsewhere);
        assert_eq!(position_at("}", 1), Position::Elsewhere);
    }

    #[test]
    fn only_the_text_before_the_cursor_counts() {
        let text = r#"{"a": 1, "b": 2}"#;
        let Position::Key { present, .. } = position_at(text, 9) else {
            panic!("not a key position");
        };
        assert_eq!(present, vec!["a".to_owned()]);
    }

    #[test]
    fn a_cursor_inside_a_character_is_moved_back_to_its_start() {
        let text = "{\"é";
        assert!(matches!(position_at(text, 3), Position::Key { .. }));
    }

    #[test]
    fn serde_json_positions_become_byte_offsets() {
        let text = r#"{"a": x}"#;
        let error = serde_json::from_str::<serde_json::Value>(text).unwrap_err();
        let offset = offset_of(text, error.line(), error.column());
        assert_eq!(&text[offset..=offset], "x");
    }
}
