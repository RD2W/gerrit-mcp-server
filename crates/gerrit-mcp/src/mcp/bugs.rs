// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Maxim Krutovercev (RD2W) <mkrutovercev@rd2w.ru>

//! Ticket/bug extraction from commit messages.
//!
//! Trackers are described declaratively (see the `[bugs]` section in the
//! configuration) so supporting another bug tracker needs no code change.
//!
//! Patterns are compiled with `regex-lite`: look-around and backreferences are
//! not available, so keep the patterns inside that subset.
//!
//! Extraction works in two phases per tracker:
//!
//! 1. **footer** — every line matching `^\s*(?:key1|key2)\s*:\s*(.+)$` is
//!    collected (multi-line, case-insensitive); the captured value is split on
//!    commas and whitespace, and each token must match `footer_value`
//!    **entirely** (the pattern is anchored automatically, hence `fb/123` does
//!    not match `(?:b/)?(\d+)`);
//! 2. **inline** — an optional pattern searched anywhere in the message.

use regex_lite::Regex;

/// A compiled tracker definition.
#[derive(Debug, Clone)]
pub struct BugTrackerSpec {
    /// Tracker name; shown in the output when several trackers produce hits.
    pub name: String,
    /// Lower-cased footer prefixes this tracker listens to.
    pub footer_keys: Vec<String>,
    /// Anchored against every token of a matched footer value.
    footer_value: Regex,
    /// Optional pattern searched anywhere in the message.
    inline: Option<Regex>,
    /// Multi-line, case-insensitive matcher for footer lines.
    footer_line: Regex,
}

/// One extracted ticket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BugHit {
    /// Name of the tracker that produced the hit.
    pub tracker: String,
    /// Ticket identifier exactly as written in the commit message.
    pub id: String,
}

/// Escapes a literal string so it can be embedded into a pattern.
///
/// `regex-lite` has no `escape()` helper, and footer keys are user-supplied.
fn escape_literal(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if !ch.is_ascii_alphanumeric() {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

impl BugTrackerSpec {
    /// Compiles one tracker definition.
    ///
    /// # Errors
    /// Returns a human-readable message naming the offending field when a
    /// pattern is invalid, when `footer_keys` is empty, or when `name` is
    /// empty.
    pub fn compile(
        name: &str,
        footer_keys: &[String],
        footer_value: &str,
        inline: Option<&str>,
    ) -> Result<Self, String> {
        if name.trim().is_empty() {
            return Err("name must not be empty".to_string());
        }

        let keys: Vec<String> = footer_keys
            .iter()
            .map(|k| k.trim().to_lowercase())
            .filter(|k| !k.is_empty())
            .collect();
        if keys.is_empty() {
            return Err("footer_keys must contain at least one non-empty key".to_string());
        }

        if footer_value.trim().is_empty() {
            return Err("footer_value must not be empty".to_string());
        }

        let alternation = keys
            .iter()
            .map(|k| escape_literal(k))
            .collect::<Vec<_>>()
            .join("|");
        let footer_line = Regex::new(&format!(r"(?im)^\s*(?:{alternation})\s*:\s*(.+)$"))
            .map_err(|e| format!("footer_keys: {e}"))?;

        // Anchored so that a token must match as a whole.
        let footer_value = Regex::new(&format!(r"^(?:{footer_value})$"))
            .map_err(|e| format!("footer_value: {e}"))?;

        let inline = match inline {
            Some(pattern) if !pattern.trim().is_empty() => {
                Some(Regex::new(pattern).map_err(|e| format!("inline: {e}"))?)
            }
            _ => None,
        };

        Ok(Self {
            name: name.trim().to_string(),
            footer_keys: keys,
            footer_value,
            inline,
            footer_line,
        })
    }

    /// One-line summary of what this tracker recognises, for startup logs.
    pub fn describe(&self) -> String {
        let footer = self.footer_keys.join(", ");
        match &self.inline {
            Some(inline) => format!(
                "{} (footer: {footer}; inline: {})",
                self.name,
                inline.as_str()
            ),
            None => format!("{} (footer: {footer})", self.name),
        }
    }

    /// Captures the identifier of `pattern` — group 1 when present, the whole
    /// match otherwise.
    fn capture_id<'a>(pattern: &Regex, haystack: &'a str) -> Option<&'a str> {
        pattern
            .captures(haystack)
            .and_then(|caps| caps.get(1).or_else(|| caps.get(0)))
            .map(|m| m.as_str())
    }

    /// Appends this tracker's hits for `message` to `out`.
    fn extract_into(&self, message: &str, out: &mut Vec<BugHit>) {
        for caps in self.footer_line.captures_iter(message) {
            let Some(value) = caps.get(1) else { continue };
            for token in value
                .as_str()
                .split(|c: char| c == ',' || c.is_whitespace())
            {
                let token = token.trim();
                if token.is_empty() {
                    continue;
                }
                if let Some(id) = Self::capture_id(&self.footer_value, token) {
                    out.push(BugHit {
                        tracker: self.name.clone(),
                        id: id.to_string(),
                    });
                }
            }
        }

        if let Some(inline) = &self.inline {
            for caps in inline.captures_iter(message) {
                let id = caps.get(1).or_else(|| caps.get(0));
                if let Some(id) = id {
                    out.push(BugHit {
                        tracker: self.name.clone(),
                        id: id.as_str().to_string(),
                    });
                }
            }
        }
    }
}

/// Extracts tickets from `message` using `trackers`.
///
/// Hits keep the configured tracker order; identifiers are sorted inside each
/// tracker and de-duplicated across all of them.
pub fn extract_bugs(message: &str, trackers: &[BugTrackerSpec]) -> Vec<BugHit> {
    let mut hits: Vec<BugHit> = Vec::new();
    for tracker in trackers {
        let mut tracker_hits: Vec<BugHit> = Vec::new();
        tracker.extract_into(message, &mut tracker_hits);
        // Sort inside the tracker to match the historical output order.
        tracker_hits.sort_by(|a, b| a.id.cmp(&b.id));
        for hit in tracker_hits {
            if !hits.contains(&hit) {
                hits.push(hit);
            }
        }
    }
    hits
}

/// The built-in tracker set used when the configuration defines none.
///
/// Reproduces the pre-`[bugs]` behaviour: Bugzilla-style footers
/// (`Bug:`/`Fixes:`/`Closes:` with optional `b/` prefix) plus inline `b/123`.
pub fn default_trackers() -> Vec<BugTrackerSpec> {
    vec![
        BugTrackerSpec::compile(
            "bugzilla",
            &["bug".to_string(), "fixes".to_string(), "closes".to_string()],
            r"(?i)(?:b/)?(\d+)",
            Some(r"(?i)\bb/(\d+)\b"),
        )
        .expect("built-in bugzilla tracker pattern must compile"),
    ]
}

/// Renders hits for tool output.
///
/// A single tracker keeps the historical flat form (`12345, 67890`); as soon
/// as several trackers produce hits, each group is prefixed with its name.
pub fn format_bugs(hits: &[BugHit]) -> String {
    let mut tracker_names: Vec<&str> = Vec::new();
    for hit in hits {
        if !tracker_names.contains(&hit.tracker.as_str()) {
            tracker_names.push(hit.tracker.as_str());
        }
    }

    if tracker_names.len() <= 1 {
        return hits
            .iter()
            .map(|h| h.id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
    }

    tracker_names
        .iter()
        .map(|name| {
            let ids = hits
                .iter()
                .filter(|h| h.tracker == *name)
                .map(|h| h.id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!("{name}: {ids}")
        })
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(hits: &[BugHit]) -> Vec<String> {
        hits.iter().map(|h| h.id.clone()).collect()
    }

    fn jira_tracker() -> BugTrackerSpec {
        BugTrackerSpec::compile(
            "jira",
            &["jira".to_string(), "ticket".to_string()],
            r"([A-Z][A-Z0-9]*-\d+)",
            None,
        )
        .unwrap()
    }

    // --- parity with the pre-`[bugs]` extractor -----------------------------

    #[test]
    fn default_tracker_reads_all_footer_prefixes() {
        let hits = extract_bugs(
            "Bug: 12345, 67890\nFixes: 11111\nSome text\nCloses: 22222",
            &default_trackers(),
        );
        assert_eq!(ids(&hits), vec!["11111", "12345", "22222", "67890"]);
    }

    #[test]
    fn default_tracker_reads_inline_mentions() {
        let hits = extract_bugs("Fixed b/12345 and also b/99999", &default_trackers());
        assert_eq!(ids(&hits), vec!["12345", "99999"]);
    }

    #[test]
    fn default_tracker_dedups_footer_and_inline() {
        let hits = extract_bugs("Bug: 12345\nFixes: 12345\nb/12345", &default_trackers());
        assert_eq!(ids(&hits), vec!["12345"]);
    }

    #[test]
    fn default_tracker_finds_nothing_in_plain_text() {
        assert!(extract_bugs("No bugs here.", &default_trackers()).is_empty());
    }

    #[test]
    fn default_tracker_strips_b_prefix_in_footer() {
        let hits = extract_bugs("Bug: b/67890", &default_trackers());
        assert_eq!(ids(&hits), vec!["67890"]);
    }

    #[test]
    fn default_tracker_splits_space_separated_footer() {
        let hits = extract_bugs("Bug: 1 2", &default_trackers());
        assert_eq!(ids(&hits), vec!["1", "2"]);
    }

    #[test]
    fn default_tracker_is_case_insensitive_inline() {
        let hits = extract_bugs("Fixed B/123", &default_trackers());
        assert_eq!(ids(&hits), vec!["123"]);
    }

    #[test]
    fn default_tracker_ignores_partial_word() {
        assert!(extract_bugs("Fixed fb/123", &default_trackers()).is_empty());
    }

    // --- configurable trackers ---------------------------------------------

    #[test]
    fn jira_tracker_reads_every_footer_line() {
        let msg = "parser: keep trailing newlines in quoted CSV fields\n\n\
                   Reported as (PROJ-1234, PROJ-5678):\n\
                   quoted fields lost their trailing newline.\n\n\
                   JIRA: PROJ-1234\nJIRA: PROJ-5678\n\n\
                   Change-Id: I0123456789abcdef0123456789abcdef01234567\n\
                   Signed-off-by: Example Developer <dev@example.com>";
        let hits = extract_bugs(msg, &[jira_tracker()]);
        assert_eq!(ids(&hits), vec!["PROJ-1234", "PROJ-5678"]);
    }

    #[test]
    fn jira_tracker_reads_comma_separated_footer_value() {
        let hits = extract_bugs("Ticket: PROJ-1, PROJ-2", &[jira_tracker()]);
        assert_eq!(ids(&hits), vec!["PROJ-1", "PROJ-2"]);
    }

    #[test]
    fn jira_tracker_ignores_service_footer_lines() {
        let msg = "Change-Id: I0123456789abcdef0123456789abcdef01234567\n\
                   Signed-off-by: Example Developer <dev@example.com>";
        assert!(extract_bugs(msg, &[jira_tracker()]).is_empty());
    }

    #[test]
    fn inline_disabled_does_not_pick_up_body_mentions() {
        let msg = "Cherry-picked from branch PROJ-1234-02\n\nJIRA: PROJ-9001";
        let hits = extract_bugs(msg, &[jira_tracker()]);
        assert_eq!(ids(&hits), vec!["PROJ-9001"]);
    }

    #[test]
    fn inline_enabled_picks_up_body_mentions() {
        let tracker = BugTrackerSpec::compile(
            "jira",
            &["jira".to_string()],
            r"([A-Z][A-Z0-9]*-\d+)",
            Some(r"\b([A-Z][A-Z0-9]*-\d+)\b"),
        )
        .unwrap();
        // A trailing "-02" is a branch suffix, not part of the ticket key.
        let hits = extract_bugs(
            "Cherry-picked from PROJ-1234-02\n\nJIRA: PROJ-9001",
            &[tracker],
        );
        assert_eq!(ids(&hits), vec!["PROJ-1234", "PROJ-9001"]);
    }

    #[test]
    fn several_trackers_keep_configuration_order() {
        let hits = extract_bugs(
            "Bug: 12345\nJIRA: PROJ-9001",
            &[default_trackers().remove(0), jira_tracker()],
        );
        assert_eq!(
            hits,
            vec![
                BugHit {
                    tracker: "bugzilla".into(),
                    id: "12345".into()
                },
                BugHit {
                    tracker: "jira".into(),
                    id: "PROJ-9001".into()
                },
            ]
        );
    }

    #[test]
    fn format_is_flat_for_a_single_tracker() {
        let hits = extract_bugs("Bug: 1 2", &default_trackers());
        assert_eq!(format_bugs(&hits), "1, 2");
    }

    #[test]
    fn format_labels_groups_for_several_trackers() {
        let hits = extract_bugs(
            "Bug: 12345\nJIRA: PROJ-1234\nJIRA: PROJ-5678",
            &[default_trackers().remove(0), jira_tracker()],
        );
        assert_eq!(
            format_bugs(&hits),
            "bugzilla: 12345; jira: PROJ-1234, PROJ-5678"
        );
    }

    #[test]
    fn capture_group_zero_is_used_when_pattern_has_no_groups() {
        let tracker =
            BugTrackerSpec::compile("plain", &["see".to_string()], r"#[0-9]+", None).unwrap();
        let hits = extract_bugs("See: #42", &[tracker]);
        assert_eq!(ids(&hits), vec!["#42"]);
    }

    // --- validation ---------------------------------------------------------

    #[test]
    fn invalid_footer_value_is_rejected_with_field_name() {
        let err = BugTrackerSpec::compile("x", &["bug".to_string()], "([0-9", None).unwrap_err();
        assert!(err.starts_with("footer_value:"), "unexpected error: {err}");
    }

    #[test]
    fn invalid_inline_is_rejected_with_field_name() {
        let err =
            BugTrackerSpec::compile("x", &["bug".to_string()], r"\d+", Some("([0-9")).unwrap_err();
        assert!(err.starts_with("inline:"), "unexpected error: {err}");
    }

    #[test]
    fn empty_footer_keys_are_rejected() {
        let err = BugTrackerSpec::compile("x", &[], r"\d+", None).unwrap_err();
        assert!(err.contains("footer_keys"), "unexpected error: {err}");
    }

    #[test]
    fn empty_name_is_rejected() {
        let err = BugTrackerSpec::compile("  ", &["bug".to_string()], r"\d+", None).unwrap_err();
        assert!(err.contains("name"), "unexpected error: {err}");
    }

    #[test]
    fn footer_keys_are_escaped_and_case_insensitive() {
        let tracker = BugTrackerSpec::compile("x", &["BUG #".to_string()], r"(\d+)", None).unwrap();
        let hits = extract_bugs("bug #: 7", &[tracker]);
        assert_eq!(ids(&hits), vec!["7"]);
    }

    // --- describe -----------------------------------------------------------

    #[test]
    fn describe_lists_footer_keys_and_inline_pattern() {
        let tracker = BugTrackerSpec::compile(
            "bugzilla",
            &["Bug".to_string(), "FIXES".to_string()],
            r"(?i)(\d+)",
            Some(r"(?i)\bb/(\d+)\b"),
        )
        .unwrap();
        assert_eq!(
            tracker.describe(),
            "bugzilla (footer: bug, fixes; inline: (?i)\\bb/(\\d+)\\b)"
        );
    }

    #[test]
    fn describe_omits_inline_when_not_configured() {
        assert_eq!(jira_tracker().describe(), "jira (footer: jira, ticket)");
    }

    // --- Yandex Tracker style (documented example) --------------------------

    /// Keys look like `QUEUE-123` — the same shape as Jira, so the example
    /// differs by restricting the pattern to the queues in use.
    fn yandex_tracker() -> BugTrackerSpec {
        BugTrackerSpec::compile(
            "yandex-tracker",
            &["tracker".to_string(), "yt".to_string()],
            r"((?:TREKER|BUGS)-\d+)",
            None,
        )
        .unwrap()
    }

    #[test]
    fn yandex_tracker_reads_configured_queues() {
        let hits = extract_bugs("Tracker: TREKER-42\nYT: BUGS-7", &[yandex_tracker()]);
        assert_eq!(ids(&hits), vec!["BUGS-7", "TREKER-42"]);
    }

    #[test]
    fn yandex_tracker_ignores_other_queues() {
        assert!(extract_bugs("Tracker: OTHER-1", &[yandex_tracker()]).is_empty());
    }

    #[test]
    fn three_trackers_are_grouped_by_name() {
        let hits = extract_bugs(
            "Bug: 12345\nJIRA: PROJ-1234\nTracker: TREKER-42",
            &[
                default_trackers().remove(0),
                jira_tracker(),
                yandex_tracker(),
            ],
        );
        assert_eq!(
            format_bugs(&hits),
            "bugzilla: 12345; jira: PROJ-1234; yandex-tracker: TREKER-42"
        );
    }
}
