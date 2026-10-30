// SPDX-License-Identifier: GPL-3.0-only
//! Parsing of a `.desktop` `Exec` value.

/// An `Exec` split into words, with the leading variables apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedExec {
    pub vars: Vec<(String, String)>,
    pub argv: Vec<String>,
}

/// Undoes the `.desktop` string escapes (`\\`, `\s`, `\n`, `\t`, `\r`).
fn unescape_desktop(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// `NAME=value` with a plain variable name.
fn split_assignment(tok: &str) -> Option<(String, String)> {
    let (k, v) = tok.split_once('=')?;
    let valid = !k.is_empty()
        && !k.starts_with(|c: char| c.is_ascii_digit())
        && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    valid.then(|| (k.to_string(), v.to_string()))
}

/// Splits an `Exec` like a shell would (no expansion), after the `.desktop`
/// unescape, and sets aside `env` and the leading `VAR=value` words.
/// `None` when it cannot be parsed or has no program.
pub fn parse_exec(exec: &str) -> Option<ParsedExec> {
    let mut words = shlex::split(&unescape_desktop(exec))?
        .into_iter()
        .peekable();
    let mut vars = Vec::new();
    if matches!(
        words.peek().map(String::as_str),
        Some("env" | "/usr/bin/env")
    ) {
        words.next();
    }
    while let Some(w) = words.peek() {
        match split_assignment(w) {
            Some(kv) => {
                vars.push(kv);
                words.next();
            }
            None => break,
        }
    }
    let argv: Vec<String> = words.collect();
    (!argv.is_empty()).then_some(ParsedExec { vars, argv })
}
