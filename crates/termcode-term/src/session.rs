use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    pub root: PathBuf,
    pub files: Vec<SessionFile>,
    pub active_tab: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionFile {
    pub path: PathBuf,
    pub cursor_line: usize,
    pub cursor_column: usize,
}

/// FNV-1a hash of byte slice. Stable across Rust versions (unlike DefaultHasher).
fn fnv1a_hash(bytes: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET_BASIS;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn session_path(root: &Path) -> PathBuf {
    let sessions_dir = termcode_config::default::config_dir().join("sessions");
    let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let hash = fnv1a_hash(canonical.as_os_str().as_encoded_bytes());
    sessions_dir.join(format!("{hash:016x}.json"))
}

pub fn save_session(session: &Session) -> anyhow::Result<()> {
    let path = session_path(&session.root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(session)?;
    std::fs::write(&path, json)?;
    Ok(())
}

pub fn clear_session(root: &Path) -> anyhow::Result<()> {
    let path = session_path(root);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

pub fn load_session(root: &Path) -> Option<Session> {
    let path = session_path(root);
    let content = std::fs::read_to_string(&path).ok()?;
    let mut session: Session = serde_json::from_str(&content).ok()?;
    session.files.retain(|f| f.path.exists());
    if session.files.is_empty() {
        return None;
    }
    Some(session)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_serialization_round_trip() {
        let session = Session {
            root: PathBuf::from("/tmp/test-project"),
            files: vec![
                SessionFile {
                    path: PathBuf::from("/tmp/test-project/main.rs"),
                    cursor_line: 10,
                    cursor_column: 5,
                },
                SessionFile {
                    path: PathBuf::from("/tmp/test-project/lib.rs"),
                    cursor_line: 0,
                    cursor_column: 0,
                },
            ],
            active_tab: 1,
        };

        let json = serde_json::to_string(&session).unwrap();
        let deserialized: Session = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.root, session.root);
        assert_eq!(deserialized.files.len(), 2);
        assert_eq!(deserialized.files[0].cursor_line, 10);
        assert_eq!(deserialized.active_tab, 1);
    }

    #[test]
    fn session_path_is_deterministic() {
        let root = PathBuf::from("/tmp/my-project");
        let path1 = session_path(&root);
        let path2 = session_path(&root);
        assert_eq!(path1, path2);
    }

    #[test]
    fn session_path_differs_for_different_roots() {
        let path1 = session_path(&PathBuf::from("/tmp/project-a"));
        let path2 = session_path(&PathBuf::from("/tmp/project-b"));
        assert_ne!(path1, path2);
    }
}
