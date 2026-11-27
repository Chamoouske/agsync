//! `agsync::init::run` with every answer given: steps come back as data, the repo
//! error carries its reason and the hook questions go to the given `Confirm`.
//!
//! One test only: it moves this process into an isolated [`Env`].

mod common;

use std::cell::RefCell;
use std::process::Command;

use agsync::init::{Access, FirstApply, Hook, Options, Scheduling};
use agsync::prompt::Confirm;
use agsync::shell::Hooked;
use common::Env;

/// Answers no to everything and keeps the questions.
#[derive(Default)]
struct No(RefCell<Vec<String>>);

impl Confirm for No {
    fn can_answer(&self) -> bool {
        true
    }

    fn ask(&self, question: &str) -> anyhow::Result<bool> {
        self.0.borrow_mut().push(question.to_string());
        Ok(false)
    }
}

/// Copies the environment [`Env::isolate`] gives a child into this process.
fn enter(env: &Env) {
    let mut cmd = Command::new("agsync");
    env.isolate(&mut cmd);
    for (key, value) in cmd.get_envs() {
        match value {
            Some(value) => unsafe { std::env::set_var(key, value) },
            None => unsafe { std::env::remove_var(key) },
        }
    }
}

#[test]
fn run_returns_each_step_as_data() {
    let env = Env::new();
    enter(&env);
    let gone = env.root.path().join("no-such-repo");
    let options = Options {
        url: gone.to_string_lossy().to_string(),
        git_ref: None,
        profiles: vec!["work".into()],
        yes: false,
        silent: false,
    };
    let confirm = No::default();

    let outcome = agsync::init::run(&options, &confirm).unwrap();

    assert!(!outcome.ok());
    assert_eq!(outcome.saved.file, env.config.join("machine.toml"));
    assert_eq!(outcome.saved.machine.profiles, ["work"]);
    assert!(env.config.join("machine.toml").is_file());
    let Access::Unreachable { reason, .. } = &outcome.access else {
        panic!("{:?}", outcome.access);
    };
    assert!(reason.contains("git ls-remote falhou"), "{reason}");
    let Scheduling::Scheduled(scheduled) = &outcome.schedule else {
        panic!("{:?}", outcome.schedule);
    };
    assert_eq!(scheduled.minutes, 15);
    assert!(env.schedule_file().is_file());
    let Hook::Offered { shells } = &outcome.hook else {
        panic!("{:?}", outcome.hook);
    };
    let declined = shells
        .iter()
        .all(|h| matches!(h, Hooked::Declined { .. } | Hooked::NoProfile { .. }));
    assert!(declined, "{shells:?}");
    let asked = confirm.0.borrow().len();
    let offered = shells
        .iter()
        .filter(|h| matches!(h, Hooked::Declined { .. }))
        .count();
    assert_eq!(asked, offered);
    assert_eq!(outcome.apply, FirstApply::Unreachable);
    assert!(!env.home.join(".bashrc").exists());
    let json = serde_json::to_value(&outcome).unwrap();
    assert_eq!(json["access"]["status"], "unreachable");
    assert_eq!(json["apply"]["status"], "unreachable");
}
