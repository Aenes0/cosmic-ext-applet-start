use super::*;

/// The recommended: the `head` first, then the rest, the latest first.
pub(super) fn put_first<T>(head: Vec<T>, mut rest: Vec<(i64, T)>) -> Vec<(i64, T)> {
    rest.sort_by_key(|(when, _)| -when);
    head.into_iter().map(|h| (i64::MAX, h)).chain(rest).collect()
}

/// When the file was made and last changed, in seconds, as far as known.
pub(super) fn file_times(path: &Path) -> (Option<i64>, Option<i64>) {
    let secs = |time: std::io::Result<SystemTime>| {
        time.ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
    };
    match std::fs::metadata(path) {
        Ok(m) => (secs(m.created()), secs(m.modified())),
        Err(_) => (None, None),
    }
}

pub(super) fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// "5 min ago", "3h ago", "Yesterday", as Windows says it.
pub(super) fn ago(when: i64) -> String {
    let t = texts();
    let seconds = (now() - when).max(0);
    match seconds {
        0..60 => t.just_now.to_owned(),
        60..3600 => t.minutes_ago.replace("{}", &(seconds / 60).to_string()),
        3600..86400 => t.hours_ago.replace("{}", &(seconds / 3600).to_string()),
        86400..172800 => t.yesterday.to_owned(),
        _ => t.days_ago.replace("{}", &(seconds / 86400).to_string()),
    }
}

/// The files changed lately: the ones apps say they opened
/// (`recently-used.xbel`, also the Flatpaks'), and the newest in the
/// usual folders, which COSMIC's apps don't say.
pub(super) fn load_recents() -> Vec<RecentFile> {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let mut found: HashMap<String, RecentFile> = HashMap::new();
    let mut add = |file: RecentFile| {
        if found.get(&file.path).is_none_or(|old| old.when < file.when) {
            found.insert(file.path.clone(), file);
        }
    };

    let mut xbels = vec![home.join(".local/share/recently-used.xbel")];
    if let Ok(apps) = std::fs::read_dir(home.join(".var/app")) {
        xbels.extend(apps.flatten().map(|a| a.path().join("data/recently-used.xbel")));
    }
    for xbel in xbels {
        let Ok(text) = std::fs::read_to_string(&xbel) else { continue };
        for bookmark in text.split("<bookmark ").skip(1) {
            let attr = |name: &str| {
                let start = bookmark.find(&format!("{name}=\""))? + name.len() + 2;
                let end = bookmark[start..].find('"')? + start;
                Some(&bookmark[start..end])
            };
            let Some(path) = attr("href").and_then(|h| h.strip_prefix("file://")).map(unescape) else {
                continue;
            };
            let when = ["visited", "modified", "added"]
                .iter()
                .filter_map(|a| attr(a))
                .filter_map(|d| chrono::DateTime::parse_from_rfc3339(d).ok())
                .map(|d| d.timestamp())
                .max()
                .unwrap_or(0);
            let mime = bookmark
                .find("mime-type type=\"")
                .map(|i| &bookmark[i + 16..])
                .and_then(|rest| rest.split('"').next());
            if Path::new(&path).is_file() {
                add(recent(&path, when, mime));
            }
        }
    }

    // The newest in the user's folders, and a level down (Screenshots…).
    let cutoff = now() - 30 * 86400;
    for dir in user_dirs(&home) {
        let mut stack = vec![(dir, 0)];
        while let Some((dir, depth)) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                if entry.file_name().to_string_lossy().starts_with('.') {
                    continue;
                }
                let Ok(meta) = entry.metadata() else { continue };
                if meta.is_dir() {
                    if depth < 1 {
                        stack.push((path, depth + 1));
                    }
                    continue;
                }
                let when = meta
                    .modified()
                    .ok()
                    .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs() as i64);
                if when > cutoff {
                    add(recent(&path.to_string_lossy(), when, None));
                }
            }
        }
    }
    let mut files: Vec<RecentFile> = found.into_values().collect();
    files.sort_by_key(|f| -f.when);
    files.truncate(40);
    files
}

fn recent(path: &str, when: i64, mime: Option<&str>) -> RecentFile {
    let name = Path::new(path)
        .file_name()
        .map_or_else(|| path.to_owned(), |n| n.to_string_lossy().into_owned());
    let extension = Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let generic = match extension.as_str() {
        "pdf" => "application-pdf",
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" | "bmp" | "heic" | "avif" => "image-x-generic",
        "mp4" | "mkv" | "webm" | "mov" | "avi" => "video-x-generic",
        "mp3" | "flac" | "ogg" | "opus" | "wav" | "m4a" => "audio-x-generic",
        "odt" | "doc" | "docx" | "rtf" => "x-office-document",
        "ods" | "xls" | "xlsx" | "csv" => "x-office-spreadsheet",
        "odp" | "ppt" | "pptx" => "x-office-presentation",
        "zip" | "tar" | "gz" | "xz" | "zst" | "7z" | "rar" | "flatpakref" | "deb" | "rpm" => "package-x-generic",
        "html" | "htm" => "text-html",
        "sh" | "py" | "rs" | "js" | "ts" | "c" | "cpp" | "h" => "text-x-script",
        _ => "text-x-generic",
    };
    let mut icons = Vec::new();
    if let Some(mime) = mime {
        icons.push(mime.replace('/', "-"));
    }
    icons.push(generic.to_owned());
    icons.push("text-x-generic".to_owned());
    RecentFile { path: path.to_owned(), name, icons, when }
}

/// "%20" back to " ", as the bookmarks keep paths.
fn unescape(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(byte) = text.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Desktop, Downloads, Documents, Pictures, Videos and Music, as
/// `user-dirs.dirs` names them in the user's language.
fn user_dirs(home: &Path) -> Vec<PathBuf> {
    let config = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home.join(".config"));
    let text = std::fs::read_to_string(config.join("user-dirs.dirs")).unwrap_or_default();
    let mut dirs: Vec<PathBuf> = text
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            let wanted = ["DESKTOP", "DOWNLOAD", "DOCUMENTS", "PICTURES", "VIDEOS", "MUSIC"];
            if !wanted.iter().any(|w| key.trim() == format!("XDG_{w}_DIR")) {
                return None;
            }
            let value = value.trim().trim_matches('"').replace("$HOME", &home.to_string_lossy());
            let path = PathBuf::from(value);
            (path != home).then_some(path)
        })
        .collect();
    if dirs.is_empty() {
        dirs = ["Desktop", "Downloads", "Documents", "Pictures"].iter().map(|d| home.join(d)).collect();
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_newest_come_first_and_the_rest_by_recency() {
        let rest = vec![(10, "old"), (30, "new"), (20, "mid")];
        let all = put_first(vec!["a", "b"], rest);
        let names: Vec<&str> = all.iter().map(|(_, n)| *n).collect();
        assert_eq!(names, ["a", "b", "new", "mid", "old"]);
    }
}
