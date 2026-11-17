//! Shared display formatting and textual helpers.

use crate::save::BackupInfo;
use crate::ui::Names;
use std::path::Path;

/// Formats a number compactly (no trailing zeros, at most 2 decimals).
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e9 {
        format!("{v:.0}")
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}
/// Turns `sterilizedbandage` into `Sterilizedbandage` as a last-resort name.
pub fn prettify_id(id: &str) -> String {
    let mut c = id.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => "(empty id)".to_owned(),
    }
}

/// Removes Unity rich-text tags (`<color=..>`, `<b>`, `<sprite ..>`).
pub fn strip_rich_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = 0;
    for ch in s.chars() {
        match ch {
            '<' => depth += 1,
            '>' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out
}

/// Icon for an item category.
pub fn category_icon(category: &str) -> &'static str {
    match category {
        "medical" => "✚",
        "drug" => "💊",
        "food" => "🍞",
        "water" => "💧",
        "container" => "🎒",
        "tool" => "🔧",
        "utility" => "🔦",
        "trash" => "🗑",
        "custom" => "★",
        "unobtainable" => "⛔",
        _ => "📦",
    }
}

pub fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

pub fn modified_text(backup: &BackupInfo, names: Names) -> String {
    backup.modified.map_or_else(
        || names.text("unknown date").into_owned(),
        |t| {
            chrono::DateTime::<chrono::Local>::from(t)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        },
    )
}

pub fn size_text(bytes: u64) -> String {
    format!("{:.1} KB", bytes as f64 / 1024.0)
}
