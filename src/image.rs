use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
}

/// Local image store at ~/.containment/images/
pub struct ImageStore {
    dir: PathBuf,
}

impl ImageStore {
    pub fn new() -> anyhow::Result<Self> {
        let dir = crate::container::containment_home().join("images");
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    /// Resolve an image name or path to an actual .wasm file path
    pub fn resolve(&self, name_or_path: &str) -> anyhow::Result<PathBuf> {
        // Direct path
        let p = PathBuf::from(name_or_path);
        if p.exists() {
            return Ok(std::fs::canonicalize(p)?);
        }
        // Image store lookup
        let stored = self.dir.join(format!("{name_or_path}.wasm"));
        if stored.exists() {
            return Ok(stored);
        }
        // Try without extension
        let stored2 = self.dir.join(name_or_path);
        if stored2.exists() {
            return Ok(stored2);
        }
        anyhow::bail!(
            "Image not found: {name_or_path}\nTry a path to a .wasm file or import one with `containment import`"
        )
    }

    /// Import a .wasm file into the image store
    pub fn import(&self, name: &str, src: &Path) -> anyhow::Result<Image> {
        anyhow::ensure!(src.exists(), "Source file not found: {}", src.display());
        let dest = self.dir.join(format!("{name}.wasm"));
        std::fs::copy(src, &dest)?;
        let size = std::fs::metadata(&dest)?.len();
        Ok(Image {
            name: name.to_string(),
            path: dest,
            size,
        })
    }

    pub fn list(&self) -> anyhow::Result<Vec<Image>> {
        let mut out = Vec::new();
        if !self.dir.exists() {
            return Ok(out);
        }
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "wasm") {
                let name = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let size = entry.metadata()?.len();
                out.push(Image { name, path, size });
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    pub fn remove(&self, name: &str) -> anyhow::Result<()> {
        let path = self.dir.join(format!("{name}.wasm"));
        anyhow::ensure!(path.exists(), "Image not found: {name}");
        std::fs::remove_file(path)?;
        Ok(())
    }
}
