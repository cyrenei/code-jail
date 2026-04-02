use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    pub id: String,
    pub short_id: String,
    pub name: String,
    pub image: String,
    pub status: ContainerStatus,
    pub pid: Option<u32>,
    pub created: DateTime<Utc>,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContainerStatus {
    Running,
    Exited(i32),
    Failed(String),
}

impl std::fmt::Display for ContainerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "Running"),
            Self::Exited(code) => write!(f, "Exited ({code})"),
            Self::Failed(e) => {
                let msg = if e.len() > 30 { &e[..30] } else { e };
                write!(f, "Failed: {msg}")
            }
        }
    }
}

impl Container {
    pub fn new(name: &str, image: &str, caps: &[String]) -> Self {
        let id = Uuid::new_v4().to_string();
        let short_id = id[..12].to_string();
        Self {
            id,
            short_id,
            name: name.to_string(),
            image: image.to_string(),
            status: ContainerStatus::Running,
            pid: None,
            created: Utc::now(),
            capabilities: caps.to_vec(),
        }
    }
}

/// Persistent store for container metadata
pub struct ContainerStore {
    dir: PathBuf,
}

impl ContainerStore {
    pub fn new() -> anyhow::Result<Self> {
        let dir = containment_home().join("containers");
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    pub fn save(&self, c: &Container) -> anyhow::Result<()> {
        let path = self.dir.join(format!("{}.json", c.id));
        std::fs::write(path, serde_json::to_string_pretty(c)?)?;
        Ok(())
    }

    pub fn load(&self, id_or_prefix: &str) -> anyhow::Result<Container> {
        // Exact match
        let exact = self.dir.join(format!("{id_or_prefix}.json"));
        if exact.exists() {
            return Ok(serde_json::from_str(&std::fs::read_to_string(exact)?)?);
        }
        // Prefix match
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            let fname = entry.file_name();
            let name = fname.to_string_lossy();
            if name.starts_with(id_or_prefix) && name.ends_with(".json") {
                return Ok(serde_json::from_str(&std::fs::read_to_string(
                    entry.path(),
                )?)?);
            }
        }
        // Name match
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            let json = std::fs::read_to_string(entry.path())?;
            if let Ok(c) = serde_json::from_str::<Container>(&json)
                && c.name == id_or_prefix
            {
                return Ok(c);
            }
        }
        anyhow::bail!("Container not found: {id_or_prefix}")
    }

    pub fn list(&self) -> anyhow::Result<Vec<Container>> {
        let mut out = Vec::new();
        if !self.dir.exists() {
            return Ok(out);
        }
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            if entry.path().extension().is_some_and(|e| e == "json") {
                let json = std::fs::read_to_string(entry.path())?;
                if let Ok(c) = serde_json::from_str::<Container>(&json) {
                    out.push(c);
                }
            }
        }
        out.sort_by(|a, b| b.created.cmp(&a.created));
        Ok(out)
    }

    pub fn remove(&self, id_or_prefix: &str) -> anyhow::Result<()> {
        let c = self.load(id_or_prefix)?;
        std::fs::remove_file(self.dir.join(format!("{}.json", c.id)))?;
        Ok(())
    }

    /// Remove all stopped/failed containers
    pub fn prune(&self) -> anyhow::Result<usize> {
        let mut count = 0;
        for c in self.list()? {
            if !matches!(c.status, ContainerStatus::Running) {
                self.remove(&c.id)?;
                count += 1;
            }
        }
        Ok(count)
    }
}

pub fn containment_home() -> PathBuf {
    std::env::var("CONTAINMENT_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("HOME")
                .map(|h| PathBuf::from(h).join(".containment"))
                .unwrap_or_else(|_| PathBuf::from("/tmp/.containment"))
        })
}
