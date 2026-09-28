//! A ring buffer with two limits, one in entries and one in bytes, that counts what it
//! drops.

use std::collections::VecDeque;

/// Something a ring buffer can hold, and knows the size of.
pub trait Measured {
    /// The bytes it holds, including a fixed amount for its bookkeeping.
    fn bytes(&self) -> usize;
}

/// The newest entries, as many as both limits allow.
///
/// Every entry has a number, counted from zero across the whole life of the buffer, so
/// that something outside the buffer can refer to an entry and find out later that it has
/// been dropped.
#[derive(Debug, Clone)]
pub struct Ring<T> {
    entries: VecDeque<T>,
    first: u64,
    bytes: usize,
    max_entries: usize,
    max_bytes: usize,
    dropped: u64,
    dropped_bytes: u64,
}

impl<T: Measured> Ring<T> {
    /// An empty buffer with these limits. Each limit is at least one.
    #[must_use]
    pub fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            first: 0,
            bytes: 0,
            max_entries: max_entries.max(1),
            max_bytes: max_bytes.max(1),
            dropped: 0,
            dropped_bytes: 0,
        }
    }

    /// Adds an entry at the end, drops from the front until both limits hold, and returns
    /// the entry's number. An entry larger than the byte limit on its own is still kept,
    /// alone, because a buffer that could not show the latest thing would be worse.
    pub fn push(&mut self, entry: T) -> u64 {
        self.bytes += entry.bytes();
        self.entries.push_back(entry);
        while self.entries.len() > 1
            && (self.entries.len() > self.max_entries || self.bytes > self.max_bytes)
        {
            self.drop_front();
        }
        self.first + self.entries.len() as u64 - 1
    }

    fn drop_front(&mut self) {
        if let Some(entry) = self.entries.pop_front() {
            let size = entry.bytes();
            self.bytes -= size;
            self.first += 1;
            self.dropped += 1;
            self.dropped_bytes += size as u64;
        }
    }

    /// The entry with this number, if it is still held.
    #[must_use]
    pub fn get(&self, number: u64) -> Option<&T> {
        let offset = usize::try_from(number.checked_sub(self.first)?).ok()?;
        self.entries.get(offset)
    }

    /// The entry with this number, to change, if it is still held. Its size is counted
    /// again when the change is done, through [`Ring::resized`].
    pub fn get_mut(&mut self, number: u64) -> Option<&mut T> {
        let offset = usize::try_from(number.checked_sub(self.first)?).ok()?;
        self.entries.get_mut(offset)
    }

    /// Counts an entry's size again after it changed by `before` bytes to its current
    /// size, and drops from the front if a limit no longer holds.
    pub fn resized(&mut self, number: u64, before: usize) {
        let Some(after) = self.get(number).map(Measured::bytes) else {
            return;
        };
        self.bytes = self.bytes.saturating_sub(before) + after;
        while self.entries.len() > 1 && self.bytes > self.max_bytes {
            self.drop_front();
        }
    }

    /// The entries held, oldest first, with their numbers.
    #[must_use]
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (u64, &T)> + ExactSizeIterator {
        let first = self.first;
        self.entries
            .iter()
            .enumerate()
            .map(move |(offset, entry)| (first + offset as u64, entry))
    }

    /// How many entries are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The number of the oldest entry held, or of the next one when nothing is.
    #[must_use]
    pub fn first_number(&self) -> u64 {
        self.first
    }

    /// The number the next entry will get.
    #[must_use]
    pub fn next_number(&self) -> u64 {
        self.first + self.entries.len() as u64
    }

    /// The bytes held.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// How many entries have been dropped to keep the limits, since the start.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// How many bytes those entries held.
    #[must_use]
    pub fn dropped_bytes(&self) -> u64 {
        self.dropped_bytes
    }

    /// The limit in entries.
    #[must_use]
    pub fn max_entries(&self) -> usize {
        self.max_entries
    }

    /// The limit in bytes.
    #[must_use]
    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    /// Empties the buffer. What is cleared by a person is not counted as dropped.
    pub fn clear(&mut self) {
        self.first += self.entries.len() as u64;
        self.entries.clear();
        self.bytes = 0;
    }

    /// The newest entry, when there is one.
    #[must_use]
    pub fn last(&self) -> Option<(u64, &T)> {
        self.iter().next_back()
    }
}

impl Measured for String {
    fn bytes(&self) -> usize {
        self.len() + crate::limits::ENTRY_OVERHEAD
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(length: usize) -> String {
        "x".repeat(length)
    }

    #[test]
    fn the_limit_in_entries_drops_the_oldest_and_counts_them() {
        let mut ring = Ring::new(3, usize::MAX);
        for n in 0..5 {
            ring.push(n.to_string());
        }
        let held: Vec<&str> = ring.iter().map(|(_, entry)| entry.as_str()).collect();
        assert_eq!(held, vec!["2", "3", "4"]);
        assert_eq!(ring.dropped(), 2);
        assert_eq!(ring.first_number(), 2);
    }

    #[test]
    fn the_limit_in_bytes_drops_as_many_as_it_takes() {
        let overhead = crate::limits::ENTRY_OVERHEAD;
        let mut ring = Ring::new(100, 3 * (10 + overhead));
        for _ in 0..3 {
            ring.push(text(10));
        }
        assert_eq!(ring.dropped(), 0);
        ring.push(text(25));
        assert_eq!(ring.len(), 2);
        assert_eq!(ring.dropped(), 2);
        assert!(ring.bytes() <= ring.max_bytes());
    }

    #[test]
    fn an_entry_larger_than_the_limit_is_kept_alone() {
        let mut ring = Ring::new(10, 100);
        ring.push(text(10));
        ring.push(text(1_000));
        assert_eq!(ring.len(), 1);
        assert_eq!(ring.last().unwrap().1.len(), 1_000);
    }

    #[test]
    fn numbers_survive_dropping_and_a_dropped_number_finds_nothing() {
        let mut ring = Ring::new(2, usize::MAX);
        let a = ring.push("a".to_owned());
        let b = ring.push("b".to_owned());
        let c = ring.push("c".to_owned());
        assert_eq!((a, b, c), (0, 1, 2));
        assert_eq!(ring.get(a), None);
        assert_eq!(ring.get(c).map(String::as_str), Some("c"));
        assert_eq!(ring.next_number(), 3);
    }

    #[test]
    fn a_resized_entry_is_counted_again_and_can_push_others_out() {
        let overhead = crate::limits::ENTRY_OVERHEAD;
        let mut ring = Ring::new(10, 2 * (5 + overhead) + 20);
        ring.push(text(5));
        let second = ring.push(text(5));
        let before = ring.get(second).unwrap().bytes();
        ring.get_mut(second).unwrap().push_str(&text(40));
        ring.resized(second, before);
        assert_eq!(ring.len(), 1);
        assert_eq!(ring.dropped(), 1);
        assert_eq!(ring.bytes(), 45 + overhead);
    }

    #[test]
    fn clearing_is_not_dropping() {
        let mut ring = Ring::new(10, usize::MAX);
        ring.push("a".to_owned());
        ring.clear();
        assert!(ring.is_empty());
        assert_eq!(ring.dropped(), 0);
        assert_eq!(ring.push("b".to_owned()), 1);
    }

    #[test]
    fn a_million_pushes_never_hold_more_than_the_limits() {
        let mut ring = Ring::new(1_000, 64 * 1024);
        for n in 0..1_000_000_u32 {
            ring.push(text((n % 97) as usize));
            assert!(ring.len() <= 1_000);
            assert!(ring.bytes() <= 64 * 1024);
        }
        assert_eq!(ring.dropped() + ring.len() as u64, 1_000_000);
    }

    #[test]
    fn a_ring_says_whether_it_holds_anything_and_what_its_limit_in_entries_is() {
        let mut ring = Ring::new(7, usize::MAX);
        assert!(ring.is_empty());
        ring.push(text(1));
        assert!(!ring.is_empty());
        assert_eq!(ring.max_entries(), 7);
    }

    #[test]
    fn an_entry_that_grows_is_counted_again_and_the_oldest_go_only_past_the_limit() {
        let overhead = crate::limits::ENTRY_OVERHEAD;
        let mut ring = Ring::new(10, 2 * overhead + 30);
        let first = ring.push(text(10));
        let second = ring.push(text(10));
        // Grown to exactly the limit: both are kept.
        let before = ring.get(second).map(Measured::bytes).unwrap();
        if let Some(entry) = ring.get_mut(second) {
            entry.push_str(&text(10));
        }
        ring.resized(second, before);
        assert_eq!(ring.bytes(), 2 * overhead + 30);
        assert_eq!(ring.len(), 2);
        assert!(ring.get(first).is_some());
        // One byte more, and the oldest goes.
        let before = ring.get(second).map(Measured::bytes).unwrap();
        if let Some(entry) = ring.get_mut(second) {
            entry.push('x');
        }
        ring.resized(second, before);
        assert_eq!(ring.len(), 1);
        assert!(ring.get(first).is_none());
        // Alone, it is kept however large it grows.
        let before = ring.get(second).map(Measured::bytes).unwrap();
        if let Some(entry) = ring.get_mut(second) {
            entry.push_str(&text(1_000));
        }
        ring.resized(second, before);
        assert_eq!(ring.len(), 1);
    }
}
