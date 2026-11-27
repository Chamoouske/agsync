//! Ssh with a passphrase key outside the ssh-agent: every command fails fast with the
//! ssh-agent hint, never asking for the passphrase. A fake ssh stands in.

mod common;

use common::{Env, path_with};

#[test]
fn ssh_key_outside_agent_fails_fast_with_hint() {
    let mut env = Env::new();
    env.path = Some(path_with(&["git", "sh"], &[]));
    let mark = env.root.path().join("prompted");
    let mark_text = mark.to_string_lossy().replace('\\', "/");
    let fake = env.root.path().join("fake-ssh");
    let script = format!(
        "case \"$*\" in *BatchMode=yes*) echo 'git@h: Permission denied (publickey).' >&2; exit 255;; esac\ntouch '{mark_text}'\nexit 1\n"
    );
    std::fs::write(&fake, script).unwrap();
    let fake_text = fake.to_string_lossy().replace('\\', "/");
    env.vars = vec![("GIT_SSH_COMMAND".to_string(), format!("sh '{fake_text}'"))];

    let url = "ssh://git@agsync.invalid/config.git";
    for args in [
        vec!["init", url, "--profile", "work"],
        vec!["check"],
        vec!["apply", "--yes"],
        vec!["doctor"],
    ] {
        let (_, out) = env.agsync(&args);
        let text = match args[0] {
            "check" => env.check_log(),
            _ => out,
        };
        assert!(text.contains("ssh-add"), "agsync {args:?}:\n{text}");
    }
    assert!(!mark.exists(), "ssh was run without BatchMode");
}
