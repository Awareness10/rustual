//! Undoable edits and the edit history (port of `textual/document/_edit.py` and `_history.py`).

use super::document::{Document, EditResult, Location, Selection};
use std::collections::VecDeque;
use std::time::Duration;
use tokio::time::Instant;

/// Replace the text between two locations with new text. An empty `text` is a deletion,
/// `from == to` an insertion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    /// The text to insert.
    pub text: String,
    pub from_location: Location,
    pub to_location: Location,
    /// Keep the selection at the same place relative to the surrounding text (API edits);
    /// otherwise the cursor jumps to the end of the edit (keyboard edits).
    pub maintain_selection_offset: bool,
    original_selection: Option<Selection>,
    updated_selection: Option<Selection>,
    edit_result: Option<EditResult>,
}

impl Edit {
    pub fn new(
        text: impl Into<String>,
        from_location: Location,
        to_location: Location,
        maintain_selection_offset: bool,
    ) -> Self {
        Edit {
            text: text.into(),
            from_location,
            to_location,
            maintain_selection_offset,
            original_selection: None,
            updated_selection: None,
            edit_result: None,
        }
    }

    /// The edit location nearest the start of the document.
    pub fn top(&self) -> Location {
        self.from_location.min(self.to_location)
    }

    /// The edit location nearest the end of the document.
    pub fn bottom(&self) -> Location {
        self.from_location.max(self.to_location)
    }

    /// The result of the last `perform`, if any.
    pub fn result(&self) -> Option<&EditResult> {
        self.edit_result.as_ref()
    }

    /// Where the selection should move to after `perform` / `undo`.
    pub fn updated_selection(&self) -> Option<Selection> {
        self.updated_selection
    }

    /// Perform the edit on `document`, given the current `selection`.
    pub fn perform(
        &mut self,
        document: &mut Document,
        selection: Selection,
        record_selection: bool,
    ) -> EditResult {
        if record_selection {
            self.original_selection = Some(selection);
        }
        let (bottom_row, bottom_col) = self.bottom();
        let (sel_start, sel_end) = (selection.start, selection.end);
        let result = document.replace_range(self.top(), self.bottom(), &self.text);
        let (new_row, new_col) = result.end_location;

        let column_offset = new_col as isize - bottom_col as isize;
        let row_offset = new_row as isize - bottom_row as isize;
        let shift = |loc: Location| -> Location {
            let col = if bottom_row == loc.0 && bottom_col <= loc.1 {
                (loc.1 as isize + column_offset).max(0) as usize
            } else {
                loc.1
            };
            let row = if bottom_row <= loc.0 {
                (loc.0 as isize + row_offset).max(0) as usize
            } else {
                loc.0
            };
            (row, col)
        };
        self.updated_selection = Some(if self.maintain_selection_offset {
            Selection::new(shift(sel_start), shift(sel_end))
        } else {
            Selection::cursor(result.end_location)
        });
        self.edit_result = Some(result.clone());
        result
    }

    /// Undo the edit (it must have been performed). The selection is restored to the one
    /// recorded when the edit was first performed.
    pub fn undo(&mut self, document: &mut Document) -> EditResult {
        let result = self
            .edit_result
            .clone()
            .expect("Edit::undo called before Edit::perform");
        let undo_result =
            document.replace_range(self.top(), result.end_location, &result.replaced_text);
        self.updated_selection = self.original_selection;
        undo_result
    }
}

/// Batches edits into checkpoints that are undone / redone together (Textual's `EditHistory`).
#[derive(Debug, Clone)]
pub struct EditHistory {
    pub max_checkpoints: usize,
    /// Maximum time since the last edit before a new batch is started.
    pub checkpoint_timer: Duration,
    /// Maximum number of characters in a batch.
    pub checkpoint_max_characters: usize,
    last_edit_time: Instant,
    character_count: usize,
    force_end_batch: bool,
    previously_replaced: bool,
    undo_stack: VecDeque<Vec<Edit>>,
    redo_stack: Vec<Vec<Edit>>,
}

impl EditHistory {
    pub fn new(max_checkpoints: usize) -> Self {
        EditHistory {
            max_checkpoints,
            checkpoint_timer: Duration::from_secs(2),
            checkpoint_max_characters: 100,
            last_edit_time: Instant::now(),
            character_count: 0,
            force_end_batch: false,
            previously_replaced: false,
            undo_stack: VecDeque::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Record a performed edit, batching it with the previous one when appropriate.
    pub fn record(&mut self, edit: Edit) {
        self.record_at(edit, Instant::now());
    }

    pub(crate) fn record_at(&mut self, edit: Edit, now: Instant) {
        let Some(result) = edit.edit_result.as_ref() else {
            return;
        };
        if edit.text.is_empty() && result.replaced_text.is_empty() {
            return;
        }
        let is_replacement = !result.replaced_text.is_empty();
        let edit_characters = edit.text.chars().count();
        let has_newline = |s: &str| s.contains('\n') || s.contains('\r');
        let contains_newline = has_newline(&edit.text) || has_newline(&result.replaced_text);

        let new_batch = self.undo_stack.is_empty()
            || self.force_end_batch
            || edit_characters > 1
            || contains_newline
            || is_replacement != self.previously_replaced
            || now.saturating_duration_since(self.last_edit_time) > self.checkpoint_timer
            || self.character_count + edit_characters > self.checkpoint_max_characters;
        if new_batch {
            self.undo_stack.push_back(vec![edit]);
            while self.undo_stack.len() > self.max_checkpoints.max(1) {
                self.undo_stack.pop_front();
            }
            self.character_count = edit_characters;
            self.force_end_batch = false;
        } else {
            self.undo_stack.back_mut().expect("non-empty").push(edit);
            self.character_count += edit_characters;
        }
        self.last_edit_time = now;
        self.previously_replaced = is_replacement;
        self.redo_stack.clear();
        if contains_newline || edit_characters > 1 {
            self.checkpoint();
        }
    }

    /// Pop the latest batch for undoing (it moves to the redo stack).
    pub(crate) fn pop_undo(&mut self) -> Option<Vec<Edit>> {
        self.undo_stack.pop_back()
    }

    /// Push a batch that has been undone.
    pub(crate) fn push_redo(&mut self, batch: Vec<Edit>) {
        self.redo_stack.push(batch);
    }

    /// Pop the latest undone batch for redoing.
    pub(crate) fn pop_redo(&mut self) -> Option<Vec<Edit>> {
        self.redo_stack.pop()
    }

    /// Push a batch that has been redone (no further edits are added to it).
    pub(crate) fn push_undo(&mut self, batch: Vec<Edit>) {
        self.undo_stack.push_back(batch);
        self.checkpoint();
    }

    /// Ensure the next recorded edit starts a new batch.
    pub fn checkpoint(&mut self) {
        self.force_end_batch = true;
    }

    /// Forget all history.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.last_edit_time = Instant::now();
        self.force_end_batch = false;
        self.previously_replaced = false;
    }

    pub fn undo_stack(&self) -> impl Iterator<Item = &Vec<Edit>> {
        self.undo_stack.iter()
    }

    pub fn redo_stack(&self) -> &[Vec<Edit>] {
        &self.redo_stack
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn performed(doc: &mut Document, text: &str, from: Location, to: Location) -> Edit {
        let mut e = Edit::new(text, from, to, false);
        e.perform(doc, Selection::cursor(to), true);
        e
    }

    #[test]
    fn edit_and_undo() {
        let mut doc = Document::new("hello world");
        let mut e = performed(&mut doc, "big ", (0, 6), (0, 6));
        assert_eq!(doc.text(), "hello big world");
        assert_eq!(e.updated_selection(), Some(Selection::cursor((0, 10))));
        e.undo(&mut doc);
        assert_eq!(doc.text(), "hello world");
        assert_eq!(e.updated_selection(), Some(Selection::cursor((0, 6))));
    }

    #[test]
    fn maintain_selection_offset() {
        let mut doc = Document::new("abc\ndef");
        let mut e = Edit::new("XY", (0, 0), (0, 0), true);
        e.perform(&mut doc, Selection::new((0, 1), (1, 2)), true);
        assert_eq!(e.updated_selection(), Some(Selection::new((0, 3), (1, 2))));
        let mut e = Edit::new("1\n2", (0, 0), (0, 0), true);
        e.perform(&mut doc, Selection::cursor((0, 2)), true);
        assert_eq!(e.updated_selection(), Some(Selection::cursor((1, 3))));
    }

    #[test]
    fn batching() {
        let t0 = Instant::now();
        let mut doc = Document::new("");
        let mut h = EditHistory::new(50);
        for (i, c) in "abc".chars().enumerate() {
            let e = performed(&mut doc, &c.to_string(), (0, i), (0, i));
            h.record_at(e, t0);
        }
        assert_eq!(h.undo_stack().count(), 1);
        // Newline gets its own batch, and the next edit starts a new one.
        let e = performed(&mut doc, "\n", (0, 3), (0, 3));
        h.record_at(e, t0);
        let e = performed(&mut doc, "d", (1, 0), (1, 0));
        h.record_at(e, t0);
        assert_eq!(h.undo_stack().count(), 3);
        // Timer expiry starts a new batch.
        let e = performed(&mut doc, "e", (1, 1), (1, 1));
        h.record_at(e, t0 + Duration::from_secs(3));
        assert_eq!(h.undo_stack().count(), 4);
        // Switching from insertion to deletion starts a new batch.
        let e = performed(&mut doc, "", (1, 1), (1, 2));
        h.record_at(e, t0 + Duration::from_secs(3));
        assert_eq!(h.undo_stack().count(), 5);
        // Pastes (multi-char) are isolated.
        let e = performed(&mut doc, "xyz", (1, 1), (1, 1));
        h.record_at(e, t0 + Duration::from_secs(3));
        assert_eq!(h.undo_stack().count(), 6);
        // max_checkpoints.
        let mut h = EditHistory::new(2);
        for i in 0..5 {
            let e = performed(&mut doc, "\n", (0, 0), (0, 0));
            h.record_at(e, t0 + Duration::from_secs(i));
        }
        assert_eq!(h.undo_stack().count(), 2);
    }

    #[test]
    fn checkpoint_forces_new_batch() {
        let t0 = Instant::now();
        let mut doc = Document::new("");
        let mut h = EditHistory::new(10);
        let e = performed(&mut doc, "a", (0, 0), (0, 0));
        h.record_at(e, t0);
        h.checkpoint();
        let e = performed(&mut doc, "b", (0, 1), (0, 1));
        h.record_at(e, t0);
        assert_eq!(h.undo_stack().count(), 2);
        // Empty edits are not recorded.
        let e = performed(&mut doc, "", (0, 1), (0, 1));
        h.record_at(e, t0);
        assert_eq!(h.undo_stack().count(), 2);
    }
}
