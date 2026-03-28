/// A byte-offset range in a document, representing a cursor or selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    /// Start of the selection (byte offset).
    pub anchor: usize,
    /// End of the selection / cursor position (byte offset).
    pub head: usize,
}

impl Range {
    pub fn new(anchor: usize, head: usize) -> Self {
        Self { anchor, head }
    }

    /// A point range where anchor == head (cursor with no selection).
    pub fn point(pos: usize) -> Self {
        Self {
            anchor: pos,
            head: pos,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// The smaller of anchor and head.
    pub fn from(&self) -> usize {
        self.anchor.min(self.head)
    }

    /// The larger of anchor and head.
    pub fn to(&self) -> usize {
        self.anchor.max(self.head)
    }

    /// Swap anchor and head.
    pub fn flip(&self) -> Self {
        Self {
            anchor: self.head,
            head: self.anchor,
        }
    }
}

/// A set of ranges representing cursors/selections in a document.
#[derive(Debug, Clone)]
pub struct Selection {
    ranges: Vec<Range>,
    primary: usize,
}

impl Selection {
    /// Single cursor at a byte position.
    pub fn point(pos: usize) -> Self {
        Self {
            ranges: vec![Range::point(pos)],
            primary: 0,
        }
    }

    /// Single range selection.
    pub fn single(anchor: usize, head: usize) -> Self {
        Self {
            ranges: vec![Range::new(anchor, head)],
            primary: 0,
        }
    }

    /// Multi-cursor selection.
    ///
    /// # Panics
    /// Panics if `ranges` is empty.
    pub fn new(ranges: Vec<Range>, primary: usize) -> Self {
        assert!(!ranges.is_empty(), "Selection must have at least one range");
        assert!(
            primary < ranges.len(),
            "Primary index out of bounds: {} >= {}",
            primary,
            ranges.len()
        );
        Self { ranges, primary }
    }

    pub fn primary(&self) -> Range {
        self.ranges[self.primary]
    }

    pub fn primary_mut(&mut self) -> &mut Range {
        &mut self.ranges[self.primary]
    }

    pub fn ranges(&self) -> &[Range] {
        &self.ranges
    }

    /// Transform all ranges using a function.
    pub fn transform<F: FnMut(Range) -> Range>(&self, mut f: F) -> Self {
        Self {
            ranges: self.ranges.iter().copied().map(&mut f).collect(),
            primary: self.primary,
        }
    }

    /// Map all range positions through a changeset (updating positions after edits).
    pub fn map(&self, changes: &super::transaction::ChangeSet) -> Self {
        self.transform(|range| {
            Range::new(
                changes.map_position(range.anchor),
                changes.map_position(range.head),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_range_point() {
        let r = Range::point(10);
        assert_eq!(r.anchor, 10);
        assert_eq!(r.head, 10);
        assert!(r.is_empty());
    }

    #[test]
    fn test_range_from_to() {
        let r = Range::new(5, 10);
        assert_eq!(r.from(), 5);
        assert_eq!(r.to(), 10);

        let r2 = Range::new(10, 5);
        assert_eq!(r2.from(), 5);
        assert_eq!(r2.to(), 10);
    }

    #[test]
    fn test_range_flip() {
        let r = Range::new(5, 10);
        let flipped = r.flip();
        assert_eq!(flipped.anchor, 10);
        assert_eq!(flipped.head, 5);
    }

    #[test]
    fn test_selection_point() {
        let sel = Selection::point(42);
        assert_eq!(sel.primary().head, 42);
        assert_eq!(sel.ranges().len(), 1);
    }

    #[test]
    fn test_selection_single() {
        let sel = Selection::single(5, 10);
        assert_eq!(sel.primary().anchor, 5);
        assert_eq!(sel.primary().head, 10);
    }

    #[test]
    fn test_selection_transform() {
        let sel = Selection::single(5, 10);
        let transformed = sel.transform(|r| Range::new(r.anchor + 1, r.head + 1));
        assert_eq!(transformed.primary().anchor, 6);
        assert_eq!(transformed.primary().head, 11);
    }

    #[test]
    #[should_panic(expected = "Selection must have at least one range")]
    fn test_selection_empty_panics() {
        Selection::new(vec![], 0);
    }
}
