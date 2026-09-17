use anyhow::{ensure, Context, Result};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use tempfile::NamedTempFile;

/// Publish only a complete, flushed output, keeping any previous file on failure.
pub fn write_atomic<T>(
    path: &Path,
    write: impl FnOnce(&mut BufWriter<&mut File>) -> Result<T>,
) -> Result<T> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("Failed to create {}", parent.display()))?;
    let mut temporary = NamedTempFile::new_in(parent)
        .with_context(|| format!("Failed to create temporary output in {}", parent.display()))?;
    let result = {
        let mut writer = BufWriter::with_capacity(1024 * 1024, temporary.as_file_mut());
        let result = write(&mut writer)?;
        writer.flush().context("Failed to flush output")?;
        result
    };
    temporary
        .persist(path)
        .with_context(|| format!("Failed to publish {}", path.display()))?;
    Ok(result)
}

/// Catch aliases (including symlinks) before a command can overwrite its input.
pub fn ensure_distinct_paths(input: &Path, output: &Path) -> Result<()> {
    let input = input
        .canonicalize()
        .with_context(|| format!("Failed to resolve input {}", input.display()))?;
    if output.exists() {
        ensure!(
            input != output.canonicalize()?,
            "Input and output paths must be different"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_write_preserves_previous_output() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("result.txt");
        fs::write(&path, "previous")?;
        let result: Result<()> = write_atomic(&path, |writer| {
            writer.write_all(b"incomplete")?;
            anyhow::bail!("simulated failure");
        });
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(path)?, "previous");
        assert_eq!(fs::read_dir(directory.path())?.count(), 1);
        Ok(())
    }
}
