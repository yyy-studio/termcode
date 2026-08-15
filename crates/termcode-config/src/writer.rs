//! Surgical writes back into the user's TOML files.
//!
//! The settings UI has to persist single values into files the user also edits
//! by hand, so re-serialising a parsed [`crate::config::AppConfig`] is not an
//! option: it would drop every comment, reorder every key, and materialise
//! defaults the user never wrote. `toml_edit` keeps the document as-is and
//! replaces only the key that changed.

use std::io::Write;
use std::path::Path;

use anyhow::{Context, anyhow};
use toml_edit::{DocumentMut, Item, Table, Value};

/// Read the document at `path`, or start an empty one when the file does not
/// exist yet. A first-run user has no `config.toml` at all, and saving a
/// setting has to create one rather than fail.
fn read_document(path: &Path) -> anyhow::Result<DocumentMut> {
    match std::fs::read_to_string(path) {
        Ok(text) => text
            .parse::<DocumentMut>()
            .with_context(|| format!("{} is not valid TOML", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(DocumentMut::new()),
        Err(e) => Err(e).with_context(|| format!("cannot read {}", path.display())),
    }
}

/// Write `bytes` to `path` through a temporary file in the same directory, so
/// an interrupted save cannot leave the user with a truncated config.
fn write_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    let mut temp = tempfile::NamedTempFile::new_in(dir)?;
    temp.write_all(bytes)?;
    temp.flush()?;
    temp.persist(path)
        .with_context(|| format!("cannot write {}", path.display()))?;
    Ok(())
}

/// Walk down to the table holding the last key, creating intermediate tables
/// that do not exist yet.
///
/// Fails rather than overwrites when a key on the path is already something
/// other than a table: `ui = 5` in the file means the user's document does not
/// have the shape we are about to write into, and clobbering it would lose
/// data.
fn parent_table<'doc>(doc: &'doc mut DocumentMut, path: &[&str]) -> anyhow::Result<&'doc mut Item> {
    let mut item: &mut Item = doc.as_item_mut();
    for key in path {
        let table = item
            .as_table_like_mut()
            .ok_or_else(|| anyhow!("[{key}] cannot be created: the path is not a table"))?;
        if table.get(key).is_none() {
            let mut created = Table::new();
            // Only suppresses the header while the table stays empty, and a
            // value is always inserted right after this.
            created.set_implicit(true);
            table.insert(key, Item::Table(created));
        }
        item = table
            .get_mut(key)
            .expect("the key was just inserted when missing");
    }
    Ok(item)
}

/// Set `keys` (a dotted path, outermost first) to `value` in the TOML file at
/// `path`, leaving the rest of the document untouched.
pub fn set_value(path: &Path, keys: &[&str], value: Value) -> anyhow::Result<()> {
    let mut doc = read_document(path)?;
    let (last, parents) = keys
        .split_last()
        .ok_or_else(|| anyhow!("no key to write"))?;
    let table = parent_table(&mut doc, parents)?
        .as_table_like_mut()
        .ok_or_else(|| anyhow!("cannot write '{last}': the parent is not a table"))?;
    match table.get_mut(last) {
        // Replace the value in place. Re-inserting the key would drop the
        // decor it carries, and a comment written above a setting lives on the
        // key, not on the value.
        Some(existing) => {
            let decor = existing.as_value().map(|old| {
                let text = |raw: Option<&toml_edit::RawString>| {
                    raw.and_then(|r| r.as_str()).unwrap_or_default().to_string()
                };
                (text(old.decor().prefix()), text(old.decor().suffix()))
            });
            *existing = Item::Value(match decor {
                Some((prefix, suffix)) => value.decorated(prefix, suffix),
                None => value,
            });
        }
        None => {
            table.insert(last, Item::Value(value));
        }
    }
    write_atomic(path, doc.to_string().as_bytes())
}

/// Remove `keys` from the TOML file at `path`. A key that is not there is not
/// an error -- unbinding a key that only exists in the preset must still leave
/// the override file valid.
pub fn remove_value(path: &Path, keys: &[&str]) -> anyhow::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let mut doc = read_document(path)?;
    let (last, parents) = keys
        .split_last()
        .ok_or_else(|| anyhow!("no key to remove"))?;
    // A missing parent means there is nothing to remove; only a parent of the
    // wrong shape is worth reporting.
    let mut item: &mut Item = doc.as_item_mut();
    for key in parents {
        let table = item
            .as_table_like_mut()
            .ok_or_else(|| anyhow!("[{key}] is not a table"))?;
        match table.get_mut(key) {
            Some(next) => item = next,
            None => return Ok(()),
        }
    }
    if let Some(table) = item.as_table_like_mut() {
        table.remove(last);
    }
    write_atomic(path, doc.to_string().as_bytes())
}

/// The TOML path of a keybinding in `keybindings.toml`: `[global]` for keys
/// that apply everywhere, `[mode.<name>]` otherwise.
pub fn keybinding_path<'a>(mode: Option<&'a str>, key_sequence: &'a str) -> Vec<&'a str> {
    match mode {
        Some(mode) => vec!["mode", mode, key_sequence],
        None => vec!["global", key_sequence],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use toml_edit::value;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("termcode-writer-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn set_value_preserves_comments_and_order() {
        let path = temp_path("comments.toml");
        std::fs::write(
            &path,
            "# my editor\ntheme = \"one-dark\"\n\n[editor]\n# indentation\ntab_size = 4\nscroll_off = 5\n",
        )
        .unwrap();

        set_value(
            &path,
            &["editor", "tab_size"],
            value(2).into_value().unwrap(),
        )
        .unwrap();

        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.contains("# my editor"), "{after}");
        assert!(after.contains("# indentation"), "{after}");
        assert!(after.contains("tab_size = 2"), "{after}");
        // Untouched keys keep both their value and their position.
        assert!(after.find("tab_size").unwrap() < after.find("scroll_off").unwrap());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn set_value_creates_missing_file_and_tables() {
        let path = temp_path("fresh.toml");
        set_value(
            &path,
            &["keymap", "preset"],
            value("vim").into_value().unwrap(),
        )
        .unwrap();

        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.contains("[keymap]"), "{after}");
        assert!(after.contains("preset = \"vim\""), "{after}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn set_value_quotes_keys_that_need_it() {
        // Keybinding keys are the binding itself, so they carry `+` and `-`,
        // which cannot appear in a bare TOML key.
        let path = temp_path("keys.toml");
        set_value(
            &path,
            &["mode", "normal", "ctrl+k ctrl+p"],
            value("palette.open").into_value().unwrap(),
        )
        .unwrap();

        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.contains("\"ctrl+k ctrl+p\""), "{after}");
        // And it has to parse back into the same binding.
        let parsed: crate::keymap::KeybindingConfig = toml::from_str(&after).unwrap();
        assert_eq!(
            parsed.modes.normal.get("ctrl+k ctrl+p").map(String::as_str),
            Some("palette.open")
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn set_value_rejects_a_non_table_on_the_path() {
        let path = temp_path("conflict.toml");
        std::fs::write(&path, "editor = 5\n").unwrap();
        assert!(
            set_value(
                &path,
                &["editor", "tab_size"],
                value(2).into_value().unwrap()
            )
            .is_err()
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn remove_value_is_quiet_about_missing_keys() {
        let path = temp_path("remove.toml");
        std::fs::write(&path, "[mode.normal]\n\"ctrl+p\" = \"fuzzy.open\"\n").unwrap();

        remove_value(&path, &["mode", "normal", "ctrl+z"]).unwrap();
        remove_value(&path, &["mode", "insert", "ctrl+p"]).unwrap();
        remove_value(&path, &["mode", "normal", "ctrl+p"]).unwrap();

        let after = std::fs::read_to_string(&path).unwrap();
        assert!(!after.contains("fuzzy.open"), "{after}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn remove_value_on_a_missing_file_is_a_noop() {
        let path = temp_path("absent.toml");
        remove_value(&path, &["global", "ctrl+p"]).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn keybinding_path_splits_global_from_modes() {
        assert_eq!(keybinding_path(None, "ctrl+p"), vec!["global", "ctrl+p"]);
        assert_eq!(
            keybinding_path(Some("normal"), "g g"),
            vec!["mode", "normal", "g g"]
        );
    }
}
