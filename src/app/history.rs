//! Bounded undo/redo history for the current save.

use serde_json::Value;

use super::EditorApp;

const MAX_UNDO: usize = 200;

pub(super) struct UndoStack<T> {
    committed: T,
    undo: Vec<T>,
    redo: Vec<T>,
}

impl<T> UndoStack<T> {
    pub(super) fn new(committed: T) -> Self {
        Self {
            committed,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    fn reset(&mut self, committed: T) {
        self.committed = committed;
        self.undo.clear();
        self.redo.clear();
    }

    pub(in crate::app) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(in crate::app) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

impl<T: Clone + PartialEq> UndoStack<T> {
    pub(in crate::app) fn has_uncommitted(&self, current: &T) -> bool {
        current != &self.committed
    }

    fn commit(&mut self, current: &T) -> bool {
        if !self.has_uncommitted(current) {
            return false;
        }
        self.undo.push(self.committed.clone());
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
        self.committed = current.clone();
        self.redo.clear();
        true
    }

    fn undo(&mut self, current: &T) -> Option<T> {
        let previous = self.undo.pop()?;
        self.redo.push(current.clone());
        self.committed = previous.clone();
        Some(previous)
    }

    fn redo(&mut self, current: &T) -> Option<T> {
        let next = self.redo.pop()?;
        self.undo.push(current.clone());
        self.committed = next.clone();
        Some(next)
    }
}

impl EditorApp {
    /// Whether the current document differs from the file on disk.
    pub fn is_dirty(&self) -> bool {
        self.save
            .as_ref()
            .is_some_and(|save| save.data != self.clean)
    }

    pub(in crate::app) fn reset_history(&mut self) {
        let data = self
            .save
            .as_ref()
            .map_or(Value::Null, |save| save.data.clone());
        self.history.reset(data);
        self.raw.text.clear();
        self.raw.text_dirty = false;
        self.inventory.selected = None;
    }

    /// Records the current document in the undo history if it changed.
    pub(in crate::app) fn commit(&mut self) {
        let Some(data) = self.save.as_ref().map(|save| save.data.clone()) else {
            return;
        };
        if self.history.commit(&data) && !self.raw.text_dirty {
            self.raw.text.clear();
        }
    }

    pub(in crate::app) fn undo(&mut self) {
        self.commit();
        let Some(save) = self.save.as_mut() else {
            return;
        };
        let Some(previous) = self.history.undo(&save.data) else {
            return;
        };
        save.data = previous;
        self.raw.text.clear();
    }

    pub(in crate::app) fn redo(&mut self) {
        let Some(save) = self.save.as_mut() else {
            return;
        };
        let Some(next) = self.history.redo(&save.data) else {
            return;
        };
        save.data = next;
        self.raw.text.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::UndoStack;

    #[test]
    fn edits_are_grouped_and_new_commits_discard_redo() {
        let mut history = UndoStack::new(0);
        assert!(history.commit(&1));
        assert!(history.commit(&2));
        assert_eq!(history.undo(&2), Some(1));
        assert_eq!(history.redo(&1), Some(2));
        assert!(history.commit(&3));
        assert!(!history.can_redo());
        assert_eq!(history.undo(&3), Some(2));
    }

    #[test]
    fn undo_history_is_bounded() {
        let mut history = UndoStack::new(0);
        for value in 1..=201 {
            history.commit(&value);
        }

        assert_eq!(history.undo(&201), Some(200));
        for value in (1..=199).rev() {
            assert_eq!(history.undo(&(value + 1)), Some(value));
        }
        assert!(!history.can_undo());
    }
}
