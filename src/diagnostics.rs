//! Debugging information for bug reports.
//!
//! The editor keeps a short log of the errors it showed ([`ErrorLog`]) and
//! writes a crash report when it panics ([`install_panic_hook`]). From these
//! and a description of the installation the user can build a [`Report`],
//! review and edit it, and open a pre-filled GitHub issue ([`issue_url`]).
//!
//! Confidentiality: nothing is sent automatically, the report never contains
//! save contents, and every text goes through a [`Redactor`] that replaces the
//! user's home folder, user and computer names, other users' profile folders,
//! Steam account ids and e-mail addresses with placeholders.

use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};

/// Number of errors kept in the [`ErrorLog`].
pub const ERROR_LOG_CAPACITY: usize = 30;

/// Longest issue URL handed to the browser; GitHub rejects much longer ones.
pub const MAX_ISSUE_URL: usize = 7000;

/// Replaces personal information in texts with placeholders.
#[derive(Debug, Clone, Default)]
pub struct Redactor {
    home: Vec<String>,
    words: Vec<(String, &'static str)>,
}

impl Redactor {
    /// A redactor for this computer: its home folder, user and computer name.
    pub fn for_this_computer() -> Self {
        let user = ["USERNAME", "USER", "LOGNAME"]
            .into_iter()
            .find_map(|v| std::env::var(v).ok());
        let host = ["COMPUTERNAME", "HOSTNAME"]
            .into_iter()
            .find_map(|v| std::env::var(v).ok())
            .or_else(|| {
                fs::read_to_string("/etc/hostname")
                    .ok()
                    .map(|h| h.trim().to_owned())
            });
        Self::new(dirs::home_dir(), user, host)
    }

    /// A redactor for the given home folder, user and computer name.
    pub fn new(home: Option<PathBuf>, user: Option<String>, host: Option<String>) -> Self {
        let mut redactor = Self::default();
        if let Some(home) = home {
            let home = home
                .to_string_lossy()
                .trim_end_matches(['/', '\\'])
                .to_owned();
            if home.len() > 3 {
                redactor.home.push(home.replace('\\', "/"));
                redactor.home.push(home.replace('/', "\\"));
                redactor.home.push(home);
            }
        }
        // Very short names would match ordinary words.
        for (name, placeholder) in [(user, "<user>"), (host, "<computer>")] {
            if let Some(name) = name.filter(|n| n.chars().count() >= 3) {
                redactor.words.push((name, placeholder));
            }
        }
        redactor
    }

    /// `text` with personal information replaced.
    pub fn redact(&self, text: &str) -> String {
        let mut text = text.to_owned();
        for home in &self.home {
            text = replace_ignore_case(&text, home, "~", false);
        }
        text = redact_profile_folders(&text);
        for (word, placeholder) in &self.words {
            text = replace_ignore_case(&text, word, placeholder, true);
        }
        text = redact_steam_ids(&text);
        redact_emails(&text)
    }
}

/// Replaces `needle` (ASCII case-insensitive); with `whole_word` only where
/// it is not part of a longer word.
fn replace_ignore_case(text: &str, needle: &str, with: &str, whole_word: bool) -> String {
    if needle.is_empty() {
        return text.to_owned();
    }
    let word_char = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        let matches = text
            .get(i..i + needle.len())
            .is_some_and(|s| s.eq_ignore_ascii_case(needle));
        if matches
            && (!whole_word
                || (!word_char(text[..i].chars().next_back())
                    && !word_char(text[i + needle.len()..].chars().next())))
        {
            out.push_str(with);
            i += needle.len();
        } else {
            let c = text[i..].chars().next().expect("inside the text");
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

/// Hides the user name in profile paths of any user: `C:\Users\<name>`,
/// `/home/<name>`, `/Users/<name>`.
fn redact_profile_folders(text: &str) -> String {
    const SHARED: [&str; 5] = ["public", "default", "all users", "steamuser", "<user>"];
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    'scan: while !rest.is_empty() {
        for marker in ["\\users\\", "/users/", "/home/"] {
            if rest
                .get(..marker.len())
                .is_some_and(|s| s.eq_ignore_ascii_case(marker))
            {
                let after = &rest[marker.len()..];
                let end = after
                    .find(['\\', '/', '"', '\'', '`', '\n', '\r', ')', ']', ';', ','])
                    .unwrap_or(after.len());
                let name = &after[..end];
                out.push_str(&rest[..marker.len()]);
                if name.is_empty() || SHARED.contains(&name.to_lowercase().as_str()) {
                    out.push_str(name);
                } else {
                    out.push_str("<user>");
                }
                rest = &after[end..];
                continue 'scan;
            }
        }
        let c = rest.chars().next().expect("not empty");
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// Hides Steam account ids: 64-bit SteamIDs (`7656119…`) and the account
/// number in Steam's `UserGameStats_<account>_<app>` file names.
fn redact_steam_ids(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() && (i == 0 || !chars[i - 1].is_ascii_alphanumeric()) {
            let end = chars[i..]
                .iter()
                .position(|c| !c.is_ascii_digit())
                .map_or(chars.len(), |n| i + n);
            let number: String = chars[i..end].iter().collect();
            let before: String = chars[..i].iter().collect();
            if (number.len() == 17 && number.starts_with("7656119"))
                || (before.ends_with("UserGameStats_") && chars.get(end) == Some(&'_'))
            {
                out.push_str("<steam-account>");
            } else {
                out.push_str(&number);
            }
            i = end;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Hides e-mail addresses.
fn redact_emails(text: &str) -> String {
    let is_part = |c: char| c.is_alphanumeric() || "._%+-".contains(c);
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        // A sentence may end right after the address.
        let address = word.trim_end_matches('.');
        let looks_like_email = address
            .split_once('@')
            .is_some_and(|(user, domain)| !user.is_empty() && domain.contains('.'));
        if looks_like_email {
            out.push_str("<email>");
            out.push_str(&word[address.len()..]);
        } else {
            out.push_str(word);
        }
        word.clear();
    };
    for c in text.chars() {
        if is_part(c) || c == '@' {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

/// An error the editor showed to the user.
#[derive(Debug, Clone, PartialEq)]
pub struct LogEntry {
    /// When it happened.
    pub at: chrono::DateTime<chrono::Local>,
    /// The message as shown.
    pub message: String,
}

/// The most recent errors, oldest first.
#[derive(Debug, Clone, Default)]
pub struct ErrorLog {
    entries: VecDeque<LogEntry>,
}

impl ErrorLog {
    /// Records an error, dropping the oldest beyond [`ERROR_LOG_CAPACITY`].
    pub fn push(&mut self, message: impl Into<String>) {
        if self.entries.len() == ERROR_LOG_CAPACITY {
            self.entries.pop_front();
        }
        self.entries.push_back(LogEntry {
            at: chrono::Local::now(),
            message: message.into(),
        });
    }

    /// The recorded errors, oldest first.
    pub fn entries(&self) -> impl Iterator<Item = &LogEntry> {
        self.entries.iter()
    }

    /// Number of recorded errors.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no error was recorded.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// A possible problem noticed in the installation, offered for reporting.
#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    /// Short English title (also the suggested issue title).
    pub title: String,
    /// Details, may contain paths (redacted when reported).
    pub detail: String,
}

/// A report: titled sections of Markdown lines.
#[derive(Debug, Clone, Default)]
pub struct Report {
    sections: Vec<(String, Vec<String>)>,
}

impl Report {
    /// Adds a section; empty sections are left out of the text.
    pub fn section(&mut self, title: &str, lines: Vec<String>) {
        self.sections.push((title.to_owned(), lines));
    }

    /// Markdown text of the report with personal information redacted.
    pub fn to_markdown(&self, redactor: &Redactor) -> String {
        let mut text = String::new();
        for (title, lines) in self.sections.iter().filter(|(_, l)| !l.is_empty()) {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&format!("### {title}\n\n"));
            for line in lines {
                text.push_str(&redactor.redact(line));
                text.push('\n');
            }
        }
        text
    }
}

/// Encodes `text` for a URL query value.
pub fn url_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// URL of a new issue in `repository` using the bug report form
/// (`.github/ISSUE_TEMPLATE/bug_report.yml`), pre-filled with the title,
/// the description and the diagnostics. If the diagnostics make the URL too
/// long they are left out (the caller should copy them to the clipboard);
/// the second value tells whether they were included.
pub fn issue_url(
    repository: &str,
    title: &str,
    description: &str,
    diagnostics: &str,
) -> (String, bool) {
    let base = format!(
        "{}/issues/new?template=bug_report.yml&title={}&what-happened={}",
        repository.trim_end_matches('/'),
        url_encode(title),
        url_encode(description),
    );
    let full = format!("{base}&diagnostics={}", url_encode(diagnostics));
    if full.len() <= MAX_ISSUE_URL {
        (full, true)
    } else {
        (base, false)
    }
}

/// Folder where crash reports are written.
pub fn crash_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("cu_save_editor").join("crashes"))
}

/// Writes a crash report (redacted) for every panic, then runs the default
/// panic handler.
pub fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        let text = format!(
            "Editor v{} ({} {})\n{}\n{info}\n\nBacktrace:\n{backtrace}\n",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH,
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        );
        if let Some(dir) = crash_dir() {
            let _ = write_crash_report(&dir, &Redactor::for_this_computer().redact(&text));
        }
        default(info);
    }));
}

/// Stores a crash report in `dir` and returns its path.
pub fn write_crash_report(dir: &Path, text: &str) -> std::io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
    let mut path = dir.join(format!("crash_{stamp}.txt"));
    let mut n = 1;
    while path.exists() {
        path = dir.join(format!("crash_{stamp}_{n}.txt"));
        n += 1;
    }
    fs::write(&path, text)?;
    Ok(path)
}

/// The newest crash report in `dir`: its path and text.
pub fn latest_crash_report(dir: &Path) -> Option<(PathBuf, String)> {
    let path = crash_reports(dir).into_iter().max()?;
    let text = fs::read_to_string(&path).ok()?;
    Some((path, text))
}

/// Deletes all crash reports in `dir`; returns how many were deleted.
pub fn delete_crash_reports(dir: &Path) -> usize {
    crash_reports(dir)
        .into_iter()
        .filter(|p| fs::remove_file(p).is_ok())
        .count()
}

/// Crash report files (named `crash_<date>…txt`, so they sort by date).
fn crash_reports(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("crash_") && n.ends_with(".txt"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn redactor() -> Redactor {
        Redactor::new(
            Some(PathBuf::from(r"C:\Users\Pavel")),
            Some("Pavel".into()),
            Some("DESKTOP-42".into()),
        )
    }

    #[test]
    fn personal_information_is_redacted() {
        let r = redactor();
        assert_eq!(
            r.redact(r"c:\users\pavel\AppData\LocalLow\save.sv"),
            r"~\AppData\LocalLow\save.sv"
        );
        assert_eq!(r.redact("C:/Users/Pavel/x"), "~/x");
        assert_eq!(
            r.redact(r"D:\Users\anna\x and /home/bob/.steam and /Users/Public/x"),
            r"D:\Users\<user>\x and /home/<user>/.steam and /Users/Public/x"
        );
        assert_eq!(
            r.redact(".../compatdata/4576490/pfx/drive_c/users/steamuser/AppData"),
            ".../compatdata/4576490/pfx/drive_c/users/steamuser/AppData"
        );
        assert_eq!(
            r.redact("pavel on DESKTOP-42; Pavelko stays"),
            "<user> on <computer>; Pavelko stays"
        );
        assert_eq!(
            r.redact("UserGameStats_104015252_4576490.bin id 76561198064281524 app 4576490"),
            "UserGameStats_<steam-account>_4576490.bin id <steam-account> app 4576490"
        );
        assert_eq!(r.redact("mail me: a.b@c.org."), "mail me: <email>.");
        assert_eq!(r.redact("Привет, Pavel"), "Привет, <user>");
    }

    #[test]
    fn long_diagnostics_are_left_out_of_the_issue_url() {
        let repo = "https://github.com/o/r";
        let (url, included) = issue_url(repo, "Crash & burn", "It broke", "small");
        assert!(included);
        assert!(url.starts_with("https://github.com/o/r/issues/new?template=bug_report.yml"));
        assert!(url.contains("title=Crash%20%26%20burn"));
        assert!(url.ends_with("&diagnostics=small"));

        let (url, included) = issue_url(repo, "t", "d", &"x".repeat(MAX_ISSUE_URL));
        assert!(!included);
        assert!(!url.contains("diagnostics="));
    }

    #[test]
    fn crash_reports_are_found_and_deleted() {
        let dir = tempfile::tempdir().unwrap();
        assert!(latest_crash_report(dir.path()).is_none());
        write_crash_report(dir.path(), "first").unwrap();
        let second = write_crash_report(dir.path(), "second").unwrap();
        assert_eq!(
            latest_crash_report(dir.path()),
            Some((second, "second".into()))
        );
        assert_eq!(delete_crash_reports(dir.path()), 2);
        assert!(latest_crash_report(dir.path()).is_none());
    }

    #[test]
    fn error_log_keeps_the_latest_errors() {
        let mut log = ErrorLog::default();
        for i in 0..ERROR_LOG_CAPACITY + 5 {
            log.push(format!("error {i}"));
        }
        assert_eq!(log.len(), ERROR_LOG_CAPACITY);
        assert_eq!(log.entries().next().unwrap().message, "error 5");
    }
}
