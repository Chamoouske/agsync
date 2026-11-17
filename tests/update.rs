//! End to end for the new version notice: a local git repo with `vX.Y.Z` tags stands
//! in for the release repo.

mod common;

use std::path::{Path, PathBuf};

use common::{Env, fake_bin, git, path_with};

const CURRENT: &str = env!("CARGO_PKG_VERSION");

fn setup() -> Env {
    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex"]);
    env.path = Some(path_with(&["git", "sh", "curl"], &[bin]));
    let log = env.root.path().join("fake.log");
    env.vars = vec![(
        "FAKE_AGENT_LOG".to_string(),
        log.to_string_lossy().to_string(),
    )];
    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push("version: 1\nmcps:\n  echo:\n    command: node\n");
    let url = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &url, "--profile", "work"]);
    env
}

/// A git repo under the env root with one commit and these tags.
fn releases(env: &Env, name: &str, tags: &[&str]) -> PathBuf {
    let dir = env.root.path().join(name);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    std::fs::write(dir.join("README"), "x").unwrap();
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "x"]);
    for tag in tags {
        git(&dir, &["tag", tag]);
    }
    dir
}

fn use_releases(env: &mut Env, dir: &Path) {
    env.vars.retain(|(key, _)| key != "AGSYNC_RELEASES_URL");
    env.vars.push((
        "AGSYNC_RELEASES_URL".to_string(),
        dir.to_string_lossy().to_string(),
    ));
}

fn update_notices(env: &Env) -> Vec<String> {
    env.notifications()
        .into_iter()
        .filter(|n| n.contains("disponivel"))
        .collect()
}

#[test]
fn check_records_a_new_version_and_notifies_once() {
    let mut env = setup();
    let repo = releases(
        &env,
        "releases",
        &["v0.0.1", "v99.0.0", "v99.1.0-rc1", "nightly"],
    );
    use_releases(&mut env, &repo);

    env.ok(&["check"]);
    let saved = std::fs::read_to_string(env.data.join("update.json")).unwrap();
    assert!(saved.contains("\"99.0.0\""), "{saved}");
    let line = format!("agsync 99.0.0 disponivel (atual {CURRENT}): rode `agsync update`");
    assert_eq!(update_notices(&env).len(), 1);
    assert!(update_notices(&env)[0].contains(&line));

    env.ok(&["check"]);
    assert_eq!(update_notices(&env).len(), 1, "same version notified twice");
    assert!(
        env.check_log().contains("(ja avisado)"),
        "{}",
        env.check_log()
    );

    let out = env.ok(&["hook"]);
    assert!(out.contains(&line), "{out}");
    let status = env.ok(&["status"]);
    assert!(status.contains(&line), "{status}");
    let doctor = env.agsync(&["doctor"]).1;
    assert!(
        doctor.contains(&format!("info    agsync {CURRENT}")),
        "{doctor}"
    );

    git(&repo, &["tag", "v99.0.1"]);
    env.ok(&["check"]);
    assert_eq!(update_notices(&env).len(), 2);

    let old = releases(&env, "old-releases", &["v0.0.1"]);
    use_releases(&mut env, &old);
    env.ok(&["check"]);
    assert!(!env.data.join("update.json").exists());
    assert!(!env.ok(&["hook"]).contains("disponivel"));
}

#[test]
fn hook_ignores_a_recorded_version_that_is_not_newer() {
    let env = setup();
    let text = format!("{{\"version\": \"{CURRENT}\"}}");
    std::fs::write(env.data.join("update.json"), text).unwrap();
    assert!(!env.ok(&["hook"]).contains("disponivel"));
}

#[test]
fn unreachable_release_repo_is_logged_and_check_goes_on() {
    let env = setup();
    env.ok(&["check"]);
    assert!(
        env.check_log().contains("consulta falhou"),
        "{}",
        env.check_log()
    );
    assert!(env.pending().is_some());
    assert!(!env.data.join("update.json").exists());
}

#[cfg(windows)]
const ASSET: &str = "agsync-x86_64-pc-windows-msvc.exe";
#[cfg(not(windows))]
const ASSET: &str = "agsync-x86_64-unknown-linux-musl";

/// A copy of the agsync under test, so that an update never touches cargo's binary.
fn agsync_copy(env: &Env) -> PathBuf {
    let dir = env.root.path().join("bin");
    std::fs::create_dir_all(&dir).unwrap();
    let exe = dir.join(format!("agsync{}", std::env::consts::EXE_SUFFIX));
    std::fs::copy(env!("CARGO_BIN_EXE_agsync"), &exe).unwrap();
    exe
}

/// Release `version` with `asset` as the binary and `hash` in `SHA256SUMS`; the
/// `AGSYNC_INSTALL_BASE_URL` is a `file://` url for curl.
fn publish(env: &mut Env, version: &str, asset: &[u8], hash: &str) {
    let base = env.root.path().join("release-files");
    let dir = base.join("download").join(format!("v{version}"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(ASSET), asset).unwrap();
    std::fs::write(dir.join("SHA256SUMS"), format!("{hash}  {ASSET}\n")).unwrap();
    let path = base.to_string_lossy().replace('\\', "/");
    let url = format!("file:///{}", path.trim_start_matches('/'));
    env.vars.push(("AGSYNC_INSTALL_BASE_URL".to_string(), url));
    let repo = releases(env, "releases", &[&format!("v{version}")]);
    use_releases(env, &repo);
}

fn run(env: &Env, exe: &Path, args: &[&str]) -> (bool, String) {
    let (code, text) = run_code(env, exe, args);
    (code == Some(0), text)
}

/// Runs `exe` with the env; `SCOOP` and `SCOOP_GLOBAL` come only from `env.vars`.
fn run_code(env: &Env, exe: &Path, args: &[&str]) -> (Option<i32>, String) {
    let mut cmd = std::process::Command::new(exe);
    cmd.args(args)
        .env_remove("SCOOP")
        .env_remove("SCOOP_GLOBAL");
    env.isolate(&mut cmd);
    let out = cmd.output().unwrap();
    let text = common::text(&out);
    eprintln!("$ {} {}\n{text}", exe.display(), args.join(" "));
    (out.status.code(), text)
}

/// A `scoop` first in `PATH` that exits with `FAKE_SCOOP_EXIT`.
fn fake_scoop(env: &mut Env) {
    let dir = env.root.path().join("scoop-bin");
    std::fs::create_dir_all(&dir).unwrap();
    if cfg!(windows) {
        std::fs::write(dir.join("scoop.cmd"), "@exit /b %FAKE_SCOOP_EXIT%\r\n").unwrap();
    } else {
        let script = dir.join("scoop");
        std::fs::write(&script, "#!/bin/sh\nexit \"$FAKE_SCOOP_EXIT\"\n").unwrap();
        make_executable(&script);
    }
    let old = env.path.clone().unwrap_or_default();
    let dirs = std::iter::once(dir).chain(std::env::split_paths(&old));
    env.path = Some(std::env::join_paths(dirs).unwrap());
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}

fn set_var(env: &mut Env, key: &str, value: &str) {
    env.vars.retain(|(k, _)| k != key);
    env.vars.push((key.to_string(), value.to_string()));
}

#[test]
fn update_from_scoop_runs_scoop_and_passes_its_exit_code() {
    let mut env = setup();
    let exe = agsync_copy(&env);
    let repo = releases(&env, "releases", &["v99.0.0"]);
    use_releases(&mut env, &repo);
    fake_scoop(&mut env);
    let scoop_root = exe.parent().unwrap().to_string_lossy().to_string();
    set_var(&mut env, "SCOOP", &scoop_root);
    env.ok(&["check"]);
    assert!(env.data.join("update.json").exists());
    let before = std::fs::read(&exe).unwrap();

    set_var(&mut env, "FAKE_SCOOP_EXIT", "3");
    let (code, out) = run_code(&env, &exe, &["update", "--yes"]);
    assert_eq!(code, Some(3), "{out}");
    assert!(env.data.join("update.json").exists());

    set_var(&mut env, "FAKE_SCOOP_EXIT", "0");
    let (code, out) = run_code(&env, &exe, &["update", "--yes"]);
    assert_eq!(code, Some(0), "{out}");
    assert!(!env.data.join("update.json").exists());
    assert_eq!(
        std::fs::read(&exe).unwrap(),
        before,
        "scoop install was replaced"
    );
}

#[test]
fn update_refuses_a_wrong_hash_and_changes_nothing() {
    let mut env = setup();
    let exe = agsync_copy(&env);
    let before = std::fs::read(&exe).unwrap();
    let wrong = "0".repeat(64);
    publish(&mut env, "99.0.0", b"not agsync", &wrong);

    let (ok, out) = run(&env, &exe, &["update", "--yes"]);
    assert!(!ok, "{out}");
    assert!(out.contains("nao confere"), "{out}");
    assert!(out.contains("nada foi trocado"), "{out}");
    assert_eq!(std::fs::read(&exe).unwrap(), before);
    assert_eq!(common::count(exe.parent().unwrap()), 1);
}

#[test]
fn update_without_terminal_needs_yes() {
    let mut env = setup();
    let exe = agsync_copy(&env);
    publish(&mut env, "99.0.0", b"x", &agsync::hash::hash_bytes(b"x"));
    let (ok, out) = run(&env, &exe, &["update"]);
    assert!(!ok, "{out}");
    assert!(out.contains("rode `agsync update --yes`"), "{out}");
}

#[test]
fn update_at_the_latest_version_does_nothing() {
    let mut env = setup();
    let repo = releases(&env, "releases", &["v0.0.1"]);
    use_releases(&mut env, &repo);
    let out = env.ok(&["update", "--yes"]);
    assert!(
        out.contains(&format!("agsync {CURRENT} ja e a ultima versao")),
        "{out}"
    );
}

#[test]
fn update_swaps_in_the_checked_binary() {
    let mut env = setup();
    let exe = agsync_copy(&env);
    let new = std::fs::read(env!("CARGO_BIN_EXE_agsync")).unwrap();
    let hash = agsync::hash::hash_bytes(&new);
    publish(&mut env, "99.0.0", &new, &hash.to_uppercase());
    env.ok(&["check"]);
    assert!(env.data.join("update.json").exists());

    let (ok, out) = run(&env, &exe, &["update", "--yes"]);
    assert!(ok, "{out}");
    assert!(out.contains("agsync atualizado para 99.0.0"), "{out}");
    assert!(!env.data.join("update.json").exists());
    assert_eq!(
        agsync::hash::hash_bytes(&std::fs::read(&exe).unwrap()),
        hash
    );
    let dir = exe.parent().unwrap();
    assert!(
        !dir.join(format!(
            "{}.new",
            exe.file_name().unwrap().to_string_lossy()
        ))
        .exists()
    );
    if cfg!(windows) {
        assert!(dir.join("agsync.exe.old").exists());
    }
    assert!(run(&env, &exe, &["--version"]).0);
}

#[test]
fn failed_notification_is_retried_by_the_next_check() {
    let mut env = setup();
    let repo = releases(&env, "releases", &["v99.0.0"]);
    use_releases(&mut env, &repo);
    let blocked = env.root.path().join("notify-is-a-dir");
    std::fs::create_dir_all(&blocked).unwrap();
    env.vars.push((
        "AGSYNC_TEST_NOTIFY_LOG".to_string(),
        blocked.to_string_lossy().to_string(),
    ));

    env.ok(&["check"]);
    assert!(
        env.check_log().contains("notificacao nao mostrada"),
        "{}",
        env.check_log()
    );
    let saved = std::fs::read_to_string(env.data.join("update.json")).unwrap();
    assert!(saved.contains("\"notified\": false"), "{saved}");
    assert!(env.ok(&["hook"]).contains("agsync 99.0.0 disponivel"));

    env.vars.retain(|(key, _)| key != "AGSYNC_TEST_NOTIFY_LOG");
    env.ok(&["check"]);
    assert_eq!(update_notices(&env).len(), 1);
    env.ok(&["check"]);
    assert_eq!(update_notices(&env).len(), 1, "notified twice");
}

#[test]
fn update_of_a_tag_without_binaries_says_so_and_changes_nothing() {
    let mut env = setup();
    let exe = agsync_copy(&env);
    let before = std::fs::read(&exe).unwrap();
    publish(&mut env, "99.0.0", b"x", &agsync::hash::hash_bytes(b"x"));
    let repo = releases(&env, "newer-releases", &["v99.0.0", "v99.0.1"]);
    use_releases(&mut env, &repo);

    let (ok, out) = run(&env, &exe, &["update", "--yes"]);
    assert!(!ok, "{out}");
    assert!(out.contains("release v99.0.1 ainda sem binarios"), "{out}");
    assert_eq!(std::fs::read(&exe).unwrap(), before);
    assert_eq!(common::count(exe.parent().unwrap()), 1);
}
