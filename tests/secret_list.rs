//! Golden output of `agsync secret list`.

mod common;

use common::Env;

const SECRETS_ENV: &str =
    "# team\nZETA=last\n\nALPHA=first-value\nEMPTY=\nbad line\nALPHA=dup-value\n";

#[test]
fn secret_list_prints_names_only() {
    let mut env = Env::new();
    env.forbidden = vec!["first-value".into(), "dup-value".into(), "last".into()];
    std::fs::create_dir_all(&env.config).unwrap();
    std::fs::write(env.config.join("secrets.env"), SECRETS_ENV).unwrap();
    assert_eq!(env.ok(&["secret", "list"]), "ALPHA\nEMPTY\nZETA\n");
}

#[test]
fn secret_list_without_file_prints_nothing() {
    let env = Env::new();
    assert_eq!(env.ok(&["secret", "list"]), "");
}
