use ropey::Rope;

use crate::transaction::Transaction;

/// A single revision in the undo history.
struct Revision {
    /// The transaction that was applied.
    transaction: Transaction,
    /// The inverse transaction (for undo).
    inverse: Transaction,
    /// Index of the parent revision (for branching undo).
    parent: usize,
}

/// Undo/redo history with branching support.
pub struct History {
    revisions: Vec<Revision>,
    /// Index into revisions representing current state.
    /// 0 means "at the initial state" (before any revisions).
    /// Values 1..=revisions.len() mean "after applying revision at index (current - 1)".
    current: usize,
}

impl History {
    pub fn new() -> Self {
        Self {
            revisions: Vec::new(),
            current: 0,
        }
    }

    /// Record a new revision. `original` is the rope BEFORE the transaction was applied.
    pub fn commit(&mut self, transaction: Transaction, original: &Rope) {
        let inverse = transaction.invert(original);
        self.commit_with_inverse(transaction, inverse);
    }

    /// Record a new revision with a pre-computed inverse.
    /// Use this when the inverse must be computed before applying the transaction
    /// to avoid history/buffer divergence on apply failure.
    pub fn commit_with_inverse(&mut self, transaction: Transaction, inverse: Transaction) {
        let revision = Revision {
            transaction,
            inverse,
            parent: self.current,
        };
        self.revisions.push(revision);
        self.current = self.revisions.len();
    }

    /// Undo the current revision. Returns an owned Transaction to apply (the inverse).
    /// Returns None if already at initial state.
    pub fn undo(&mut self) -> Option<Transaction> {
        if self.current == 0 {
            return None;
        }
        let rev_idx = self.current - 1;
        let inverse = self.revisions[rev_idx].inverse.clone();
        self.current = self.revisions[rev_idx].parent;
        Some(inverse)
    }

    /// Redo: find a revision whose parent is `current` and re-apply it.
    /// Returns the most recently added revision with that parent.
    pub fn redo(&mut self) -> Option<Transaction> {
        let child_idx = self
            .revisions
            .iter()
            .enumerate()
            .rev()
            .find(|(_, rev)| rev.parent == self.current)
            .map(|(idx, _)| idx);

        if let Some(idx) = child_idx {
            let txn = self.revisions[idx].transaction.clone();
            self.current = idx + 1;
            Some(txn)
        } else {
            None
        }
    }

    /// Current revision index (for tracking saved state).
    pub fn current_revision(&self) -> usize {
        self.current
    }

    /// Check if current state matches a saved revision.
    pub fn is_at_saved(&self, saved_revision: usize) -> bool {
        self.current == saved_revision
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_history_undo() {
        let mut history = History::new();
        assert!(history.undo().is_none());
    }

    #[test]
    fn test_empty_history_redo() {
        let mut history = History::new();
        assert!(history.redo().is_none());
    }

    #[test]
    fn test_commit_and_undo() {
        let mut history = History::new();
        let rope = Rope::from_str("hello");

        let txn = Transaction::insert(" world", 5, rope.len_bytes());
        history.commit(txn, &rope);

        assert_eq!(history.current_revision(), 1);

        let inverse = history.undo().unwrap();
        assert_eq!(history.current_revision(), 0);

        // Apply inverse to verify it restores original
        let mut modified = Rope::from_str("hello world");
        inverse.apply(&mut modified).unwrap();
        assert_eq!(modified.to_string(), "hello");
    }

    #[test]
    fn test_undo_redo_cycle() {
        let mut history = History::new();
        let rope = Rope::from_str("hello");

        let txn = Transaction::insert("!", 5, rope.len_bytes());
        history.commit(txn, &rope);

        history.undo().unwrap();
        assert_eq!(history.current_revision(), 0);

        let redo_txn = history.redo().unwrap();
        assert_eq!(history.current_revision(), 1);

        let mut test_rope = Rope::from_str("hello");
        redo_txn.apply(&mut test_rope).unwrap();
        assert_eq!(test_rope.to_string(), "hello!");
    }

    #[test]
    fn test_branching_undo() {
        let mut history = History::new();
        let rope = Rope::from_str("abc");

        // First edit: abc -> abcd
        let txn1 = Transaction::insert("d", 3, 3);
        history.commit(txn1, &rope);
        assert_eq!(history.current_revision(), 1);

        // Undo back to abc
        history.undo().unwrap();
        assert_eq!(history.current_revision(), 0);

        // New edit from initial state: abc -> abcx (branch)
        let txn2 = Transaction::insert("x", 3, 3);
        history.commit(txn2, &rope);
        assert_eq!(history.current_revision(), 2);

        // Both revisions are preserved; redo finds the most recent child
        history.undo().unwrap();
        let redo = history.redo().unwrap();
        let mut test = Rope::from_str("abc");
        redo.apply(&mut test).unwrap();
        assert_eq!(test.to_string(), "abcx");
    }

    #[test]
    fn test_is_at_saved() {
        let mut history = History::new();
        let rope = Rope::from_str("hello");

        let saved = history.current_revision();
        assert!(history.is_at_saved(saved));

        let txn = Transaction::insert("!", 5, rope.len_bytes());
        history.commit(txn, &rope);

        assert!(!history.is_at_saved(saved));
        assert!(history.is_at_saved(1));
    }
}
