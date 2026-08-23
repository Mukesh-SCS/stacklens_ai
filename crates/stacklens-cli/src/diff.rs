use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::Context;
use stacklens_core::{diff_snapshots, ChangeKind, EnvironmentSnapshot};

pub fn run(stdout: &mut dyn Write, left: &Path, right: &Path) -> anyhow::Result<()> {
    let left_snapshot = load(left)?;
    let right_snapshot = load(right)?;
    let diff = diff_snapshots(&left_snapshot, &right_snapshot)?;

    writeln!(stdout, "left captured_at:  {}", diff.left_captured_at)?;
    writeln!(stdout, "right captured_at: {}", diff.right_captured_at)?;
    writeln!(stdout)?;

    if diff.changes.is_empty() {
        writeln!(stdout, "No differences.")?;
        return Ok(());
    }

    for change in &diff.changes {
        writeln!(stdout, "{}", change.path)?;
        match &change.kind {
            ChangeKind::Added { right } => {
                writeln!(stdout, "  left:  (absent)")?;
                writeln!(stdout, "  right: {}", render_value(right))?;
            }
            ChangeKind::Removed { left } => {
                writeln!(stdout, "  left:  {}", render_value(left))?;
                writeln!(stdout, "  right: (absent)")?;
            }
            ChangeKind::Changed { left, right } => {
                writeln!(stdout, "  left:  {}", render_value(left))?;
                writeln!(stdout, "  right: {}", render_value(right))?;
            }
        }
        writeln!(stdout)?;
    }

    Ok(())
}

fn load(path: &Path) -> anyhow::Result<EnvironmentSnapshot> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read snapshot {}", path.display()))?;
    EnvironmentSnapshot::from_json_str(&text)
        .with_context(|| format!("failed to parse snapshot {}", path.display()))
}

fn render_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::run;
    use std::io::Write;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/snapshot")
            .join(name)
    }

    #[test]
    fn diffs_workspace_fixtures() {
        let mut buf = Vec::new();
        run(&mut buf, &fixture("left.json"), &fixture("right.json")).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("system.data.os"));
        assert!(text.contains("linux"));
        assert!(text.contains("windows"));
        assert!(text.contains("LD_LIBRARY_PATH"));
    }

    #[test]
    fn missing_file_is_an_error() {
        let mut buf = Vec::new();
        let err = run(
            &mut buf,
            &fixture("left.json"),
            &PathBuf::from("missing.json"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("failed to read snapshot"));
    }

    #[test]
    fn writes_temp_identical_snapshots() {
        let dir = std::env::temp_dir().join(format!("stacklens-diff-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("same.json");
        let body = std::fs::read_to_string(fixture("left.json")).unwrap();
        std::fs::File::create(&path)
            .unwrap()
            .write_all(body.as_bytes())
            .unwrap();

        let mut buf = Vec::new();
        run(&mut buf, &path, &path).unwrap();
        assert!(String::from_utf8(buf).unwrap().contains("No differences."));
        let _ = std::fs::remove_dir_all(dir);
    }
}
