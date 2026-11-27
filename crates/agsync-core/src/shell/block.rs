//! The hook block inside a profile file, between two fixed marker lines.
//!
//! The markers are `#` comment lines in the user's file, the comment syntax of every
//! supported shell. They make install idempotent and removal exact.

pub const BEGIN: &str = "# >>> agsync hook >>>";
pub const END: &str = "# <<< agsync hook <<<";

/// `text` with the block holding `body`: replaced in place when present, else appended.
/// Keeps the file's line ending (`\r\n` when it already uses it).
pub fn with_block(text: &str, body: &str) -> String {
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let block: String = [BEGIN, body.trim_end(), END]
        .iter()
        .flat_map(|part| part.lines())
        .map(|line| format!("{line}{eol}"))
        .collect();
    if let Some((before, after)) = split(text) {
        return format!("{before}{block}{after}");
    }
    let mut out = text.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push_str(eol);
    }
    out.push_str(&block);
    out
}

/// `text` without the block; every other byte kept.
pub fn without_block(text: &str) -> String {
    match split(text) {
        Some((before, after)) => format!("{before}{after}"),
        None => text.to_string(),
    }
}

pub fn has_block(text: &str) -> bool {
    split(text).is_some()
}

/// Text before the BEGIN line and after the END line (line endings included).
fn split(text: &str) -> Option<(&str, &str)> {
    let start = line_start(text, BEGIN)?;
    let end_line = start + line_start(&text[start..], END)?;
    let after = match text[end_line..].find('\n') {
        Some(i) => end_line + i + 1,
        None => text.len(),
    };
    Some((&text[..start], &text[after..]))
}

/// Byte offset of the first line equal to `marker` (ignoring a trailing `\r`).
fn line_start(text: &str, marker: &str) -> Option<usize> {
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']) == marker {
            return Some(offset);
        }
        offset += line.len();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_once_and_replaces_in_place() {
        let user = "alias ll='ls -l'\nexport A=1";
        let one = with_block(user, "run hook\n");
        assert_eq!(
            one,
            "alias ll='ls -l'\nexport A=1\n# >>> agsync hook >>>\nrun hook\n# <<< agsync hook <<<\n"
        );
        assert_eq!(with_block(&one, "run hook"), one);
        let tail = format!("{one}echo after\n");
        let moved = with_block(&tail, "new hook");
        assert!(
            moved.ends_with("# >>> agsync hook >>>\nnew hook\n# <<< agsync hook <<<\necho after\n")
        );
        assert_eq!(moved.matches(BEGIN).count(), 1);
    }

    #[test]
    fn removal_restores_the_rest_exactly() {
        let user = "a\r\nb\r\n";
        let with = with_block(user, "x");
        assert_eq!(
            with,
            "a\r\nb\r\n# >>> agsync hook >>>\r\nx\r\n# <<< agsync hook <<<\r\n"
        );
        assert!(has_block(&with));
        assert_eq!(without_block(&with), user);
        assert_eq!(without_block(user), user);
        assert_eq!(without_block(&with_block("", "x")), "");
    }
}
