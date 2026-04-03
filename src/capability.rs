use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// JailFile — capability manifest for a sandbox (like Dockerfile)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JailFile {
    pub sandbox: SandboxMeta,
    #[serde(default)]
    pub capabilities: Capabilities,
    #[serde(default)]
    pub limits: Limits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxMeta {
    pub name: Option<String>,
    pub entrypoint: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Capabilities {
    #[serde(default)]
    pub fs_read: Vec<String>,
    #[serde(default)]
    pub fs_write: Vec<String>,
    #[serde(default)]
    pub net_allow: Vec<String>,
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub inherit_env: bool,
    #[serde(default = "default_true")]
    pub stdin: bool,
    #[serde(default = "default_true")]
    pub stdout: bool,
    #[serde(default = "default_true")]
    pub stderr: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Limits {
    pub memory_mb: Option<u64>,
    pub fuel: Option<u64>,
    pub wall_time_secs: Option<u64>,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            memory_mb: Some(256),
            fuel: Some(1_000_000_000),
            wall_time_secs: Some(300),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FsMount {
    pub host: PathBuf,
    pub guest: String,
    pub writable: bool,
}

#[derive(Debug, Clone)]
pub enum CapGrant {
    Fs(FsMount),
    Net(String),
    Env(Vec<String>),
}

impl CapGrant {
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        if let Some(rest) = s.strip_prefix("fs:read:") {
            Ok(CapGrant::Fs(FsMount {
                host: PathBuf::from(rest),
                guest: rest.to_string(),
                writable: false,
            }))
        } else if let Some(rest) = s.strip_prefix("fs:write:") {
            Ok(CapGrant::Fs(FsMount {
                host: PathBuf::from(rest),
                guest: rest.to_string(),
                writable: true,
            }))
        } else if let Some(rest) = s.strip_prefix("fs:") {
            Ok(CapGrant::Fs(FsMount {
                host: PathBuf::from(rest),
                guest: rest.to_string(),
                writable: true,
            }))
        } else if let Some(rest) = s.strip_prefix("net:") {
            Ok(CapGrant::Net(rest.to_string()))
        } else if let Some(rest) = s.strip_prefix("env:") {
            Ok(CapGrant::Env(rest.split(',').map(String::from).collect()))
        } else {
            anyhow::bail!(
                "Invalid capability: {s}\n\
                 Expected: fs:read:/path, fs:write:/path, fs:/path, net:host:port, env:VAR1,VAR2"
            )
        }
    }
}

pub struct ResolvedCaps {
    pub fs_mounts: Vec<FsMount>,
    pub net_rules: Vec<String>,
    pub env_vars: Vec<(String, String)>,
    pub inherit_stdio: bool,
}

impl ResolvedCaps {
    pub fn from_parts(
        base: &Capabilities,
        grants: &[CapGrant],
        volumes: &[String],
        env_overrides: &[String],
        net_flag: bool,
    ) -> Self {
        let mut fs_mounts = Vec::new();
        let mut net_rules: Vec<String> = base.net_allow.clone();
        let mut env_vars: Vec<(String, String)> = Vec::new();

        for path in &base.fs_read {
            fs_mounts.push(FsMount {
                host: PathBuf::from(path),
                guest: path.clone(),
                writable: false,
            });
        }
        for path in &base.fs_write {
            fs_mounts.push(FsMount {
                host: PathBuf::from(path),
                guest: path.clone(),
                writable: true,
            });
        }

        if base.inherit_env {
            for (k, v) in std::env::vars() {
                env_vars.push((k, v));
            }
        } else {
            for var in &base.env {
                if let Ok(val) = std::env::var(var) {
                    env_vars.push((var.clone(), val));
                }
            }
        }

        for grant in grants {
            match grant {
                CapGrant::Fs(mount) => fs_mounts.push(mount.clone()),
                CapGrant::Net(rule) => net_rules.push(rule.clone()),
                CapGrant::Env(vars) => {
                    for var in vars {
                        if let Ok(val) = std::env::var(var) {
                            env_vars.push((var.clone(), val));
                        }
                    }
                }
            }
        }

        for v in volumes {
            let (host, guest) = if let Some((h, g)) = v.split_once(':') {
                (h.to_string(), g.to_string())
            } else {
                (v.clone(), v.clone())
            };
            fs_mounts.push(FsMount {
                host: PathBuf::from(host),
                guest,
                writable: true,
            });
        }

        for e in env_overrides {
            if let Some((k, v)) = e.split_once('=') {
                env_vars.push((k.to_string(), v.to_string()));
            } else if let Ok(val) = std::env::var(e) {
                env_vars.push((e.to_string(), val));
            }
        }

        if net_flag {
            net_rules.push("*".to_string());
        }

        Self {
            fs_mounts,
            net_rules,
            env_vars,
            inherit_stdio: base.stdin || base.stdout || base.stderr,
        }
    }
}
