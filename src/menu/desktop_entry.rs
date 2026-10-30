// SPDX-License-Identifier: GPL-3.0-only

//! Reading desktop entries, also those the reader of the crate refuses.
//!
//! `freedesktop-desktop-entry` keeps aside a localized key that does not
//! follow its default (`Keywords[pt_BR]=` above `Keywords=`) and looks for the
//! default when the next group begins, before it has stored the group's last
//! key. When that last key is the one translated and a group follows, as in
//! Mousepad's entry, the reader fails with `KeyDoesNotExist` and the menu
//! would drop the app. The specification does not fix the order of the keys,
//! so such an entry is valid: it is read again with each group's default
//! keys first.

use std::path::Path;

use cosmic::desktop::fde;

/// Reads the entry in the languages given; if the crate refuses it, once
/// more with the keys of each group put in the order it expects.
pub fn read(path: &Path, locales: &[String]) -> Result<fde::DesktopEntry, String> {
    match fde::DesktopEntry::from_path(path, Some(locales)) {
        Ok(entry) => Ok(entry),
        Err(first) => {
            let Ok(text) = std::fs::read_to_string(path) else {
                return Err(first.to_string());
            };
            fde::DesktopEntry::from_str(path, &reorder(&text), Some(locales))
                .map_err(|_| first.to_string())
        }
    }
}

/// Whether the line is `Key[locale]=value`.
fn is_localized(line: &str) -> bool {
    let Some((key, _)) = line.split_once('=') else {
        return false;
    };
    key.trim_end().ends_with(']') && key.contains('[')
}

/// The same text, each group with its default keys before the localized ones.
fn reorder(text: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut localized: Vec<&str> = Vec::new();
    for line in text.lines() {
        if line.trim_start().starts_with('[') && !is_localized(line) {
            out.append(&mut localized);
            out.push(line);
        } else if is_localized(line) {
            localized.push(line);
        } else {
            out.push(line);
        }
    }
    out.append(&mut localized);
    let mut joined = out.join("\n");
    joined.push('\n');
    joined
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// Mousepad's shape: the localized keywords above the default, the
    /// default being the last key of the main group, and a group of actions
    /// after it.
    const MOUSEPAD_LIKE: &str = "[Desktop Entry]\nVersion=1.0\nName=Mousepad\nName[pt_BR]=Mousepad\n\
        Exec=mousepad %U\nType=Application\nActions=preferences;\nKeywords[pt_BR]=Editar;Texto;R\u{e1}pido;\n\
        Keywords[pt]=Editar;Texto;R\u{e1}pido;\nKeywords=Edit;Text;Fast;\n\n\
        [Desktop Action preferences]\nName=Mousepad Preferences\nName[pt_BR]=Prefer\u{ea}ncias\n\
        Exec=mousepad --preferences\n";

    fn locales() -> Vec<String> {
        vec!["pt_BR".to_owned(), "pt".to_owned()]
    }

    fn temp_entry(name: &str, text: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("qs-entry-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        path
    }

    /// Removes the entry and the directory made for it.
    fn cleanup(path: &Path) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn an_unreadable_file_reports_the_crates_own_error() {
        let path =
            std::env::temp_dir().join(format!("qs-entry-{}-missing.desktop", std::process::id()));
        let crate_error = fde::DesktopEntry::from_path(&path, Some(&locales()))
            .unwrap_err()
            .to_string();
        assert_eq!(read(&path, &locales()).unwrap_err(), crate_error);
    }

    #[test]
    fn the_crate_alone_refuses_the_mousepad_shape() {
        let path = temp_entry("crate-alone.desktop", MOUSEPAD_LIKE);
        let result = fde::DesktopEntry::from_path(&path, Some(&locales()));
        cleanup(&path);
        assert!(
            result.is_err(),
            "if the crate is fixed, the fallback can go"
        );
    }

    #[test]
    fn the_shape_the_crate_refuses_is_read_with_its_keys_in_place() {
        let path = temp_entry("reordered.desktop", MOUSEPAD_LIKE);
        let entry = read(&path, &locales()).expect("must read");
        cleanup(&path);
        assert_eq!(entry.exec(), Some("mousepad %U"));
        assert_eq!(entry.name(&locales()).as_deref(), Some("Mousepad"));
        let keywords = entry.keywords(&locales()).unwrap_or_default();
        assert!(
            keywords.iter().any(|k| k == "R\u{e1}pido"),
            "localized keywords kept: {keywords:?}"
        );
        assert!(entry.actions().unwrap_or_default().contains(&"preferences"));
    }

    #[test]
    fn an_entry_that_was_fine_is_read_as_before() {
        let text =
            "[Desktop Entry]\nName=Calc\nExec=calc\nType=Application\nName[pt]=Calculadora\n";
        let path = temp_entry("calc.desktop", text);
        let entry = read(&path, &locales()).expect("must read");
        cleanup(&path);
        assert_eq!(entry.name(&locales()).as_deref(), Some("Calculadora"));
    }

    #[test]
    fn what_is_not_an_entry_stays_an_error() {
        let path = temp_entry("broken.desktop", "this is not a desktop entry");
        let result = read(&path, &locales());
        cleanup(&path);
        assert!(result.is_err());
    }

    #[test]
    fn reordering_keeps_every_line_and_the_group_it_belongs_to() {
        let text = "[A]\nk[pt]=2\nk=1\n[B]\nj[pt]=4\nj=3\n";
        assert_eq!(reorder(text), "[A]\nk=1\nk[pt]=2\n[B]\nj=3\nj[pt]=4\n");
    }

    #[test]
    fn a_group_header_is_not_mistaken_for_a_localized_key() {
        assert!(!is_localized("[Desktop Action preferences]"));
        assert!(is_localized("Keywords[pt_BR]=a;b;"));
        assert!(!is_localized("Keywords=a;b;"));
    }
}
