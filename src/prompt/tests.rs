use super::*;

struct Fake {
    answer: bool,
    tty: bool,
}

impl Confirm for Fake {
    fn can_answer(&self) -> bool {
        self.tty
    }

    fn ask(&self, _question: &str) -> anyhow::Result<bool> {
        Ok(self.answer)
    }
}

#[test]
fn confirm_follows_the_fake_answer() {
    let no = Fake {
        answer: false,
        tty: true,
    };
    let yes = Fake {
        answer: true,
        tty: true,
    };
    assert!(!confirm(&no, "Aplicar?", "agsync apply").unwrap());
    assert!(confirm(&yes, "Aplicar?", "agsync apply").unwrap());
}

#[test]
fn confirm_without_answer_names_the_yes_flag() {
    let none = Fake {
        answer: true,
        tty: false,
    };
    let err = confirm(&none, "Aplicar?", "agsync apply").unwrap_err();
    assert_eq!(
        err.to_string(),
        "sem TTY: nada feito; rode `agsync apply --yes`"
    );
}

#[test]
fn yes_answers_without_asking() {
    assert!(confirm(&Yes, "Aplicar?", "agsync apply").unwrap());
    assert!(from_flag(true).can_answer());
}

#[test]
fn terminal_question_reads_one_line() {
    for (line, expected) in [
        ("s\n", true),
        ("SIM\n", true),
        ("y\n", true),
        ("yes\n", true),
        ("n\n", false),
        ("\n", false),
        ("", false),
    ] {
        let mut out = Vec::new();
        let answer = yes_no("Aplicar estas mudancas?", &mut line.as_bytes(), &mut out).unwrap();
        assert_eq!(answer, expected, "{line:?}");
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "Aplicar estas mudancas? [s/N] "
        );
    }
}
