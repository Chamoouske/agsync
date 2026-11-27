//! Windows toast through WinRT.
//!
//! A toast needs an AppUserModelID tied to a Start menu shortcut. agsync borrows the one
//! of Windows PowerShell, whose shortcut ships with every Windows 10 and 11, so nothing
//! has to be installed or registered. The toast shows "agsync" as its title; Windows
//! names Windows PowerShell as the sender.

use anyhow::Context;
use windows::Data::Xml::Dom::XmlDocument;
use windows::UI::Notifications::{
    NotificationSetting, ToastNotification, ToastNotificationManager,
};
use windows::core::HSTRING;

use super::Notifier;

/// AppUserModelID of the Windows PowerShell Start menu shortcut.
pub const AUMID: &str =
    r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\WindowsPowerShell\v1.0\powershell.exe";

/// Tag of every agsync toast; a new one replaces the previous in the action center.
pub const TAG: &str = "agsync-pending";

pub struct Toast;

impl Notifier for Toast {
    fn notify(&self, title: &str, text: &str) -> anyhow::Result<()> {
        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(AUMID))
            .context("criar notificador de toast")?;
        if let Ok(setting) = notifier.Setting()
            && setting != NotificationSetting::Enabled
        {
            anyhow::bail!("toasts desligados para o Windows PowerShell ({setting:?})");
        }
        let xml = XmlDocument::new()?;
        xml.LoadXml(&HSTRING::from(toast_xml(title, text)))
            .context("montar xml do toast")?;
        let toast = ToastNotification::CreateToastNotification(&xml)?;
        toast.SetTag(&HSTRING::from(TAG))?;
        notifier.Show(&toast).context("mostrar toast")?;
        Ok(())
    }
}

fn toast_xml(title: &str, text: &str) -> String {
    format!(
        "<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>",
        escape(title),
        escape(text)
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_escapes_text() {
        let xml = toast_xml("agsync", "1 < 2 & `apply`");
        assert!(xml.contains("<text>1 &lt; 2 &amp; `apply`</text>"), "{xml}");
    }

    /// Shows a real toast. Run by hand: `cargo test real_toast -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_toast() {
        let shown = Toast.notify("agsync", "teste manual do toast; pode fechar");
        if let Err(e) = &shown {
            panic!("{e:#}");
        }
        let history = ToastNotificationManager::History()
            .unwrap()
            .GetHistoryWithId(&HSTRING::from(AUMID))
            .unwrap();
        let tags: Vec<String> = history
            .into_iter()
            .map(|t| t.Tag().unwrap().to_string())
            .collect();
        println!("toast mostrado pelo AUMID {AUMID}; central de acoes tem tags {tags:?}");
        assert!(tags.iter().any(|t| t == TAG));
    }
}
