// SPDX-License-Identifier: GPL-3.0-only
use super::*;
use crate::texts::texts;
use super::run::run_with;
use std::cell::RefCell;
use std::collections::HashSet;

const HOME: &str = "/home/u";

#[derive(Default)]
struct FakeFs {
    present: HashSet<PathBuf>,
    regular: HashSet<PathBuf>,
    owned: HashSet<PathBuf>,
    no_home: bool,
}

impl FakeFs {
    fn with(mut self, p: &str) -> Self {
        self.present.insert(p.into());
        self
    }
    fn file(mut self, p: &str) -> Self {
        self.present.insert(p.into());
        self.regular.insert(p.into());
        self
    }
    fn mine(mut self, p: &str) -> Self {
        self.owned.insert(p.into());
        self
    }
}

fn lexical(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

impl Fs for FakeFs {
    fn exists(&self, p: &Path) -> bool {
        self.present.contains(p)
    }
    fn is_regular_nolink(&self, p: &Path) -> bool {
        self.regular.contains(p)
    }
    fn canonicalize(&self, p: &Path) -> Option<PathBuf> {
        let c = lexical(p);
        (self.present.contains(&c) || self.present.contains(p)).then_some(c)
    }
    fn home(&self) -> Option<PathBuf> {
        (!self.no_home).then(|| HOME.into())
    }
    fn owned_by_me(&self, p: &Path) -> bool {
        self.owned.contains(p)
    }
}

type Handler = Box<dyn Fn(&str, &[String]) -> Result<Out, String>>;

struct FakeRunner {
    calls: RefCell<Vec<String>>,
    handler: Handler,
}

impl FakeRunner {
    fn new(h: impl Fn(&str, &[String]) -> Result<Out, String> + 'static) -> Self {
        Self {
            calls: RefCell::new(vec![]),
            handler: Box::new(h),
        }
    }
    fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}

impl Runner for FakeRunner {
    fn run(&self, program: &str, args: &[String]) -> Result<Out, String> {
        self.calls
            .borrow_mut()
            .push(format!("{program} {}", args.join(" ")));
        (self.handler)(program, args)
    }
}

fn ok(s: &str) -> Result<Out, String> {
    Ok(Out {
        ok: true,
        stdout: s.into(),
        stderr: String::new(),
    })
}
fn fail(s: &str) -> Result<Out, String> {
    Ok(Out {
        ok: false,
        stdout: String::new(),
        stderr: s.into(),
    })
}

fn facts(appid: &str, path: &str, exec: Option<&str>) -> DesktopFacts {
    DesktopFacts {
        appid: appid.into(),
        path: path.into(),
        exec: exec.map(String::from),
        ..Default::default()
    }
}

fn loose(exec: &str) -> DesktopFacts {
    facts(
        "foo",
        "/home/u/.local/share/applications/foo.desktop",
        Some(exec),
    )
}

// ---- parse_exec ----

#[test]
fn parse_exec_table() {
    let p = parse_exec(r#"env "WINEPREFIX=/home/u/.wine" wine "C:\\\\a b\\x.lnk""#).unwrap();
    assert_eq!(
        p.vars,
        vec![("WINEPREFIX".to_string(), "/home/u/.wine".to_string())]
    );
    assert_eq!(
        p.argv,
        vec!["wine".to_string(), r"C:\a b\x.lnk".to_string()]
    );
    let p = parse_exec("A=1 B=2 foo --x").unwrap();
    assert_eq!(p.vars.len(), 2);
    assert_eq!(p.argv, vec!["foo".to_string(), "--x".to_string()]);
    let p = parse_exec(r#""/opt/My App/run" %U"#).unwrap();
    assert_eq!(p.argv[0], "/opt/My App/run");
    assert!(parse_exec(r#"foo "unclosed"#).is_none());
    assert!(parse_exec("").is_none());
    assert!(parse_exec("env A=1").is_none());
}

// ---- detect ----

#[test]
fn protected_ids() {
    let fs = FakeFs::default();
    for id in [
        "io.github.Aenes0.CosmicStart",
        "cosmic-edit",
        "com.system76.CosmicStore",
    ] {
        let f = facts(id, "/usr/share/applications/x.desktop", None);
        assert_eq!(detect(&f, &fs), Origin::Protected, "{id}");
    }
}

#[test]
fn flatpak_scope_by_data_dir() {
    let r = "com.x.App";
    let mut f = facts(
        r,
        "/var/lib/flatpak/exports/share/applications/com.x.App.desktop",
        None,
    );
    f.flatpak_ref = Some(r.into());
    let user = FakeFs::default().with("/home/u/.local/share/flatpak/app/com.x.App");
    assert_eq!(
        detect(&f, &user),
        Origin::Flatpak {
            reference: r.into(),
            system: false
        }
    );
    let sys = FakeFs::default().with("/var/lib/flatpak/app/com.x.App");
    assert_eq!(
        detect(&f, &sys),
        Origin::Flatpak {
            reference: r.into(),
            system: true
        }
    );
    assert_eq!(detect(&f, &FakeFs::default()), Origin::Unknown);
}

#[test]
fn override_hides_flatpak_not_loose() {
    let mut f = facts(
        "com.x.App",
        "/home/u/.local/share/applications/com.x.App.desktop",
        Some("x"),
    );
    f.flatpak_ref = Some("com.x.App".into());
    f.shadows_other_data_dir = true;
    let fs = FakeFs::default().with("/var/lib/flatpak/app/com.x.App");
    assert_eq!(
        detect(&f, &fs),
        Origin::Flatpak {
            reference: "com.x.App".into(),
            system: true
        }
    );
}

#[test]
fn shadowing_loose_entry_is_unknown() {
    let mut f = loose("foo");
    f.shadows_other_data_dir = true;
    assert_eq!(detect(&f, &FakeFs::default()), Origin::Unknown);
}

#[test]
fn flatpak_part_of_never_removes_parent() {
    let mut f = facts(
        "brave-x",
        "/home/u/.local/share/applications/brave-x.desktop",
        Some("x"),
    );
    f.flatpak_ref = Some("com.brave.Browser".into());
    f.flatpak_part_of = Some("com.brave.Browser".into());
    let fs = FakeFs::default().with("/home/u/.local/share/flatpak/app/com.brave.Browser");
    assert_eq!(detect(&f, &fs), Origin::Unknown);
}

#[test]
fn flatpak_ref_with_leading_dash_is_unknown() {
    let mut f = facts("x", "/x.desktop", None);
    f.flatpak_ref = Some("--all".into());
    assert_eq!(detect(&f, &FakeFs::default()), Origin::Unknown);
}

#[test]
fn steam_ids() {
    let fs = FakeFs::default();
    let f = loose("steam steam://rungameid/12345");
    assert_eq!(
        detect(&f, &fs),
        Origin::Steam {
            app_id: "12345".into()
        }
    );
    assert_eq!(
        detect(&loose("steam steam://rungameid/12a"), &fs),
        Origin::Unknown
    );
    assert_eq!(
        detect(&loose("steam steam://rungameid/"), &fs),
        Origin::Unknown
    );
}

#[test]
fn waydroid_rules() {
    let fs = FakeFs::default();
    let f = loose("waydroid app launch com.foo.bar");
    assert_eq!(
        detect(&f, &fs),
        Origin::Waydroid {
            package: "com.foo.bar".into()
        }
    );
    for bad in ["com.android.settings", "org.lineageos.jelly", "-x", "a;b"] {
        let f = loose(&format!("waydroid app launch {bad}"));
        assert_eq!(detect(&f, &fs), Origin::Unknown, "{bad}");
    }
}

#[test]
fn wine_needs_valid_prefix() {
    let exec = r#"env "WINEPREFIX=/home/u/.wine" wine "C:\\\\a.lnk""#;
    let good = FakeFs::default()
        .with("/home/u/.wine/system.reg")
        .with("/home/u/.wine")
        .mine("/home/u/.wine");
    assert_eq!(
        detect(&loose(exec), &good),
        Origin::Wine {
            prefix: "/home/u/.wine".into()
        }
    );
    let no_reg = FakeFs::default()
        .with("/home/u/.wine")
        .mine("/home/u/.wine");
    assert_eq!(detect(&loose(exec), &no_reg), Origin::Unknown);
    let not_mine = FakeFs::default()
        .with("/home/u/.wine")
        .with("/home/u/.wine/system.reg");
    assert_eq!(detect(&loose(exec), &not_mine), Origin::Unknown);
    // WINEPREFIX without wine
    let f = loose("env WINEPREFIX=/home/u/.wine proton run x");
    assert_eq!(detect(&f, &good), Origin::Unknown);
    // relative prefix
    let f = loose("env WINEPREFIX=rel wine x");
    assert_eq!(detect(&f, &good), Origin::Unknown);
}

#[test]
fn appimage_rules() {
    let img = "/home/u/Apps/My App.AppImage";
    let fs = FakeFs::default().file(img).with("/home/u");
    let f = loose(r#""/home/u/Apps/My App.AppImage" --no-sandbox %U"#);
    assert_eq!(detect(&f, &fs), Origin::AppImage { file: img.into() });
    // outside HOME
    let out = FakeFs::default().file("/opt/x.AppImage").with("/home/u");
    assert_eq!(
        detect(&loose("/opt/x.AppImage"), &out),
        Origin::LooseDesktop
    );
    // dot-dot escape
    let esc = FakeFs::default().file("/x.appimage").with("/home/u");
    assert_eq!(
        detect(&loose("/home/u/../../x.appimage"), &esc),
        Origin::LooseDesktop
    );
    // symlink: present but not regular
    let link = FakeFs::default().with("/home/u/l.AppImage").with("/home/u");
    assert_eq!(
        detect(&loose("/home/u/l.AppImage"), &link),
        Origin::LooseDesktop
    );
    // token starting with dash
    let dash = FakeFs::default()
        .file("/home/u/-x.AppImage")
        .with("/home/u");
    // only argv[0] counts
    assert_eq!(
        detect(&loose("app /home/u/-x.AppImage"), &dash),
        Origin::LooseDesktop
    );
    assert_eq!(
        detect(&loose("/home/u/-x.AppImage"), &dash),
        Origin::AppImage {
            file: "/home/u/-x.AppImage".into()
        }
    );
}

#[test]
fn pacman_and_loose() {
    let fs = FakeFs::default();
    let f = facts(
        "firefox",
        "/usr/share/applications/firefox.desktop",
        Some("firefox %u"),
    );
    assert_eq!(detect(&f, &fs), Origin::Pacman);
    let f = facts("x", "/usr/share/../../etc/x.desktop", None);
    assert_eq!(detect(&f, &fs), Origin::Unknown);
    assert_eq!(detect(&loose("foo --bar"), &fs), Origin::LooseDesktop);
    let nohome = FakeFs {
        no_home: true,
        ..Default::default()
    };
    assert_eq!(detect(&loose("foo"), &nohome), Origin::Unknown);
    let other = facts("x", "/opt/x.desktop", Some("x"));
    assert_eq!(detect(&other, &fs), Origin::Unknown);
}

#[test]
fn wrappers_are_not_loose() {
    let fs = FakeFs::default();
    for e in [
        "sh -c 'cd /x && ~/.proton/proton run fusion.exe'",
        "bash -c foo",
        "flatpak run com.x.App",
        "/home/u/.steam/proton/proton run x.exe",
        "wine x.exe",
        "steam -applaunch 1",
        r#"foo "unclosed"#,
    ] {
        assert_eq!(detect(&loose(e), &fs), Origin::Unknown, "{e}");
    }
}

// ---- plan ----

fn pacman_runner() -> FakeRunner {
    FakeRunner::new(|prog, args| {
        let a = args.join(" ");
        match (prog, a.as_str()) {
            ("pacman", "-Qqo -- /usr/share/applications/my app.desktop") => ok("pkg-a\n"),
            ("pacman", "-Qqg cosmic") => ok("cosmic-edit\n"),
            ("pacman", "-Rs -p --print-format %n %v -- pkg-a") => ok("pkg-a 1.0-1\nlib-b 2-1\n"),
            ("pacman", "-Qql -- pkg-a") => ok(
                "/usr/share/applications/my app.desktop\n/usr/share/applications/other.desktop\n/usr/bin/x\n",
            ),
            _ => fail("unexpected"),
        }
    })
}

#[test]
fn plan_pacman_lists_package_and_sibling_apps() {
    let f = facts("a", "/usr/share/applications/my app.desktop", Some("a"));
    let r = pacman_runner();
    let p = plan(&Origin::Pacman, &f, &r).unwrap();
    assert!(p.preview.contains(&"pkg-a 1.0-1".to_string()));
    assert!(p.preview.contains(&"lib-b 2-1".to_string()));
    assert!(p.preview.iter().any(|l| l.contains("other.desktop")));
    assert!(!p.preview.iter().any(|l| l.contains("my app.desktop")));
    assert!(!p.supports_data_delete);
    assert!(r.calls().iter().all(|c| !c.contains("-Rdd")));
}

#[test]
fn plan_pacman_errors() {
    let f = facts("a", "/usr/share/applications/x.desktop", Some("a"));
    let none = FakeRunner::new(|_, _| fail("erro: Nenhum pacote possui /usr/x"));
    assert!(plan(&Origin::Pacman, &f, &none).is_err());
    let cosmic = FakeRunner::new(|_, _| ok("cosmic-edit\n"));
    let e = plan(&Origin::Pacman, &f, &cosmic).unwrap_err();
    assert_eq!(e, texts().err_cosmic_pkg.replace("{}", "cosmic-edit"));
    let dash = FakeRunner::new(|_, _| ok("--evil\n"));
    assert!(plan(&Origin::Pacman, &f, &dash).is_err());
    let required = FakeRunner::new(|_, a| match a[0].as_str() {
        "-Qqo" => ok("pkg-a\n"),
        "-Qqg" => ok(""),
        _ => fail("erro: não foi possível: base requer pkg-a"),
    });
    assert!(
        plan(&Origin::Pacman, &f, &required)
            .unwrap_err()
            .contains("base")
    );
}

#[test]
fn plan_refuses_non_removable() {
    let f = loose("x");
    let r = FakeRunner::new(|_, _| ok(""));
    assert!(plan(&Origin::Unknown, &f, &r).is_err());
    assert!(plan(&Origin::Protected, &f, &r).is_err());
    assert!(r.calls().is_empty());
}

#[test]
fn plan_waydroid_requires_session_and_refuses_system() {
    let f = loose("waydroid app launch com.foo");
    let o = Origin::Waydroid {
        package: "com.foo".into(),
    };
    let stopped = FakeRunner::new(|_, _| ok("Session:\tSTOPPED\nContainer:\tSTOPPED\n"));
    assert!(plan(&o, &f, &stopped).unwrap_err() == texts().err_start_waydroid);
    let running = FakeRunner::new(|_, _| ok("Session:\tRUNNING\n"));
    assert!(plan(&o, &f, &running).is_ok());
    let sys = Origin::Waydroid {
        package: "com.android.settings".into(),
    };
    assert!(plan(&sys, &f, &running).is_err());
}

#[test]
fn plan_simple_origins() {
    let r = FakeRunner::new(|_, _| ok(""));
    let f = loose("x");
    let fp = Origin::Flatpak {
        reference: "com.x.App".into(),
        system: false,
    };
    assert!(plan(&fp, &f, &r).unwrap().supports_data_delete);
    let st = Origin::Steam { app_id: "7".into() };
    assert!(plan(&st, &f, &r).is_ok());
    assert!(plan(&Origin::LooseDesktop, &f, &r).is_ok());
    let ai = Origin::AppImage {
        file: "/home/u/a b.AppImage".into(),
    };
    let p = plan(&ai, &f, &r).unwrap();
    assert_eq!(p.preview.len(), 2);
    let bad = Origin::Steam {
        app_id: "7;x".into(),
    };
    assert!(plan(&bad, &f, &r).is_err());
    assert!(r.calls().is_empty());
}

#[test]
fn only_the_plans_that_hand_over_to_another_program_say_so() {
    let r = FakeRunner::new(|_, _| ok(""));
    let f = loose("x");
    let steam = plan(&Origin::Steam { app_id: "7".into() }, &f, &r).unwrap();
    assert!(steam.opens_other_program());
    assert!(
        steam.preview.iter().any(|l| l.contains("Steam")),
        "{:?}",
        steam.preview
    );
    let wine = plan(
        &Origin::Wine {
            prefix: "/home/u/.wine".into(),
        },
        &f,
        &r,
    )
    .unwrap();
    assert!(wine.opens_other_program());
    assert!(
        wine.preview.iter().any(|l| l.contains("Wine")),
        "{:?}",
        wine.preview
    );
    let fp = Origin::Flatpak {
        reference: "com.x.App".into(),
        system: false,
    };
    assert!(!plan(&fp, &f, &r).unwrap().opens_other_program());
    assert!(
        !plan(&Origin::LooseDesktop, &f, &r)
            .unwrap()
            .opens_other_program()
    );
}

// ---- run ----

fn plan_for(o: &Origin, f: &DesktopFacts) -> Plan {
    let r = FakeRunner::new(|_, _| ok("Session:\tRUNNING\n"));
    plan(o, f, &r).unwrap()
}

#[test]
fn run_flatpak_commands() {
    let f = loose("x");
    let o = Origin::Flatpak {
        reference: "com.x.App".into(),
        system: true,
    };
    let p = plan_for(&o, &f);
    let r = FakeRunner::new(|_, _| ok(""));
    run(&p, false, &r).unwrap();
    run(&p, true, &r).unwrap();
    assert_eq!(
        r.calls(),
        vec![
            "flatpak uninstall -y --noninteractive --system com.x.App",
            "flatpak uninstall -y --noninteractive --system --delete-data com.x.App",
        ]
    );
}

#[test]
fn run_steam_wine_trash_pacman() {
    let f = facts(
        "a",
        "/home/u/.local/share/applications/a b.desktop",
        Some("x"),
    );
    let r = FakeRunner::new(|_, _| ok(""));
    run(
        &plan_for(
            &Origin::Steam {
                app_id: "42".into(),
            },
            &f,
        ),
        true,
        &r,
    )
    .unwrap();
    run(&plan_for(&Origin::LooseDesktop, &f), true, &r).unwrap();
    let ai = Origin::AppImage {
        file: "/home/u/x y.AppImage".into(),
    };
    run(&plan_for(&ai, &f), true, &r).unwrap();
    assert_eq!(
        r.calls(),
        vec![
            "xdg-open steam://uninstall/42",
            "gio trash -- /home/u/.local/share/applications/a b.desktop",
            "gio trash -- /home/u/x y.AppImage /home/u/.local/share/applications/a b.desktop",
        ]
    );
    let pf = facts("a", "/usr/share/applications/my app.desktop", Some("a"));
    let p = plan(&Origin::Pacman, &pf, &pacman_runner()).unwrap();
    let r = FakeRunner::new(|_, _| ok(""));
    run(&p, false, &r).unwrap();
    assert_eq!(
        r.calls(),
        vec!["pkexec /usr/bin/pacman -Rs --noconfirm -- pkg-a"]
    );
}

#[test]
fn run_waydroid_success_trashes_after_confirming_absence() {
    let f = facts("w", "/home/u/.local/share/applications/w.desktop", None);
    let o = Origin::Waydroid {
        package: "com.foo".into(),
    };
    let p = plan_for(&o, &f);
    let r = FakeRunner::new(|_, a| {
        if a.get(1).map(String::as_str) == Some("list") {
            ok("Name: Other\npackageName: com.foo.bar\n")
        } else {
            ok("")
        }
    });
    run(&p, false, &r).unwrap();
    assert_eq!(
        r.calls(),
        vec![
            "waydroid app remove com.foo",
            "waydroid app list",
            "gio trash -- /home/u/.local/share/applications/w.desktop",
        ]
    );
}

#[test]
fn run_waydroid_exit_zero_but_still_listed_is_err_and_keeps_desktop() {
    let f = facts("w", "/home/u/.local/share/applications/w.desktop", None);
    let o = Origin::Waydroid {
        package: "com.foo".into(),
    };
    let p = plan_for(&o, &f);
    let r = FakeRunner::new(|_, a| {
        if a.get(1).map(String::as_str) == Some("list") {
            ok("Name: Foo\npackageName: com.foo\n")
        } else {
            ok("")
        }
    });
    assert!(run(&p, false, &r).is_err());
    assert!(r.calls().iter().all(|c| !c.starts_with("gio")));
}

#[test]
fn run_error_is_truncated_and_stops() {
    let f = loose("x");
    let o = Origin::Flatpak {
        reference: "com.x.App".into(),
        system: false,
    };
    let p = plan_for(&o, &f);
    let long = "é".repeat(1000);
    let r = FakeRunner::new(move |_, _| fail(&long));
    let e = run(&p, false, &r).unwrap_err();
    assert!(e.chars().count() <= 400);
    let spawn = FakeRunner::new(|_, _| Err("sem flatpak".into()));
    assert_eq!(run(&p, false, &spawn).unwrap_err(), "sem flatpak");
}

#[test]
fn appimage_package_in_usr_is_pacman() {
    let fs = FakeFs::default().file("/opt/x/x.AppImage").with("/home/u");
    let f = facts(
        "x",
        "/usr/share/applications/x.desktop",
        Some("/opt/x/x.AppImage %U"),
    );
    assert_eq!(detect(&f, &fs), Origin::Pacman);
    let f = facts("x", "/usr/share/applications/x.desktop", Some("sh -c 'x'"));
    assert_eq!(detect(&f, &fs), Origin::Pacman);
}

#[test]
fn appimage_and_waydroid_need_desktop_in_user_apps() {
    let img = "/home/u/a.AppImage";
    let fs = FakeFs::default().file(img).with("/home/u");
    let f = facts("a", "/home/u/elsewhere/a.desktop", Some(img));
    assert_eq!(detect(&f, &fs), Origin::Unknown);
    let w = facts(
        "w",
        "/home/u/elsewhere/w.desktop",
        Some("waydroid app launch com.foo"),
    );
    assert_eq!(detect(&w, &fs), Origin::Unknown);
    let r = FakeRunner::new(|_, _| ok("Session:\tRUNNING\n"));
    let o = Origin::Waydroid {
        package: "com.foo".into(),
    };
    assert!(plan(&o, &w, &r).is_err());
    let ai = Origin::AppImage { file: img.into() };
    assert!(plan(&ai, &f, &r).is_err());
}

#[test]
fn usr_local_is_not_pacman() {
    let f = facts("x", "/usr/local/share/applications/x.desktop", Some("x"));
    assert_eq!(detect(&f, &FakeFs::default()), Origin::Unknown);
}

#[test]
fn flatpak_ref_must_match_appid() {
    let fs = FakeFs::default().with("/home/u/.local/share/flatpak/app/com.brave.Browser");
    let mut f = facts(
        "brave-webapp",
        "/home/u/.local/share/applications/w.desktop",
        Some("x"),
    );
    f.flatpak_ref = Some("com.brave.Browser".into());
    assert_eq!(detect(&f, &fs), Origin::Unknown);
    // A loose PWA shortcut with the browser's prefix must not offer to remove the browser.
    f.appid = "com.brave.Browser.Portal".into();
    assert_eq!(detect(&f, &fs), Origin::Unknown);
}

#[test]
fn flatpak_prefix_ids_only_count_in_flatpak_exports() {
    let fs = FakeFs::default().with("/home/u/.local/share/flatpak/app/com.brave.Browser");
    let reference = Origin::Flatpak {
        reference: "com.brave.Browser".into(),
        system: false,
    };
    for dir in [
        "/var/lib/flatpak/exports/share/applications",
        "/home/u/.local/share/flatpak/exports/share/applications",
    ] {
        let mut f = facts(
            "com.brave.Browser.Portal",
            &format!("{dir}/x.desktop"),
            Some("x"),
        );
        f.flatpak_ref = Some("com.brave.Browser".into());
        assert_eq!(detect(&f, &fs), reference, "{dir}");
    }
}

#[test]
fn flatpak_override_equal_to_ref_stays_flatpak() {
    let fs = FakeFs::default().with("/home/u/.local/share/flatpak/app/com.brave.Browser");
    let mut f = facts(
        "com.brave.Browser",
        "/home/u/.local/share/applications/c.desktop",
        Some("x"),
    );
    f.flatpak_ref = Some("com.brave.Browser".into());
    assert!(matches!(detect(&f, &fs), Origin::Flatpak { .. }));
}

#[test]
fn only_the_direct_child_of_user_apps_counts() {
    let img = "/home/u/a.AppImage";
    let fs = FakeFs::default().file(img).with("/home/u");
    let sub = "/home/u/.local/share/applications/sub";
    let a = facts("a", &format!("{sub}/a.desktop"), Some(img));
    assert_eq!(detect(&a, &fs), Origin::Unknown);
    let w = facts(
        "w",
        &format!("{sub}/w.desktop"),
        Some("waydroid app launch com.foo"),
    );
    assert_eq!(detect(&w, &fs), Origin::Unknown);
    let l = facts("l", &format!("{sub}/l.desktop"), Some("foo"));
    assert_eq!(detect(&l, &fs), Origin::Unknown);
}

#[test]
fn run_wine_rechecks_system_reg() {
    let prefix = "/home/u/.wine";
    let f = loose("x");
    let p = plan_for(
        &Origin::Wine {
            prefix: prefix.into(),
        },
        &f,
    );
    let r = FakeRunner::new(|_, _| ok(""));
    let gone = FakeFs::default();
    let e = run_with(&p, false, &r, &gone).unwrap_err();
    assert_eq!(e, texts().err_wine_gone.replace("{}", prefix));
    assert!(r.calls().is_empty());
    let there = FakeFs::default().with("/home/u/.wine/system.reg");
    assert!(run_with(&p, false, &r, &there).is_ok());
    assert_eq!(
        r.calls(),
        vec![format!("env WINEPREFIX={prefix} wine uninstaller")]
    );
}
