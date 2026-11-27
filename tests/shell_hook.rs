//! Shell hook install and remove on temp profiles: temp HOME and Documents, fake shell
//! executables so every shell counts as present. Real profiles are checked unchanged.

mod common;

use std::path::PathBuf;
use std::process::Command;

use common::{Env, Untouched, fake_bin};

const BEGIN: &str = "# >>> agsync hook >>>";
const END: &str = "# <<< agsync hook <<<";

/// Profiles of the real user, which the test must never change.
fn real_profiles() -> Vec<PathBuf> {
    let home = PathBuf::from(
        std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .unwrap(),
    );
    let mut files = vec![
        home.join(".bashrc"),
        home.join(".zshrc"),
        home.join(".config")
            .join("fish")
            .join("conf.d")
            .join("agsync.fish"),
    ];
    if let Some(exe) = agsync_core::which::find("powershell") {
        let out = Command::new(exe)
            .args([
                "-NoProfile",
                "-Command",
                "[Environment]::GetFolderPath('MyDocuments')",
            ])
            .output()
            .unwrap();
        let docs = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
        files.push(docs.join("WindowsPowerShell").join("profile.ps1"));
        files.push(docs.join("PowerShell").join("profile.ps1"));
    }
    files
}

#[test]
fn shell_hook_installs_once_and_removes_cleanly() {
    let untouched = Untouched::snapshot(&real_profiles());
    let mut env = Env::new();
    let bin = fake_bin(&env, &["powershell", "pwsh", "bash", "zsh", "fish"]);
    env.path = Some(std::env::join_paths([bin]).unwrap());
    let docs = env.home.join("Documents");
    let ps5 = docs.join("WindowsPowerShell").join("profile.ps1");
    let ps7 = docs.join("PowerShell").join("profile.ps1");
    let bashrc = env.home.join(".bashrc");
    let zshrc = env.home.join(".zshrc");
    let fish = env
        .home
        .join(".config")
        .join("fish")
        .join("conf.d")
        .join("agsync.fish");
    std::fs::write(&bashrc, "export A=1\n").unwrap();
    std::fs::create_dir_all(ps5.parent().unwrap()).unwrap();
    std::fs::write(&ps5, "Set-Alias ll Get-ChildItem\r\n").unwrap();

    let out = env.ok(&["hook", "install"]);
    assert!(out.contains("sem TTY"), "{out}");
    assert!(!zshrc.exists());

    let out = env.ok(&["hook", "install", "--yes"]);
    for file in [&ps5, &ps7, &bashrc, &zshrc, &fish] {
        let text = std::fs::read_to_string(file).unwrap();
        assert_eq!(text.matches(BEGIN).count(), 1, "{}: {text}", file.display());
        assert_eq!(text.matches(END).count(), 1);
        assert!(text.contains(" hook"), "{text}");
        assert!(out.contains(&file.display().to_string()), "{out}");
    }
    let bash_text = std::fs::read_to_string(&bashrc).unwrap();
    assert!(bash_text.starts_with("export A=1\n# >>> agsync hook >>>\nif [ -x '"));
    let ps5_text = std::fs::read_to_string(&ps5).unwrap();
    assert!(
        ps5_text
            .starts_with("Set-Alias ll Get-ChildItem\r\n# >>> agsync hook >>>\r\nif (Test-Path")
    );
    assert!(
        std::fs::read_to_string(&fish)
            .unwrap()
            .contains("status is-interactive")
    );

    let before: Vec<String> = [&ps5, &ps7, &bashrc, &zshrc, &fish]
        .iter()
        .map(|f| std::fs::read_to_string(f).unwrap())
        .collect();
    let out = env.ok(&["hook", "install", "--yes"]);
    assert_eq!(out.matches("hook ja estava").count(), 5, "{out}");
    let after: Vec<String> = [&ps5, &ps7, &bashrc, &zshrc, &fish]
        .iter()
        .map(|f| std::fs::read_to_string(f).unwrap())
        .collect();
    assert_eq!(before, after);

    let out = env.ok(&["hook", "remove"]);
    assert_eq!(out.matches("hook removido").count(), 5, "{out}");
    assert_eq!(std::fs::read_to_string(&bashrc).unwrap(), "export A=1\n");
    assert_eq!(
        std::fs::read_to_string(&ps5).unwrap(),
        "Set-Alias ll Get-ChildItem\r\n"
    );
    for created in [&ps7, &zshrc, &fish] {
        assert!(!created.exists(), "{}", created.display());
    }
    assert!(env.ok(&["hook", "remove"]).contains("nenhum hook de shell"));
    assert!(common::count(&env.data.join("backups")) >= 5);
    untouched.assert_same();
}
