use super::load_icons::{apps_signature, rasters_for};
use super::*;

/// Reads the apps again, unless the folders are as `known` says.
pub(super) fn reload_apps(known: Option<u64>) -> app::Task<Message> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                let signature = apps_signature();
                (known != Some(signature)).then(|| (signature, load_apps()))
            })
            .await
            .ok()
            .flatten()
        },
        |apps| match apps {
            Some((signature, apps)) => cosmic::action::app(Message::Apps(signature, Arc::new(apps))),
            None => cosmic::action::none(),
        },
    )
}

/// Which of `texts().categories` an app goes in, by its desktop entry.
fn category_of(categories: &[&str]) -> usize {
    let has = |names: &[&str]| categories.iter().any(|c| names.iter().any(|n| c.eq_ignore_ascii_case(n)));
    if has(&["Game"]) {
        5
    } else if has(&["Development", "IDE"]) {
        6
    } else if has(&["Chat", "InstantMessaging", "Email", "IRCClient", "Telephony", "VideoConference", "Feed"]) {
        1
    } else if has(&["Office", "WordProcessor", "Spreadsheet", "Presentation", "Calendar", "ContactManagement", "TextEditor", "Viewer"]) {
        0
    } else if has(&["WebBrowser", "Network", "FileTransfer", "P2P"]) {
        2
    } else if has(&["Graphics", "2DGraphics", "3DGraphics", "Photography", "VectorGraphics", "RasterGraphics"]) {
        3
    } else if has(&["AudioVideo", "Audio", "Video", "Player", "Music", "Recorder"]) {
        4
    } else if has(&["System", "Settings", "PackageManager", "Monitor", "TerminalEmulator", "HardwareSettings"]) {
        7
    } else if has(&["Utility", "Accessories", "FileManager", "Archiving", "Compression"]) {
        8
    } else {
        9
    }
}

/// The user's photo, where AccountsService or `~/.face` keep it.
pub(super) fn avatar() -> Option<PathBuf> {
    let user = std::env::var("USER").unwrap_or_default();
    let home = std::env::var("HOME").unwrap_or_default();
    [
        PathBuf::from(format!("/var/lib/AccountsService/icons/{user}")),
        Path::new(&home).join(".face"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

/// Lower case, without accents, to find "configuracoes" in "Configurações".
pub(super) fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            c => c,
        })
        .collect()
}

/// The languages to read names in, "pt_BR" then "pt": without the
/// ".UTF-8" that LANG has, which no desktop entry is translated to.
pub(super) fn locales() -> Vec<String> {
    let mut locales = Vec::new();
    let language = std::env::var("LANGUAGE").unwrap_or_default();
    let others = ["LC_ALL", "LC_MESSAGES", "LANG"].map(|v| std::env::var(v).unwrap_or_default());
    for locale in language.split(':').chain(others.iter().map(String::as_str)) {
        let locale = locale.split(['.', '@']).next().unwrap_or("");
        if locale.is_empty() || locale == "C" || locale == "POSIX" {
            continue;
        }
        for candidate in [locale, locale.split('_').next().unwrap_or(locale)] {
            if !locales.iter().any(|l| l == candidate) {
                locales.push(candidate.to_owned());
            }
        }
    }
    locales
}

pub(super) fn find_app<'a>(apps: &'a [AppEntry], id: &str) -> Option<&'a AppEntry> {
    let wanted = id.to_lowercase();
    apps.iter()
        .find(|a| a.id_lower == wanted)
        .or_else(|| apps.iter().find(|a| loosely(a, &wanted)))
}

/// The app of a window's `app_id`, if it has one.
pub(super) fn app_for_id<'a>(apps: &'a [AppEntry], app_id: &str) -> Option<&'a AppEntry> {
    if app_id.is_empty() { None } else { find_app(apps, app_id) }
}

/// The loose match of `find_app`: the window class, or an id ending in `.wanted`
/// (`wanted` in lower case).
fn loosely(app: &AppEntry, wanted: &str) -> bool {
    app.wm_class_lower.as_deref() == Some(wanted) || dotted_suffix(&app.id_lower, wanted)
}

/// Whether `hay` ends with `.wanted`, without building that text.
fn dotted_suffix(hay: &str, wanted: &str) -> bool {
    hay.ends_with(wanted)
        && hay.len() > wanted.len()
        && hay.as_bytes().get(hay.len() - wanted.len() - 1) == Some(&b'.')
}

/// Whether the pinned `item` is the app `app`, as `find_app` resolves it.
/// Whether a pinned entry is the same as `x`: the same text (a group too) or the same app.
pub(super) fn same_entry(apps: &[AppEntry], item: &str, x: &str) -> bool {
    item == x || same_app(apps, item, x)
}

fn same_app(apps: &[AppEntry], item: &str, app: &str) -> bool {
    find_app(apps, item).is_some_and(|a| a.id == app)
}

/// What the uninstall needs of an entry, read here, not in the view.
fn uninstall_facts(de: &fde::DesktopEntry, copies: &HashMap<String, usize>) -> uninstall::DesktopFacts {
    uninstall::DesktopFacts {
        appid: de.appid.clone(),
        path: de.path.clone(),
        exec: de.exec().map(str::to_owned),
        flatpak_ref: de.flatpak().map(str::to_owned),
        flatpak_part_of: de.desktop_entry("X-Flatpak-Part-Of").map(str::to_owned),
        shadows_other_data_dir: copies.get(de.id()).is_some_and(|n| *n > 1),
    }
}

fn load_apps() -> Vec<AppEntry> {
    let cpu = crate::backend::is_cpu();
    let locales = locales();
    let started = now();
    let desktop = fde::current_desktop();
    let mut seen = std::collections::HashSet::new();
    let entries: Vec<fde::DesktopEntry> = fde::Iter::new(fde::default_paths())
        .filter_map(|path| desktop_entry::read(&path, &locales).ok())
        .collect();
    // An id in more than one folder: the first shadows the others.
    let mut copies: HashMap<String, usize> = HashMap::new();
    for de in &entries {
        *copies.entry(de.id().to_owned()).or_default() += 1;
    }
    let mut apps: Vec<AppEntry> = entries
        .into_iter()
        .filter(|de| seen.insert(de.id().to_owned()))
        .filter(|de| !de.hidden() && !de.no_display() && de.exec().is_some())
        .filter(|de| de.type_().is_none_or(|t| t == "Application"))
        .filter(|de| {
            let Some(desktop) = &desktop else { return true };
            let shown = de
                .only_show_in()
                .is_none_or(|only| only.iter().any(|o| desktop.iter().any(|d| d.eq_ignore_ascii_case(o))));
            let hidden = de
                .not_show_in()
                .is_some_and(|not| not.iter().any(|n| desktop.iter().any(|d| d.eq_ignore_ascii_case(n))));
            shown && !hidden
        })
        .map(|de| {
            let name = de.name(&locales).map(|n| n.to_string()).unwrap_or_else(|| de.appid.clone());
            let mut haystack = vec![name.clone(), de.appid.clone()];
            haystack.extend(de.generic_name(&locales).map(|g| g.to_string()));
            haystack.extend(de.comment(&locales).map(|c| c.to_string()));
            haystack.extend(de.keywords(&locales).unwrap_or_default().into_iter().map(|k| k.to_string()));
            haystack.extend(de.exec().map(|e| e.split_whitespace().next().unwrap_or("").to_string()));
            let icon = fde::IconSource::from_unknown(de.icon().unwrap_or(&de.appid)).as_cosmic_icon();
            let rasters = rasters_for(&icon, cpu);
            let actions = de
                .actions()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|action| {
                    let name = de.action_entry_localized(action, "Name", &locales)?;
                    let exec = de.action_entry(action, "Exec")?;
                    Some((name.to_string(), exec.to_string()))
                })
                .collect();
            let facts = uninstall_facts(&de, &copies);
            let origin = uninstall::detect(&facts, &uninstall::RealFs);
            AppEntry {
                path: facts.path,
                flatpak_ref: facts.flatpak_ref,
                flatpak_part_of: facts.flatpak_part_of,
                shadows_other_data_dir: facts.shadows_other_data_dir,
                origin,
                id: de.appid.clone(),
                folded_name: fold(&name),
                haystack: fold(&haystack.join(" ")),
                name,
                id_lower: de.appid.to_lowercase(),
                wm_class_lower: de.startup_wm_class().map(str::to_lowercase),
                exec: de.exec().map(str::to_owned),
                terminal: de.terminal(),
                icon,
                rasters,
                actions,
                category: category_of(&de.categories().unwrap_or_default()),
                added: {
                    let (created, modified) = file_times(&de.path);
                    recent::pick_added(created, modified, started)
                },
            }
        })
        .collect();
    apps.sort_by(|a, b| a.folded_name.cmp(&b.folded_name));
    apps
}

pub(super) fn config() -> Option<Config> {
    Config::new(APP_ID, 1).ok()
}

/// What the menu kept when it was part of the Central, the first time.
pub fn migrate() {
    let Some(new) = config() else { return };
    if new.get::<Vec<String>>("pinned").is_ok() {
        return;
    }
    let Ok(old) = Config::new("io.github.Aenes0.CosmicCentral.Start", 1) else { return };
    if let Ok(pinned) = old.get::<Vec<String>>("pinned") {
        _ = new.set("pinned", pinned);
    }
    if let Ok(launched) = old.get::<Vec<(String, i64)>>("launched") {
        _ = new.set("launched", launched);
    }
    if let Ok(view) = old.get::<u8>("all_view") {
        _ = new.set("all_view", view);
    }
}

/// What was pinned; the first time, what the dock has, and Files, the
/// Terminal and Settings.
pub(super) fn load_pinned() -> Vec<String> {
    if let Some(pinned) = config().and_then(|c| c.get::<Vec<String>>("pinned").ok()) {
        return pinned;
    }
    let mut pinned: Vec<String> = Config::new("com.system76.CosmicAppList", 1)
        .ok()
        .and_then(|c| c.get::<Vec<String>>("favorites").ok())
        .unwrap_or_default();
    for id in ["com.system76.CosmicFiles", "com.system76.CosmicTerm", "com.system76.CosmicSettings", "com.system76.CosmicStore", "com.system76.CosmicEdit"] {
        if !pinned.iter().any(|p| p == id) {
            pinned.push(id.into());
        }
    }
    pinned
}

pub(super) fn save_pinned(pinned: &[String]) {
    if let Some(Err(err)) = config().map(|c| c.set("pinned", pinned.to_vec())) {
        tracing::warn!("cannot save the pinned apps: {err}");
    }
}

/// The user's groups, where the apps are and their order, as saved.
pub(super) fn load_groups() -> groups::Groups {
    let Some(c) = config() else {
        return groups::Groups::default();
    };
    let user = c.get::<Vec<(u32, String)>>("groups");
    let place = c.get::<Vec<(String, String)>>("group_of");
    let order = c.get::<Vec<(String, Vec<String>)>>("group_order");
    let next = c.get::<u32>("group_next");
    let readable = [
        readable(&user),
        readable(&place),
        readable(&order),
        readable(&next),
    ];
    if readable.contains(&false) {
        // What is saved may be fine and only unread: never write over it.
        tracing::warn!("cannot read the saved groups: they are kept as they are, not saved over");
        GROUPS_UNREADABLE.store(true, Ordering::Relaxed);
    }
    groups::Groups::from_stored(
        user.unwrap_or_default(),
        place.unwrap_or_default(),
        order.unwrap_or_default(),
    )
    .with_next(next.unwrap_or_default())
}

/// Set when the groups saved could not be read: saving would erase them.
static GROUPS_UNREADABLE: AtomicBool = AtomicBool::new(false);

/// Whether a key was read, or is only not there yet (both are fine).
fn readable<T>(read: &Result<T, cosmic::cosmic_config::Error>) -> bool {
    use cosmic::cosmic_config::Error;
    match read {
        Ok(_) | Err(Error::NotFound) => true,
        Err(Error::GetKey(_, io)) => io.kind() == std::io::ErrorKind::NotFound,
        Err(_) => false,
    }
}

pub(super) fn save_groups(g: &groups::Groups) {
    if GROUPS_UNREADABLE.load(Ordering::Relaxed) {
        return;
    }
    let (user, place, order) = g.to_stored();
    let Some(c) = config() else {
        return;
    };
    let saved = c
        .set("groups", user)
        .and(c.set("group_of", place))
        .and(c.set("group_order", order))
        .and(c.set("group_next", g.next));
    if let Err(err) = saved {
        tracing::warn!("cannot save the groups: {err}");
    }
}

/// Whether the panel is at the top, for the menu to open from there.
pub(super) fn panel_on_top() -> bool {
    let config = std::env::var("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(|h| std::path::Path::new(&h).join(".config")))
        .unwrap_or_default();
    std::fs::read_to_string(config.join("cosmic/com.system76.CosmicPanel.Panel/v1/anchor"))
        .is_ok_and(|anchor| anchor.trim() == "Top")
}

/// The user's full name, as the system has it, else the login.
pub(super) fn user_name() -> String {
    // SAFETY: getpwuid returns a pointer into static storage, read at once.
    let full = unsafe {
        let pw = libc::getpwuid(libc::getuid());
        if pw.is_null() || (*pw).pw_gecos.is_null() {
            None
        } else {
            std::ffi::CStr::from_ptr((*pw).pw_gecos)
                .to_str()
                .ok()
                .map(|g| g.split(',').next().unwrap_or("").trim().to_owned())
                .filter(|g| !g.is_empty())
        }
    };
    full.or_else(|| std::env::var("USER").ok()).unwrap_or_default()
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    pub(super) fn entry(id: &str, wm_class: Option<&str>) -> AppEntry {
        AppEntry {
            id: id.to_owned(),
            id_lower: id.to_lowercase(),
            name: String::new(),
            wm_class_lower: wm_class.map(str::to_lowercase),
            exec: None,
            terminal: false,
            icon: icon::from_name("x").handle(),
            rasters: AppRasters::default(),
            haystack: String::new(),
            folded_name: String::new(),
            actions: Vec::new(),
            category: 0,
            added: 0,
            path: PathBuf::new(),
            flatpak_ref: None,
            flatpak_part_of: None,
            shadows_other_data_dir: false,
            origin: uninstall::Origin::LooseDesktop,
        }
    }

    fn old_find_app<'a>(apps: &'a [AppEntry], id: &str) -> Option<&'a AppEntry> {
        let wanted = id.to_lowercase();
        apps.iter().find(|a| a.id.to_lowercase() == wanted).or_else(|| {
            apps.iter().find(|a| {
                a.wm_class_lower.as_deref().is_some_and(|c| c == wanted)
                    || a.id.to_lowercase().ends_with(&format!(".{wanted}"))
            })
        })
    }

    fn old_app_for_window<'a>(apps: &'a [AppEntry], app_id: &str) -> Option<&'a AppEntry> {
        let wanted = app_id.to_lowercase();
        if wanted.is_empty() {
            return None;
        }
        apps.iter().find(|a| a.id.to_lowercase() == wanted).or_else(|| {
            apps.iter().find(|a| {
                a.wm_class_lower.as_deref().is_some_and(|c| c == wanted)
                    || a.id.to_lowercase().rsplit('.').next() == Some(wanted.as_str())
                    || a.id.to_lowercase().ends_with(&format!(".{wanted}"))
            })
        })
    }

    #[test]
    fn the_loose_match_is_class_or_suffix_in_any_case() {
        assert!(loosely(&entry("org.gnome.Calculator", None), "calculator"));
        assert!(loosely(&entry("x.y", Some("Firefox")), "firefox"));
        assert!(!loosely(&entry("org.gnome.Calculator", None), "gnome"));
        assert!(!loosely(&entry("calculator", None), "calculator"));
    }

    #[test]
    fn dotted_suffix_matches_the_formatted_form() {
        let pairs = [
            ("org.gnome.calculator", "calculator"),
            ("calculator", "calculator"),
            ("acalculator", "calculator"),
            ("org.exemplo.configurações", "configurações"),
            ("x.ção", "ção"),
            ("xção", "ção"),
            ("", ""),
            ("abc", ""),
            ("abc.", ""),
            (".abc", "abc"),
            ("abc", "abc."),
        ];
        for (h, w) in pairs {
            assert_eq!(dotted_suffix(h, w), h.ends_with(&format!(".{w}")), "{h:?} {w:?}");
        }
    }

    #[test]
    fn find_app_and_app_for_window_match_the_old_ones() {
        let apps = [
            entry("org.gnome.Calculator", None),
            entry("firefox", Some("Firefox")),
            entry("acalculator", None),
            entry("com.Exemplo.Configurações", None),
            entry("steam_app_1", None),
            entry("x.y", Some("Navegador")),
        ];
        let queries = [
            "org.gnome.Calculator", "ORG.GNOME.CALCULATOR", "firefox", "FIREFOX", "navegador", "calculator",
            "Calculator", "configurações", "CONFIGURAÇÕES", "gnome", "org", "alculator", "acalculator", "", "y",
            "x.y", "nada",
        ];
        for q in queries {
            let id = |a: Option<&AppEntry>| a.map(|a| a.id.clone());
            assert_eq!(id(find_app(&apps, q)), id(old_find_app(&apps, q)), "find_app {q:?}");
            assert_eq!(id(app_for_id(&apps, q)), id(old_app_for_window(&apps, q)), "app_for_id {q:?}");
        }
        let id = |q| find_app(&apps, q).map(|a| a.id.as_str());
        assert_eq!(id("calculator"), Some("org.gnome.Calculator"));
        assert_eq!(id("acalculator"), Some("acalculator"));
        assert_eq!(id("gnome"), None);
        assert_eq!(id("navegador"), Some("x.y"));
    }

    #[test]
    fn the_same_group_is_not_pinned_twice() {
        assert!(same_entry(&[], "group:cat:games", "group:cat:games"));
        assert!(!same_entry(&[], "group:cat:games", "group:cat:social"));
    }

    #[test]
    fn a_missing_key_is_readable_a_broken_one_is_not() {
        use cosmic::cosmic_config::Error;
        let io = |k| std::io::Error::from(k);
        assert!(readable::<u32>(&Ok(1)));
        assert!(readable::<u32>(&Err(Error::NotFound)));
        assert!(readable::<u32>(&Err(Error::GetKey("k".into(), io(std::io::ErrorKind::NotFound)))));
        assert!(!readable::<u32>(&Err(Error::GetKey("k".into(), io(std::io::ErrorKind::InvalidData)))));
        assert!(!readable::<u32>(&Err(Error::Io(io(std::io::ErrorKind::PermissionDenied)))));
    }
}
