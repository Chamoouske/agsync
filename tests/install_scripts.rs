//! `install.ps1` (Windows) and `install.sh` (Linux) against a fake release served on
//! 127.0.0.1 through `AGSYNC_INSTALL_BASE_URL`; nothing reaches GitHub. The binary of
//! the fake release is this build of agsync. On Windows the user PATH is a test
//! variable (`AGSYNC_TEST_PATH_VAR`), deleted at the end.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};

use agsync_core::tmp::TempDir;
use sha2::{Digest, Sha256};

#[cfg(windows)]
const ASSET: &str = "agsync-x86_64-pc-windows-msvc.exe";
#[cfg(not(windows))]
const ASSET: &str = "agsync-x86_64-unknown-linux-musl";

const NEXT: &str = "proximo passo: agsync init <url> --profile <perfil>";

/// Files under `root` over HTTP, and every path asked for.
fn serve(root: PathBuf) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let asked = Arc::new(Mutex::new(Vec::new()));
    let log = asked.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                continue;
            };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request = String::new();
            reader.read_line(&mut request).unwrap_or(0);
            let mut header = String::new();
            while reader.read_line(&mut header).unwrap_or(0) > 2 {
                header.clear();
            }
            let path = request.split_whitespace().nth(1).unwrap_or("/").to_string();
            log.lock().unwrap().push(path.clone());
            let body = std::fs::read(root.join(path.trim_start_matches('/')));
            let reply = match body {
                Ok(body) => {
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    [head.into_bytes(), body].concat()
                }
                Err(_) => {
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        .to_vec()
                }
            };
            let _ = stream.write_all(&reply);
        }
    });
    (format!("http://{addr}"), asked)
}

/// A release dir with the binary and its `SHA256SUMS` under `latest/download` and
/// `download/v0.1.0`; `sum` replaces the real hash when given.
fn release(root: &Path, sum: Option<&str>) {
    let binary = std::fs::read(env!("CARGO_BIN_EXE_agsync")).unwrap();
    let digest = Sha256::digest(&binary);
    let real: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    let sums = format!("{}  {ASSET}\n", sum.unwrap_or(&real));
    for dir in [
        root.join("latest").join("download"),
        root.join("download").join("v0.1.0"),
    ] {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(ASSET), &binary).unwrap();
        std::fs::write(dir.join("SHA256SUMS"), &sums).unwrap();
    }
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn script(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(name)
}

fn version_of(exe: &Path) -> String {
    let out = Command::new(exe).arg("--version").output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[cfg(windows)]
mod os {
    use super::*;

    pub struct PathVar(pub String);

    impl PathVar {
        pub fn get(&self) -> String {
            let script = "[Environment]::GetEnvironmentVariable($env:N, 'User')";
            powershell(script, &[("N", &self.0)])
        }
    }

    impl Drop for PathVar {
        fn drop(&mut self) {
            let script = "[Environment]::SetEnvironmentVariable($env:N, $null, 'User')";
            powershell(script, &[("N", &self.0)]);
        }
    }

    fn powershell(script: &str, vars: &[(&str, &str)]) -> String {
        let mut cmd = Command::new("powershell.exe");
        cmd.args(["-NoProfile", "-NonInteractive", "-Command", script]);
        for (key, value) in vars {
            cmd.env(key, value);
        }
        String::from_utf8_lossy(&cmd.output().unwrap().stdout)
            .trim()
            .to_string()
    }

    pub fn install(base: &str, local: &Path, var: &str, args: &[&str]) -> Output {
        Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
            ])
            .arg("-File")
            .arg(script("install.ps1"))
            .args(args)
            .env("AGSYNC_INSTALL_BASE_URL", base)
            .env("LOCALAPPDATA", local)
            .env("AGSYNC_TEST_PATH_VAR", var)
            .output()
            .unwrap()
    }
}

#[cfg(windows)]
#[test]
fn install_ps1_downloads_checks_installs_and_extends_the_user_path() {
    let tmp = TempDir::new("agsync-install-test").unwrap();
    let good = tmp.path().join("release");
    release(&good, None);
    let (base, asked) = serve(good);
    let var = os::PathVar(format!("AGSYNC_TEST_INSTALL_PATH_{}", std::process::id()));
    let local = tmp.path().join("localappdata");
    let bin = local.join("agsync").join("bin");

    let out = os::install(&base, &local, &var.0, &[]);
    let said = text(&out);
    assert!(out.status.success(), "{said}");
    assert!(said.contains(NEXT), "{said}");
    let exe = bin.join("agsync.exe");
    assert_eq!(version_of(&exe), "agsync 0.1.0");
    assert_eq!(var.get(), bin.to_string_lossy());
    assert!(
        asked
            .lock()
            .unwrap()
            .contains(&format!("/latest/download/{ASSET}"))
    );

    let out = os::install(&base, &local, &var.0, &["-Version", "0.1.0"]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(
        asked
            .lock()
            .unwrap()
            .contains(&format!("/download/v0.1.0/{ASSET}"))
    );
    assert_eq!(var.get(), bin.to_string_lossy());

    let bad = tmp.path().join("bad");
    release(&bad, Some(&"0".repeat(64)));
    let (bad_base, _) = serve(bad);
    let other = tmp.path().join("other");
    let out = os::install(&bad_base, &other, &var.0, &[]);
    assert!(!out.status.success());
    assert!(text(&out).contains("nao confere"), "{}", text(&out));
    assert!(!other.join("agsync").join("bin").join("agsync.exe").exists());
}

#[cfg(not(windows))]
fn install_sh(base: &str, home: &Path, args: &[&str]) -> Output {
    Command::new("sh")
        .arg(script("install.sh"))
        .args(args)
        .env("AGSYNC_INSTALL_BASE_URL", base)
        .env("HOME", home)
        .env("PATH", "/usr/local/bin:/usr/bin:/bin")
        .output()
        .unwrap()
}

#[cfg(not(windows))]
#[test]
fn install_sh_downloads_checks_installs_and_extends_the_path() {
    let tmp = TempDir::new("agsync-install-test").unwrap();
    let good = tmp.path().join("release");
    release(&good, None);
    let (base, asked) = serve(good);
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(home.join(".bashrc"), "# mine\n").unwrap();
    let line = "export PATH=\"$HOME/.local/bin:$PATH\"";

    let out = install_sh(&base, &home, &[]);
    let said = text(&out);
    assert!(out.status.success(), "{said}");
    assert!(said.contains(NEXT), "{said}");
    let exe = home.join(".local").join("bin").join("agsync");
    assert_eq!(version_of(&exe), "agsync 0.1.0");
    assert!(
        asked
            .lock()
            .unwrap()
            .contains(&format!("/latest/download/{ASSET}"))
    );
    for rc in [".profile", ".bashrc"] {
        let text = std::fs::read_to_string(home.join(rc)).unwrap();
        assert_eq!(text.matches(line).count(), 1, "{rc}: {text}");
    }
    assert!(!home.join(".zshrc").exists());

    let out = install_sh(&base, &home, &["--version", "0.1.0"]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(
        asked
            .lock()
            .unwrap()
            .contains(&format!("/download/v0.1.0/{ASSET}"))
    );
    let bashrc = std::fs::read_to_string(home.join(".bashrc")).unwrap();
    assert_eq!(bashrc.matches(line).count(), 1, "{bashrc}");
    assert!(bashrc.starts_with("# mine\n"));

    let bad = tmp.path().join("bad");
    release(&bad, Some(&"0".repeat(64)));
    let (bad_base, _) = serve(bad);
    let other = tmp.path().join("other");
    std::fs::create_dir_all(&other).unwrap();
    let out = install_sh(&bad_base, &other, &[]);
    assert!(!out.status.success());
    assert!(text(&out).contains("nao confere"), "{}", text(&out));
    assert!(!other.join(".local").join("bin").join("agsync").exists());
}
