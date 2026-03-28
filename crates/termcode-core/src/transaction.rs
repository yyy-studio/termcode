use ropey::Rope;

use crate::selection::Selection;

/// A single edit operation within a ChangeSet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    /// Skip n bytes unchanged.
    Retain(usize),
    /// Insert string at current position.
    Insert(String),
    /// Delete n bytes from current position.
    Delete(usize),
}

/// A set of sequential edit operations against a document of known length.
#[derive(Debug, Clone)]
pub struct ChangeSet {
    ops: Vec<Operation>,
    input_len: usize,
}

/// Find the nearest char boundary in `s` at or before byte offset `n`.
/// If `n` is already on a char boundary, returns `n`.
/// Otherwise rounds down to the start of the character containing byte `n`.
fn find_char_boundary(s: &str, n: usize) -> usize {
    if n >= s.len() {
        return s.len();
    }
    // str::floor_char_boundary is nightly-only, so we manually scan backwards.
    let mut pos = n;
    while pos > 0 && !s.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

impl ChangeSet {
    pub fn new(input_len: usize) -> Self {
        Self {
            ops: Vec::new(),
            input_len,
        }
    }

    pub fn insert(&mut self, text: String) {
        self.ops.push(Operation::Insert(text));
    }

    pub fn delete(&mut self, count: usize) {
        self.ops.push(Operation::Delete(count));
    }

    pub fn retain(&mut self, count: usize) {
        self.ops.push(Operation::Retain(count));
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn input_len(&self) -> usize {
        self.input_len
    }

    /// Apply this changeset to a rope, mutating it in place.
    pub fn apply(&self, rope: &mut Rope) -> anyhow::Result<()> {
        let mut cursor: usize = 0;
        for op in &self.ops {
            match op {
                Operation::Retain(n) => {
                    cursor += n;
                }
                Operation::Insert(text) => {
                    let char_idx = rope.byte_to_char(cursor);
                    rope.insert(char_idx, text);
                    cursor += text.len();
                }
                Operation::Delete(n) => {
                    let start_char = rope.byte_to_char(cursor);
                    let end_char = rope.byte_to_char(cursor + n);
                    rope.remove(start_char..end_char);
                }
            }
        }
        Ok(())
    }

    /// Produce the inverse changeset that undoes this one.
    /// `original` must be the rope BEFORE this changeset was applied.
    pub fn invert(&self, original: &Rope) -> ChangeSet {
        let mut inverse = ChangeSet::new(self.output_len());
        let mut cursor: usize = 0;
        for op in &self.ops {
            match op {
                Operation::Retain(n) => {
                    inverse.retain(*n);
                    cursor += n;
                }
                Operation::Insert(text) => {
                    inverse.delete(text.len());
                }
                Operation::Delete(n) => {
                    let start_char = original.byte_to_char(cursor);
                    let end_char = original.byte_to_char(cursor + n);
                    let deleted: String = original.slice(start_char..end_char).into();
                    inverse.insert(deleted);
                    cursor += n;
                }
            }
        }
        inverse
    }

    /// Compute the output length after applying this changeset.
    fn output_len(&self) -> usize {
        let mut len = self.input_len;
        for op in &self.ops {
            match op {
                Operation::Retain(_) => {}
                Operation::Insert(text) => len += text.len(),
                Operation::Delete(n) => len -= n,
            }
        }
        len
    }

    /// Compose two sequential changesets into one.
    ///
    /// When splitting an Insert's text at a byte offset, `find_char_boundary`
    /// ensures the split does not land in the middle of a multi-byte UTF-8 char.
    pub fn compose(self, other: ChangeSet) -> ChangeSet {
        let mut composed = ChangeSet::new(self.input_len);

        let mut a_iter = self.ops.into_iter().peekable();
        let mut b_iter = other.ops.into_iter().peekable();

        let mut a_cur: Option<Operation> = a_iter.next();
        let mut b_cur: Option<Operation> = b_iter.next();

        loop {
            match (&a_cur, &b_cur) {
                (None, None) => break,
                (Some(Operation::Insert(text)), _) => {
                    // Insert from A is present in A's output; B may Retain or Delete over it
                    match &b_cur {
                        Some(Operation::Retain(n)) => {
                            let text_len = text.len();
                            if text_len <= *n {
                                composed.insert(text.clone());
                                let remaining = *n - text_len;
                                a_cur = a_iter.next();
                                b_cur = if remaining > 0 {
                                    Some(Operation::Retain(remaining))
                                } else {
                                    b_iter.next()
                                };
                            } else {
                                // text is longer than retain; split text at char boundary
                                let split = find_char_boundary(text, *n);
                                composed.insert(text[..split].to_string());
                                a_cur = Some(Operation::Insert(text[split..].to_string()));
                                b_cur = b_iter.next();
                            }
                        }
                        Some(Operation::Delete(n)) => {
                            let text_len = text.len();
                            if text_len <= *n {
                                // B deletes A's insertion entirely (or more)
                                let remaining = *n - text_len;
                                a_cur = a_iter.next();
                                b_cur = if remaining > 0 {
                                    Some(Operation::Delete(remaining))
                                } else {
                                    b_iter.next()
                                };
                            } else {
                                // B deletes part of A's insertion
                                let split = find_char_boundary(text, *n);
                                a_cur = Some(Operation::Insert(text[split..].to_string()));
                                b_cur = b_iter.next();
                            }
                        }
                        Some(Operation::Insert(b_text)) => {
                            // Both A and B insert at the same position.
                            // B operates on A's output, so B's insert comes first
                            // to satisfy: apply(compose(A,B), doc) == apply(B, apply(A, doc))
                            composed.insert(b_text.clone());
                            b_cur = b_iter.next();
                        }
                        None => {
                            composed.insert(text.clone());
                            a_cur = a_iter.next();
                        }
                    }
                }
                (_, Some(Operation::Insert(text))) => {
                    composed.insert(text.clone());
                    b_cur = b_iter.next();
                }
                (Some(Operation::Retain(a_n)), Some(Operation::Retain(b_n))) => {
                    let min = (*a_n).min(*b_n);
                    composed.retain(min);
                    a_cur = if *a_n > min {
                        Some(Operation::Retain(*a_n - min))
                    } else {
                        a_iter.next()
                    };
                    b_cur = if *b_n > min {
                        Some(Operation::Retain(*b_n - min))
                    } else {
                        b_iter.next()
                    };
                }
                (Some(Operation::Retain(a_n)), Some(Operation::Delete(b_n))) => {
                    let min = (*a_n).min(*b_n);
                    composed.delete(min);
                    a_cur = if *a_n > min {
                        Some(Operation::Retain(*a_n - min))
                    } else {
                        a_iter.next()
                    };
                    b_cur = if *b_n > min {
                        Some(Operation::Delete(*b_n - min))
                    } else {
                        b_iter.next()
                    };
                }
                (Some(Operation::Delete(n)), _) => {
                    composed.delete(*n);
                    a_cur = a_iter.next();
                }
                (Some(Operation::Retain(n)), None) => {
                    composed.retain(*n);
                    a_cur = a_iter.next();
                }
                (None, Some(op)) => {
                    composed.ops.push(op.clone());
                    b_cur = b_iter.next();
                }
            }
        }

        composed
    }

    /// Map a byte position through this changeset.
    /// Returns the new position after the edits have been applied.
    /// Positions at an insert point are pushed after the insertion.
    pub fn map_position(&self, pos: usize) -> usize {
        let mut old_cursor: usize = 0;
        let mut new_cursor: usize = 0;

        for op in &self.ops {
            match op {
                Operation::Retain(n) => {
                    if old_cursor + n > pos {
                        let advance = pos - old_cursor;
                        new_cursor += advance;
                        return new_cursor;
                    }
                    old_cursor += n;
                    new_cursor += n;
                }
                Operation::Insert(text) => {
                    new_cursor += text.len();
                }
                Operation::Delete(n) => {
                    if old_cursor + n > pos {
                        // Position is within the deleted range; map to start of deletion
                        return new_cursor;
                    }
                    old_cursor += n;
                }
            }
        }

        new_cursor + (pos - old_cursor)
    }
}

/// An atomic editing transaction combining a changeset with an optional selection update.
#[derive(Debug, Clone)]
pub struct Transaction {
    pub changes: ChangeSet,
    pub selection: Option<Selection>,
}

impl Transaction {
    pub fn new(changes: ChangeSet) -> Self {
        Self {
            changes,
            selection: None,
        }
    }

    pub fn with_selection(mut self, selection: Selection) -> Self {
        self.selection = Some(selection);
        self
    }

    /// Convenience: insert text at a position in a document of given length.
    pub fn insert(text: &str, pos: usize, doc_len: usize) -> Self {
        let mut cs = ChangeSet::new(doc_len);
        if pos > 0 {
            cs.retain(pos);
        }
        cs.insert(text.to_string());
        let remaining = doc_len - pos;
        if remaining > 0 {
            cs.retain(remaining);
        }
        Self::new(cs)
    }

    /// Convenience: delete a byte range in a document of given length.
    pub fn delete(range: std::ops::Range<usize>, doc_len: usize) -> Self {
        let mut cs = ChangeSet::new(doc_len);
        if range.start > 0 {
            cs.retain(range.start);
        }
        cs.delete(range.len());
        let remaining = doc_len - range.end;
        if remaining > 0 {
            cs.retain(remaining);
        }
        Self::new(cs)
    }

    /// Convenience: replace a byte range with new text.
    pub fn replace(range: std::ops::Range<usize>, text: &str, doc_len: usize) -> Self {
        let mut cs = ChangeSet::new(doc_len);
        if range.start > 0 {
            cs.retain(range.start);
        }
        cs.delete(range.len());
        cs.insert(text.to_string());
        let remaining = doc_len - range.end;
        if remaining > 0 {
            cs.retain(remaining);
        }
        Self::new(cs)
    }

    /// Apply this transaction to a rope.
    pub fn apply(&self, rope: &mut Rope) -> anyhow::Result<()> {
        self.changes.apply(rope)
    }

    /// Produce the inverse transaction for undo.
    pub fn invert(&self, original: &Rope) -> Transaction {
        Transaction::new(self.changes.invert(original))
    }

    /// Compose two transactions sequentially.
    pub fn compose(self, other: Transaction) -> Transaction {
        Transaction {
            changes: self.changes.compose(other.changes),
            selection: other.selection.or(self.selection),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_transaction() {
        let mut rope = Rope::from_str("hello world");
        let txn = Transaction::insert(" beautiful", 5, rope.len_bytes());
        txn.apply(&mut rope).unwrap();
        assert_eq!(rope.to_string(), "hello beautiful world");
    }

    #[test]
    fn test_delete_transaction() {
        let mut rope = Rope::from_str("hello beautiful world");
        let txn = Transaction::delete(5..15, rope.len_bytes());
        txn.apply(&mut rope).unwrap();
        assert_eq!(rope.to_string(), "hello world");
    }

    #[test]
    fn test_replace_transaction() {
        let mut rope = Rope::from_str("hello world");
        let txn = Transaction::replace(6..11, "rust", rope.len_bytes());
        txn.apply(&mut rope).unwrap();
        assert_eq!(rope.to_string(), "hello rust");
    }

    #[test]
    fn test_invert_insert() {
        let mut rope = Rope::from_str("hello world");
        let original = rope.clone();
        let txn = Transaction::insert("!", 11, rope.len_bytes());
        txn.apply(&mut rope).unwrap();
        assert_eq!(rope.to_string(), "hello world!");

        let inverse = txn.invert(&original);
        inverse.apply(&mut rope).unwrap();
        assert_eq!(rope.to_string(), "hello world");
    }

    #[test]
    fn test_invert_delete() {
        let mut rope = Rope::from_str("hello world");
        let original = rope.clone();
        let txn = Transaction::delete(5..11, rope.len_bytes());
        txn.apply(&mut rope).unwrap();
        assert_eq!(rope.to_string(), "hello");

        let inverse = txn.invert(&original);
        inverse.apply(&mut rope).unwrap();
        assert_eq!(rope.to_string(), "hello world");
    }

    #[test]
    fn test_map_position_after_insert() {
        let cs = {
            let mut cs = ChangeSet::new(11);
            cs.retain(5);
            cs.insert(" beautiful".to_string());
            cs.retain(6);
            cs
        };
        // Position before insert point stays the same
        assert_eq!(cs.map_position(3), 3);
        // Position at insert point shifts right by inserted length
        assert_eq!(cs.map_position(5), 15);
        // Position after insert point shifts right
        assert_eq!(cs.map_position(8), 18);
    }

    #[test]
    fn test_map_position_after_delete() {
        let cs = {
            let mut cs = ChangeSet::new(21);
            cs.retain(5);
            cs.delete(10);
            cs.retain(6);
            cs
        };
        // Position before delete stays the same
        assert_eq!(cs.map_position(3), 3);
        // Position in deleted range maps to start of deletion
        assert_eq!(cs.map_position(10), 5);
        // Position after delete shifts left
        assert_eq!(cs.map_position(18), 8);
    }

    #[test]
    fn test_compose_simple() {
        let mut rope = Rope::from_str("abc");

        let mut cs1 = ChangeSet::new(3);
        cs1.retain(3);
        cs1.insert("d".to_string());

        let mut cs2 = ChangeSet::new(4);
        cs2.retain(4);
        cs2.insert("e".to_string());

        let composed = cs1.compose(cs2);
        composed.apply(&mut rope).unwrap();
        assert_eq!(rope.to_string(), "abcde");
    }

    #[test]
    fn test_changeset_empty() {
        let cs = ChangeSet::new(10);
        assert!(cs.is_empty());
    }

    #[test]
    fn test_insert_at_beginning() {
        let mut rope = Rope::from_str("world");
        let txn = Transaction::insert("hello ", 0, rope.len_bytes());
        txn.apply(&mut rope).unwrap();
        assert_eq!(rope.to_string(), "hello world");
    }

    #[test]
    fn test_delete_at_end() {
        let mut rope = Rope::from_str("hello!");
        let txn = Transaction::delete(5..6, rope.len_bytes());
        txn.apply(&mut rope).unwrap();
        assert_eq!(rope.to_string(), "hello");
    }

    #[test]
    fn test_compose_multibyte_insert_then_retain() {
        // Insert a multi-byte char, then compose with a retain over it
        let mut rope = Rope::from_str("ab");

        // cs1: insert "é" (2 bytes) at position 1
        let mut cs1 = ChangeSet::new(2);
        cs1.retain(1);
        cs1.insert("é".to_string());
        cs1.retain(1);

        // cs2: retain all of cs1's output (4 bytes: "a" + "é" + "b")
        let mut cs2 = ChangeSet::new(4);
        cs2.retain(4);

        let composed = cs1.compose(cs2);
        composed.apply(&mut rope).unwrap();
        assert_eq!(rope.to_string(), "aéb");
    }

    #[test]
    fn test_compose_multibyte_insert_then_delete() {
        // Insert multi-byte text, then delete part of it in the second changeset
        let mut rope = Rope::from_str("ab");

        // cs1: insert "日本" (6 bytes) at position 1
        let mut cs1 = ChangeSet::new(2);
        cs1.retain(1);
        cs1.insert("日本".to_string());
        cs1.retain(1);

        // cs2: retain 1, delete "日" (3 bytes), retain rest
        let mut cs2 = ChangeSet::new(8);
        cs2.retain(1);
        cs2.delete(3);
        cs2.retain(4);

        let composed = cs1.compose(cs2);
        composed.apply(&mut rope).unwrap();
        assert_eq!(rope.to_string(), "a本b");
    }
}
