//! `secrets.env` on disk: private permissions and line-preserving updates.

use std::path::Path;

use anyhow::Context;

use super::{parse, parse_line, valid_name};

/// Sets `name=value` in the file, keeping every other line as it was.
pub fn set(path: &Path, name: &str, value: &str) -> anyhow::Result<()> {
    if !valid_name(name) {
        anyhow::bail!("nome '{name}' invalido (use letras, numeros e _)");
    }
    if value.contains('\n') || value.contains('\r') {
        anyhow::bail!("o valor nao pode ter quebra de linha");
    }
    let old = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).with_context(|| format!("ler {}", path.display())),
    };
    write_private(path, with_value(&old, name, value).as_bytes())
}

/// Names defined in the file, sorted.
pub fn names(path: &Path) -> anyhow::Result<Vec<String>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("ler {}", path.display())),
    };
    Ok(parse(&text).into_keys().collect())
}

/// `text` with the first `name=` line replaced (later ones dropped), or the line appended.
fn with_value(text: &str, name: &str, value: &str) -> String {
    let mut out = String::new();
    let mut done = false;
    for line in text.lines() {
        let same = parse_line(line).is_some_and(|(n, _)| n == name);
        if same && done {
            continue;
        }
        if same {
            out.push_str(&format!("{name}={value}\n"));
            done = true;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !done {
        out.push_str(&format!("{name}={value}\n"));
    }
    out
}

/// Writes the file so only the current user can read it, then renames it in place.
///
/// Linux: mode 0600. Windows: a protected DACL with one entry for the current user,
/// set through the Win32 API (see `acl.rs`).
pub fn write_private(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let parent = path.parent().context("caminho sem pasta pai")?;
    std::fs::create_dir_all(parent).with_context(|| format!("criar pasta {}", parent.display()))?;
    let tmp = parent.join(format!(".secrets.env.tmp-{}", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    create_private(&tmp)?;
    std::fs::write(&tmp, bytes).with_context(|| format!("gravar {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("renomear para {}", path.display()))?;
    restrict(path)
}

/// Creates `dir` if needed and makes it reachable by the current user only.
///
/// Linux: mode 0700. Windows: a protected DACL with one inheritable entry for the
/// current user, so files copied in later get it too.
pub fn private_dir(dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir).with_context(|| format!("criar pasta {}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
            .with_context(|| format!("chmod 700 {}", dir.display()))
    }
    #[cfg(windows)]
    {
        super::acl::restrict_dir(dir)
    }
}

/// True if only the current user can access the file or folder.
pub fn is_private(path: &Path) -> anyhow::Result<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(path)?.permissions().mode();
        Ok(mode & 0o077 == 0)
    }
    #[cfg(windows)]
    {
        super::acl::is_private(path)
    }
}

#[cfg(unix)]
fn create_private(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("criar {}", path.display()))?;
    Ok(())
}

#[cfg(windows)]
fn create_private(path: &Path) -> anyhow::Result<()> {
    std::fs::File::create_new(path).with_context(|| format!("criar {}", path.display()))?;
    super::acl::restrict(path)
}

#[cfg(unix)]
fn restrict(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("chmod 600 {}", path.display()))
}

#[cfg(windows)]
fn restrict(path: &Path) -> anyhow::Result<()> {
    super::acl::restrict(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmp::TempDir;

    #[test]
    fn set_keeps_other_lines() {
        let text = "# team secrets\nA=1\n\nB=2\nA=dup\n";
        assert_eq!(
            with_value(text, "A", "new"),
            "# team secrets\nA=new\n\nB=2\n"
        );
        assert_eq!(with_value(text, "C", "3"), format!("{text}C=3\n"));
        assert_eq!(with_value("", "C", "3"), "C=3\n");
    }

    #[test]
    fn file_is_private_and_round_trips() {
        let dir = TempDir::new("agsync-test").unwrap();
        let file = dir.path().join("cfg").join("secrets.env");
        set(&file, "TOKEN", "abc").unwrap();
        set(&file, "OTHER", "x=y").unwrap();
        set(&file, "TOKEN", "def").unwrap();
        assert_eq!(names(&file).unwrap(), ["OTHER", "TOKEN"]);
        assert_eq!(
            parse(&std::fs::read_to_string(&file).unwrap())["TOKEN"],
            "def"
        );
        assert!(is_private(&file).unwrap());
        assert!(set(&file, "bad name", "x").is_err());
        assert!(set(&file, "A", "x\ny").is_err());
        let leftovers = std::fs::read_dir(file.parent().unwrap()).unwrap().count();
        assert_eq!(leftovers, 1);
    }

    #[test]
    fn private_dir_covers_files_copied_in() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let dir = tmp.path().join("backups");
        private_dir(&dir).unwrap();
        private_dir(&dir).unwrap();
        assert!(is_private(&dir).unwrap());
        let src = tmp.path().join("config.json");
        std::fs::write(&src, "{}").unwrap();
        let copy = dir.join("config.json.1");
        std::fs::copy(&src, &copy).unwrap();
        #[cfg(windows)]
        assert!(super::super::acl::only_user(&copy).unwrap());
    }
}
