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
    let none = Fake {
        answer: false,
        tty: false,
    };
    assert!(yes_or(true, &none).can_answer());
}
