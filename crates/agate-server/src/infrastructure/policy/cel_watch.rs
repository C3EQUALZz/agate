//! File-watch for the CEL policy: emit a signal whenever the policy file changes
//! on disk, so the composition root can trigger the same fail-safe
//! [`CelPolicyAdapter::reload`](super::CelPolicyAdapter::reload) that `SIGHUP`
//! does — just driven by the filesystem instead of a signal.

use std::path::Path;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

/// A live filesystem watch on the CEL policy file. Holds the OS watcher — which
/// stops watching the moment it is dropped — alongside the receiver that yields
/// one signal per detected change.
pub struct CelWatch {
    // Kept alive for the lifetime of the watch; dropping it ends the watch. Never
    // read directly (the events arrive through `changes`), hence the `_` prefix.
    _watcher: RecommendedWatcher,
    /// Yields `()` each time the policy file changed; signals are coalesced, so
    /// one received value may stand for a burst of filesystem events.
    pub changes: mpsc::Receiver<()>,
}

/// Watch `path`'s **parent directory** for changes to that file.
///
/// Watching the directory rather than the file inode is deliberate: editors and
/// deployment tools write atomically (temp file + rename), which replaces the
/// inode and would silently drop a file-level watch. Directory events are
/// filtered to `path`'s file name and coalesced into a bare "something changed"
/// signal — the reload re-reads the latest content regardless of how many
/// individual events a single save produced.
pub fn watch(path: &Path) -> Result<CelWatch, String> {
    let directory = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        // A bare filename has no parent component — watch the current directory.
        _ => Path::new(".").to_path_buf(),
    };
    let file_name = path
        .file_name()
        .ok_or_else(|| {
            format!(
                "CEL policy path '{}' has no file name to watch",
                path.display()
            )
        })?
        .to_os_string();

    let (tx, changes) = mpsc::channel(8);
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<Event>| {
        let Ok(event) = result else {
            return;
        };
        // The directory may hold other files; react only to ours.
        if event
            .paths
            .iter()
            .any(|changed| changed.file_name() == Some(file_name.as_os_str()))
        {
            // Coalesce: if the channel is already full there is a pending signal,
            // so a dropped send loses nothing — the reload reads the latest file.
            let _ = tx.try_send(());
        }
    })
    .map_err(|error| format!("cannot create the CEL policy watcher: {error}"))?;

    watcher
        .watch(&directory, RecursiveMode::NonRecursive)
        .map_err(|error| format!("cannot watch '{}': {error}", directory.display()))?;

    Ok(CelWatch {
        _watcher: watcher,
        changes,
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::watch;

    #[test]
    fn watching_a_path_without_a_file_name_errors() {
        // A path ending in `..` has no final file-name component.
        let Err(error) = watch(std::path::Path::new("some/dir/..")) else {
            panic!("a path with no file name must not be watchable");
        };
        assert!(error.contains("no file name"), "got: {error}");
    }

    #[tokio::test]
    async fn a_change_to_the_watched_file_is_signalled() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("policy.toml");
        std::fs::write(&path, "# initial\n").expect("write initial");

        let mut watch = watch(&path).expect("watch installs");

        // Modify the file a few times until the watcher reports a change; some
        // backends coalesce or briefly miss the first event after install.
        let signalled = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                std::fs::write(&path, "# changed\n").expect("rewrite");
                if tokio::time::timeout(Duration::from_millis(500), watch.changes.recv())
                    .await
                    .is_ok()
                {
                    return true;
                }
            }
        })
        .await
        .unwrap_or(false);

        assert!(
            signalled,
            "a change to the watched file should be signalled"
        );
    }
}
