//! Shared `<MEMORY>…</MEMORY>` extractor for the knowledge-capture observers.
//!
//! Two observers capture the *intentional* knowledge an actor marked for the
//! future via a `<MEMORY>…</MEMORY>` block:
//!
//! - [`super::agent_summary_observer`] — a dispatched subagent's terminal
//!   output (`PostToolUse(Task)`).
//! - [`super::session_stop_observer`] — the **orchestrator's** own final output
//!   on the main-session `Stop`, the only capture point for light, direct work
//!   (`/task`/bugfix) that never dispatches a subagent.
//!
//! Both must read the *same* convention, so the extractor lives once here and
//! both call it — one extractor, no drift (SOLID).

/// Extract every `<MEMORY>…</MEMORY>` block body from `text`, trimmed and
/// non-empty, in source order.
///
/// The convention is a literal `<MEMORY>` open tag and a literal `</MEMORY>`
/// close tag. A block whose body is empty/whitespace-only is dropped (it
/// carries nothing). An unterminated open tag ends the scan. Pure and total —
/// never panics, allocates only the captured bodies.
#[must_use]
pub(crate) fn extract_memory_blocks(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find("<MEMORY>") {
        let after = &rest[open + "<MEMORY>".len()..];
        let Some(close) = after.find("</MEMORY>") else {
            break;
        };
        let body = after[..close].trim();
        if !body.is_empty() {
            out.push(body.to_string());
        }
        rest = &after[close + "</MEMORY>".len()..];
    }
    out
}

/// Extract the **first** `<MEMORY>…</MEMORY>` block body, trimmed. `None` when
/// absent or empty. Convenience over [`extract_memory_blocks`] for the
/// single-block subagent path.
#[must_use]
pub(crate) fn extract_memory_block(text: &str) -> Option<String> {
    extract_memory_blocks(text).into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_block_round_trips() {
        let text = "blurb\n<MEMORY>\nKey insight here.\nLine two.\n</MEMORY>\nmore";
        let body = extract_memory_block(text).unwrap();
        assert!(body.contains("Key insight"));
        assert!(body.contains("Line two"));
    }

    #[test]
    fn absent_returns_none_and_empty() {
        assert!(extract_memory_block("no marker here").is_none());
        assert!(extract_memory_blocks("no marker here").is_empty());
    }

    #[test]
    fn empty_block_is_dropped() {
        assert!(extract_memory_block("<MEMORY>   \n\t </MEMORY>").is_none());
        assert!(extract_memory_blocks("<MEMORY></MEMORY>").is_empty());
    }

    #[test]
    fn multiple_blocks_are_all_captured_in_order() {
        let text = "<MEMORY>first</MEMORY> middle <MEMORY>second</MEMORY>";
        let blocks = extract_memory_blocks(text);
        assert_eq!(blocks, vec!["first".to_string(), "second".to_string()]);
    }

    #[test]
    fn unterminated_open_tag_ends_scan() {
        let text = "<MEMORY>captured</MEMORY> tail <MEMORY>never closed";
        let blocks = extract_memory_blocks(text);
        assert_eq!(blocks, vec!["captured".to_string()]);
    }
}
