//! Compatibility views of the currently selected installation.
//!
//! Published snapshots are immutable and retained for the process lifetime:
//! legacy schema lookups return `'static` references which may outlive a game
//! directory change. Identical installations reuse the same snapshot.

use std::collections::HashSet;
use std::ops::Deref;
use std::sync::{Mutex, OnceLock, RwLock};

use super::metadata::Metadata;

pub fn intern(value: impl AsRef<str>) -> &'static str {
    static STRINGS: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let mut strings = STRINGS.get_or_init(Default::default).lock().unwrap();
    if let Some(value) = strings.get(value.as_ref()) {
        return value;
    }
    let value = Box::leak(value.as_ref().to_owned().into_boxed_str());
    strings.insert(value);
    value
}

fn current() -> &'static RwLock<&'static Metadata> {
    static CURRENT: OnceLock<RwLock<&'static Metadata>> = OnceLock::new();
    CURRENT.get_or_init(|| RwLock::new(Box::leak(Box::new(Metadata::default()))))
}

pub fn metadata() -> &'static Metadata {
    #[cfg(test)]
    if let Some(metadata) = TEST_METADATA.with(|current| *current.borrow()) {
        return metadata;
    }
    *current().read().unwrap()
}

#[cfg(test)]
thread_local! {
    static TEST_METADATA: std::cell::RefCell<Option<&'static Metadata>> = const { std::cell::RefCell::new(None) };
}

/// Isolates synthetic metadata from concurrent tests and restores the previous snapshot.
#[cfg(test)]
pub(crate) fn scoped_for_test(metadata: Metadata) -> TestMetadata {
    let metadata = Box::leak(Box::new(metadata));
    let previous = TEST_METADATA.with(|current| current.replace(Some(metadata)));
    TestMetadata(previous)
}

#[cfg(test)]
pub(crate) struct TestMetadata(Option<&'static Metadata>);

#[cfg(test)]
impl Drop for TestMetadata {
    fn drop(&mut self) {
        TEST_METADATA.with(|current| current.replace(self.0));
    }
}

pub(super) fn install(metadata: Metadata) {
    static SNAPSHOTS: OnceLock<Mutex<Vec<&'static Metadata>>> = OnceLock::new();
    let mut snapshots = SNAPSHOTS.get_or_init(Default::default).lock().unwrap();
    let snapshot = snapshots
        .iter()
        .copied()
        .find(|snapshot| **snapshot == metadata)
        .unwrap_or_else(|| {
            let snapshot = Box::leak(Box::new(metadata));
            snapshots.push(snapshot);
            snapshot
        });
    *current().write().unwrap() = snapshot;
}

/// Clears all game-dependent catalogue and schema views.
pub fn clear() {
    install(Metadata::default());
}

/// A slice-shaped view of the current immutable metadata snapshot.
pub struct RuntimeSlice<T: 'static>(pub fn() -> &'static [T]);

impl<T> Copy for RuntimeSlice<T> {}
impl<T> Clone for RuntimeSlice<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> RuntimeSlice<T> {
    pub fn snapshot(self) -> &'static [T] {
        (self.0)()
    }
}

impl<T> Deref for RuntimeSlice<T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        self.snapshot()
    }
}

impl<T> IntoIterator for RuntimeSlice<T> {
    type Item = &'static T;
    type IntoIter = std::slice::Iter<'static, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.snapshot().iter()
    }
}

impl<T> IntoIterator for &RuntimeSlice<T> {
    type Item = &'static T;
    type IntoIter = std::slice::Iter<'static, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.snapshot().iter()
    }
}
