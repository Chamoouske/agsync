//! Notifier that appends `title<TAB>text` lines to a file, so tests can count them.

use std::io::Write;
use std::path::PathBuf;

use super::Notifier;

pub struct FileNotifier(pub PathBuf);

impl Notifier for FileNotifier {
    fn notify(&self, title: &str, text: &str) -> anyhow::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.0)?;
        writeln!(file, "{title}\t{text}")?;
        Ok(())
    }
}
