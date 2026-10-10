//! Selections and multi-cursor editing primitives.
//!
//! A [`Selection`] is an anchor/head pair of byte offsets; a
//! [`SelectionSet`] is an ordered collection of them, kept sorted and
//! non-overlapping. [`map_ranges_through_edits`] maps ranges across a batch
//! of replacements, which is what lets a multi-cursor edit produce the
//! right post-edit carets.

use std::ops::Range;

/// A selection expressed as an `anchor` and a `head` byte offset.
///
/// The head is where the caret is; the anchor is where the selection was
/// started. When they are equal the selection is collapsed at the caret.
/// Keeping both (instead of a sorted range) preserves the direction of the
/// selection, which matters for shift+arrow behaviour and for rendering the
/// caret at the moving end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    /// The byte offset where the selection started.
    pub anchor: usize,
    /// The byte offset of the caret (the moving end).
    pub head: usize,
}

impl Selection {
    /// Creates a collapsed selection (caret only) at `offset`.
    pub const fn collapsed_at(offset: usize) -> Self {
        Self {
            anchor: offset,
            head: offset,
        }
    }

    /// Creates a selection covering `start..end`.
    pub const fn region(start: usize, end: usize) -> Self {
        Self {
            anchor: start,
            head: end,
        }
    }

    /// Returns the smaller byte offset of the selection.
    pub fn start(&self) -> usize {
        self.anchor.min(self.head)
    }

    /// Returns the larger byte offset of the selection.
    pub fn end(&self) -> usize {
        self.anchor.max(self.head)
    }

    /// Returns the byte range covered by the selection.
    pub fn range(&self) -> Range<usize> {
        self.start()..self.end()
    }

    /// Returns `true` if the selection contains no text.
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// Returns the length of the selection in bytes.
    pub fn len(&self) -> usize {
        self.end() - self.start()
    }

    /// Returns the caret offset (the moving end of the selection).
    pub fn cursor(&self) -> usize {
        self.head
    }

    /// Moves the caret to `offset`, keeping the anchor in place.
    pub fn select_to(&mut self, offset: usize) {
        self.head = offset;
    }

    /// Collapses the selection at `offset`.
    pub fn collapse_to(&mut self, offset: usize) {
        self.anchor = offset;
        self.head = offset;
    }
}

/// An ordered, normalized set of selections.
///
/// Invariants, enforced on construction and insertion:
///
/// - selections are sorted by their start offset;
/// - strictly overlapping selections are merged;
/// - identical selections are deduplicated;
/// - the set is never empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectionSet {
    selections: Vec<Selection>,
}

impl SelectionSet {
    /// Creates a set with a single selection.
    pub fn new(selection: Selection) -> Self {
        Self {
            selections: vec![selection],
        }
    }

    /// Creates a set from selections, normalizing order and overlaps.
    ///
    /// An empty input produces a collapsed selection at offset 0.
    pub fn from_selections(selections: Vec<Selection>) -> Self {
        let mut selections = selections;
        selections.sort_by_key(|selection| (selection.start(), selection.end()));
        let mut normalized: Vec<Selection> = Vec::with_capacity(selections.len());
        for selection in selections {
            match normalized.last_mut() {
                Some(last) if same_range(last, &selection) => {}
                Some(last) if overlaps(last, &selection) => {
                    let start = last.start().min(selection.start());
                    let end = last.end().max(selection.end());
                    *last = Selection::region(start, end);
                }
                _ => normalized.push(selection),
            }
        }
        if normalized.is_empty() {
            normalized.push(Selection::collapsed_at(0));
        }
        Self {
            selections: normalized,
        }
    }

    /// Returns the selections in ascending order.
    pub fn iter(&self) -> std::slice::Iter<'_, Selection> {
        self.selections.iter()
    }

    /// Returns the number of selections.
    pub fn len(&self) -> usize {
        self.selections.len()
    }

    /// Returns `true` if no text is selected: a single collapsed caret.
    pub fn is_empty(&self) -> bool {
        self.selections.len() == 1 && self.primary().is_empty()
    }

    /// Returns `true` if there are multiple selections.
    pub fn is_multi(&self) -> bool {
        self.selections.len() > 1
    }

    /// Returns the primary (first) selection, which drives status display
    /// and IME.
    pub fn primary(&self) -> &Selection {
        // The constructor guarantees at least one selection.
        self.selections
            .first()
            .expect("selection set is never empty")
    }

    /// Returns the byte ranges of all selections, ascending.
    pub fn ranges(&self) -> Vec<Range<usize>> {
        self.selections.iter().map(Selection::range).collect()
    }

    /// Adds a selection, merging overlaps and deduplicating.
    pub fn add(&mut self, selection: Selection) {
        let mut selections = std::mem::take(&mut self.selections);
        selections.push(selection);
        *self = Self::from_selections(selections);
    }

    /// Replaces the set with a single selection.
    pub fn set_single(&mut self, selection: Selection) {
        self.selections = vec![selection];
    }

    /// Collapses every selection to the primary caret, returning `true` if
    /// anything changed.
    pub fn collapse_to_primary(&mut self) -> bool {
        if self.selections.len() == 1 && self.primary().is_empty() {
            return false;
        }
        let caret = self.primary().cursor();
        self.selections = vec![Selection::collapsed_at(caret)];
        true
    }
}

impl IntoIterator for SelectionSet {
    type Item = Selection;
    type IntoIter = std::vec::IntoIter<Selection>;

    fn into_iter(self) -> Self::IntoIter {
        self.selections.into_iter()
    }
}

fn same_range(a: &Selection, b: &Selection) -> bool {
    a.start() == b.start() && a.end() == b.end()
}

fn overlaps(a: &Selection, b: &Selection) -> bool {
    // Strict overlap only: adjacent selections (sharing a boundary) stay
    // separate so that selecting consecutive words keeps two selections.
    b.start() < a.end()
}

/// A replacement that will be applied to a buffer, described by the range
/// being replaced and the byte length of the text replacing it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replacement {
    /// The range being replaced, in pre-edit coordinates.
    pub range: Range<usize>,
    /// The byte length of the inserted text.
    pub new_len: usize,
}

/// Maps ranges across a batch of replacements.
///
/// `replacements` must be sorted ascending by start and non-overlapping —
/// which is exactly the order [`SelectionSet::ranges`] produces and the
/// order a multi-cursor edit applies.
///
/// Boundary behaviour:
///
/// - offsets before a replacement are unchanged;
/// - offsets after a non-empty replacement shift by the length delta;
/// - offsets strictly inside a replaced range map to the end of the
///   inserted text;
/// - offsets at an insertion point map to the end of the inserted text
///   (caret affinity to the right, so typing moves carets forward).
pub fn map_ranges_through_edits(
    ranges: &[Range<usize>],
    replacements: &[Replacement],
) -> Vec<Range<usize>> {
    ranges
        .iter()
        .map(|range| {
            let start = map_offset(range.start, replacements);
            let end = map_offset(range.end, replacements);
            start..end.max(start)
        })
        .collect()
}

fn map_offset(offset: usize, replacements: &[Replacement]) -> usize {
    let mut delta: isize = 0;
    for replacement in replacements {
        if replacement.range.is_empty() {
            // Insertion: offsets at or after the insertion point shift
            // right (caret affinity to the right).
            if offset < replacement.range.start {
                break;
            }
            delta += replacement.new_len as isize;
        } else if offset <= replacement.range.start {
            // Clamp to the left edge of the replacement.
            break;
        } else if offset >= replacement.range.end {
            delta += replacement.new_len as isize - replacement.range.len() as isize;
        } else {
            // Inside the replaced range: snap to the end of the inserted
            // text.
            return (replacement.range.start as isize + delta + replacement.new_len as isize).max(0)
                as usize;
        }
    }
    (offset as isize + delta).max(0) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selections_normalize_order_and_overlap() {
        let set = SelectionSet::from_selections(vec![
            Selection::region(10, 15),
            Selection::region(0, 5),
            Selection::region(3, 12),
        ]);
        assert_eq!(set.len(), 1);
        assert_eq!(set.primary().range(), 0..15);
    }

    #[test]
    fn adjacent_selections_stay_separate() {
        let set = SelectionSet::from_selections(vec![
            Selection::region(0, 3),
            Selection::region(3, 6),
            Selection::region(9, 12),
        ]);
        assert_eq!(set.ranges(), vec![0..3, 3..6, 9..12]);
    }

    #[test]
    fn duplicate_carets_are_deduplicated() {
        let set = SelectionSet::from_selections(vec![
            Selection::collapsed_at(5),
            Selection::collapsed_at(5),
            Selection::collapsed_at(8),
        ]);
        assert_eq!(set.len(), 2);
        assert_eq!(set.ranges(), vec![5..5, 8..8]);
    }

    #[test]
    fn empty_set_becomes_a_caret_at_zero() {
        let set = SelectionSet::from_selections(Vec::new());
        assert_eq!(set.ranges(), vec![0..0]);
    }

    #[test]
    fn add_merges_overlapping_selections() {
        let mut set = SelectionSet::new(Selection::region(0, 4));
        set.add(Selection::region(6, 9));
        set.add(Selection::region(8, 12));
        assert_eq!(set.ranges(), vec![0..4, 6..12]);
    }

    #[test]
    fn collapse_to_primary_keeps_the_first_caret() {
        let mut set =
            SelectionSet::from_selections(vec![Selection::region(2, 5), Selection::region(9, 12)]);
        assert!(set.collapse_to_primary());
        assert_eq!(set.ranges(), vec![5..5]);
        assert!(!set.collapse_to_primary());
    }

    #[test]
    fn maps_offsets_before_and_after_edits() {
        let replacements = [
            Replacement {
                range: 2..4,
                new_len: 6,
            },
            Replacement {
                range: 10..12,
                new_len: 0,
            },
        ];
        assert_eq!(
            map_ranges_through_edits(std::slice::from_ref(&(0..1)), &replacements),
            vec![0..1]
        );
        // After the first edit: +4 bytes.
        assert_eq!(
            map_ranges_through_edits(std::slice::from_ref(&(5..8)), &replacements),
            vec![9..12]
        );
        // After both: +4 then -2.
        assert_eq!(
            map_ranges_through_edits(std::slice::from_ref(&(13..15)), &replacements),
            vec![15..17]
        );
    }

    #[test]
    fn maps_offsets_inside_replacements_to_the_insert_end() {
        let replacements = [Replacement {
            range: 4..8,
            new_len: 2,
        }];
        assert_eq!(
            map_ranges_through_edits(std::slice::from_ref(&(6..7)), &replacements),
            vec![6..6]
        );
    }

    #[test]
    fn insertions_shift_carets_to_the_right() {
        let replacements = [Replacement {
            range: 3..3,
            new_len: 2,
        }];
        // A caret exactly at the insertion point ends up after the text.
        assert_eq!(
            map_ranges_through_edits(std::slice::from_ref(&(3..3)), &replacements),
            vec![5..5]
        );
        assert_eq!(
            map_ranges_through_edits(std::slice::from_ref(&(1..2)), &replacements),
            vec![1..2]
        );
        assert_eq!(
            map_ranges_through_edits(std::slice::from_ref(&(9..9)), &replacements),
            vec![11..11]
        );
    }

    #[test]
    fn multi_edit_typing_maps_each_caret() {
        // Replace the selections 0..3 and 8..11 with one byte each.
        let replacements = [
            Replacement {
                range: 0..3,
                new_len: 1,
            },
            Replacement {
                range: 8..11,
                new_len: 1,
            },
        ];
        let mapped = map_ranges_through_edits(&[0..3, 8..11], &replacements);
        assert_eq!(mapped, vec![0..1, 6..7]);
    }
}
