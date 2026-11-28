//! Windows, or Linux outside WSL: nothing foreign, the OS notifier.

use super::Host;
use crate::notify::{self, Notifier};

pub struct Native;

impl Host for Native {
    fn notifier(&self) -> Box<dyn Notifier> {
        notify::system()
    }
}
