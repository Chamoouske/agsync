//! Scheduler: the test file stand-in through the CLI, and on Windows one real Task
//! Scheduler task named `agsync-test-<random>`, always deleted at the end.

mod common;

use common::Env;

#[test]
fn schedule_commands_use_the_test_scheduler() {
    let env = Env::new();
    let (ok, out) = env.agsync(&["schedule", "status"]);
    assert!(!ok, "{out}");
    assert!(out.contains("agendamento ausente"), "{out}");
    let out = env.agsync(&["doctor"]).1;
    assert!(out.contains("AVISO   agendamento ausente"), "{out}");

    let out = env.ok(&["schedule", "install"]);
    assert!(out.contains("a cada 15 min"), "{out}");
    let saved = std::fs::read_to_string(env.schedule_file()).unwrap();
    let exe = std::path::absolute(env!("CARGO_BIN_EXE_agsync")).unwrap();
    assert_eq!(saved, format!("{}\t15\n", exe.display()));
    let out = env.ok(&["schedule", "status"]);
    assert!(out.contains("agendamento ok"), "{out}");
    assert!(env.agsync(&["doctor"]).1.contains("ok      agendamento ok"));

    std::fs::write(env.schedule_file(), "C:/old/agsync.exe\t15\n").unwrap();
    let (ok, out) = env.agsync(&["schedule", "status"]);
    assert!(!ok);
    assert!(
        out.contains("aponta para outro binario (C:/old/agsync.exe)"),
        "{out}"
    );
    assert!(
        env.agsync(&["doctor"])
            .1
            .contains("PROBLEMA agendamento aponta")
    );

    assert!(
        env.ok(&["schedule", "remove"])
            .contains("agendamento removido")
    );
    assert!(
        env.ok(&["schedule", "remove"])
            .contains("nenhum agendamento")
    );
    assert!(!env.schedule_file().exists());
}

#[cfg(windows)]
mod windows_task {
    use std::path::Path;

    use agsync_core::schedule::task::TaskScheduler;
    use agsync_core::schedule::{Scheduler, Status};

    /// Deletes the test task on drop, also when an assertion failed.
    struct Guard(String);

    impl Drop for Guard {
        fn drop(&mut self) {
            let _ = TaskScheduler::new(&self.0).remove();
            assert!(TaskScheduler::new(&self.0).query_xml().is_none());
        }
    }

    fn unique_name() -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("agsync-test-{}-{nanos}", std::process::id())
    }

    #[test]
    fn real_task_is_created_checked_and_deleted() {
        let name = unique_name();
        assert!(name.starts_with("agsync-test-"));
        let guard = Guard(name.clone());
        let task = TaskScheduler::new(&guard.0);
        let exe = Path::new(r"C:\agsync test\agsync.exe");
        assert_eq!(task.status(exe).unwrap(), Status::Missing);

        task.install(exe, 15).unwrap();
        let xml = task.query_xml().expect("task created");
        eprintln!("{xml}");
        for part in [
            "<LogonTrigger>",
            "<Interval>PT15M</Interval>",
            "<MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>",
            "<ExecutionTimeLimit>PT10M</ExecutionTimeLimit>",
            "<LogonType>InteractiveToken</LogonType>",
            r"System32\conhost.exe</Command>",
            "<Arguments>--headless \"C:\\agsync test\\agsync.exe\" check</Arguments>",
        ] {
            assert!(xml.contains(part), "missing {part}");
        }
        assert!(
            !xml.contains("HighestAvailable"),
            "task must not ask for admin"
        );
        assert_eq!(task.status(exe).unwrap(), Status::Current);
        let other = task.status(Path::new(r"C:\other\agsync.exe")).unwrap();
        assert!(matches!(other, Status::Other(_)), "{other:?}");

        task.install(exe, 30).unwrap();
        assert!(
            task.query_xml()
                .unwrap()
                .contains("<Interval>PT30M</Interval>")
        );

        assert!(task.remove().unwrap());
        assert!(!task.remove().unwrap());
        assert_eq!(task.status(exe).unwrap(), Status::Missing);
        drop(guard);
    }
}
