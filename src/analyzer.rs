//! Binary analysis for `codejail make`.
//!
//! Inspects a native executable to determine its type, dependencies,
//! and inferred capability requirements for JailFile generation.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Result of analyzing a native binary.
#[derive(Debug, Clone)]
pub struct BinaryAnalysis {
    /// Canonical absolute path to the binary.
    pub binary_path: PathBuf,
    /// Detected binary format.
    pub binary_type: BinaryType,
    /// Interpreter for scripts or ELF PT_INTERP.
    pub interpreter: Option<PathBuf>,
    /// Dynamically linked shared libraries.
    pub linked_libraries: Vec<LinkedLibrary>,
    /// Filesystem paths the binary needs read access to.
    pub inferred_fs_read: Vec<PathBuf>,
    /// Filesystem paths the binary needs write access to.
    pub inferred_fs_write: Vec<PathBuf>,
    /// Whether the binary likely needs network access.
    pub needs_network: bool,
    /// Environment variables the binary likely needs.
    pub inferred_env: Vec<String>,
    /// Human-readable analysis notes.
    pub notes: Vec<String>,
}

/// Type of binary detected.
#[derive(Debug, Clone, PartialEq)]
pub enum BinaryType {
    /// Native ELF executable.
    Elf,
    /// Interpreted script with a shebang line.
    Script { interpreter: PathBuf },
    /// Symbolic link (resolved, underlying type also analyzed).
    Symlink { target: PathBuf },
    /// Unknown format.
    Unknown,
}

/// A dynamically linked library.
#[derive(Debug, Clone)]
pub struct LinkedLibrary {
    pub name: String,
    pub path: Option<PathBuf>,
}

/// Analyze a native binary at the given path.
pub fn analyze(path: &Path) -> anyhow::Result<BinaryAnalysis> {
    let binary_path = fs::canonicalize(path)
        .map_err(|e| anyhow::anyhow!("cannot resolve '{}': {e}", path.display()))?;

    let meta = fs::metadata(&binary_path)?;
    if meta.permissions().mode() & 0o111 == 0 {
        anyhow::bail!(
            "'{}' is not executable (mode {:o})",
            binary_path.display(),
            meta.permissions().mode()
        );
    }

    let mut notes = Vec::new();

    // Detect symlinks on the original path
    let symlink_target = if fs::symlink_metadata(path)?.file_type().is_symlink() {
        Some(fs::read_link(path)?)
    } else {
        None
    };

    // Read first bytes for magic number detection
    let header = fs::read(&binary_path)
        .map(|b| b[..b.len().min(512)].to_vec())
        .unwrap_or_default();

    let (binary_type, interpreter) =
        detect_type(&binary_path, &header, &symlink_target, &mut notes);

    let linked_libraries = get_linked_libraries(&binary_path, &interpreter);

    let (inferred_fs_read, inferred_fs_write, needs_network, inferred_env) =
        infer_capabilities(&binary_path, &binary_type, &interpreter, &linked_libraries, &mut notes);

    Ok(BinaryAnalysis {
        binary_path,
        binary_type,
        interpreter,
        linked_libraries,
        inferred_fs_read,
        inferred_fs_write,
        needs_network,
        inferred_env,
        notes,
    })
}

fn detect_type(
    path: &Path,
    header: &[u8],
    symlink_target: &Option<PathBuf>,
    notes: &mut Vec<String>,
) -> (BinaryType, Option<PathBuf>) {
    if let Some(target) = symlink_target {
        notes.push(format!("symlink -> {}", target.display()));
        // Continue analysis on the resolved target — don't return early.
        // The symlink info is noted but we detect the real type below.
    }

    // ELF magic: 0x7f 'E' 'L' 'F'
    if header.len() >= 4 && header[..4] == *b"\x7fELF" {
        notes.push("ELF binary detected".into());
        let interp = read_elf_interpreter(path);
        if let Some(ref i) = interp {
            notes.push(format!("ELF interpreter: {}", i.display()));
        }
        return (BinaryType::Elf, interp);
    }

    // Shebang: #!
    if header.len() >= 2 && header[..2] == *b"#!" {
        let first_line = header
            .iter()
            .position(|&b| b == b'\n')
            .map(|pos| String::from_utf8_lossy(&header[2..pos]).trim().to_string())
            .unwrap_or_default();

        if let Some(interp) = parse_shebang(&first_line) {
            notes.push(format!("script interpreter: {}", interp.display()));
            return (
                BinaryType::Script {
                    interpreter: interp.clone(),
                },
                Some(interp),
            );
        }
    }

    notes.push("unknown binary format — treating as native executable".into());
    (BinaryType::Unknown, None)
}

fn parse_shebang(line: &str) -> Option<PathBuf> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }
    if parts[0] == "/usr/bin/env" && parts.len() > 1 {
        which(parts[1]).or_else(|| Some(PathBuf::from(parts[1])))
    } else {
        Some(PathBuf::from(parts[0]))
    }
}

/// Resolve a command name to its full path.
pub fn which(name: &str) -> Option<PathBuf> {
    Command::new("which")
        .arg(name)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()))
}

fn read_elf_interpreter(path: &Path) -> Option<PathBuf> {
    let output = Command::new("readelf")
        .args(["-l", "--wide"])
        .arg(path)
        .output()
        .ok()?;

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        if let Some(start_marker) = line.find("[Requesting program interpreter: ") {
            let start = start_marker + "[Requesting program interpreter: ".len();
            let end = line[start..].find(']').map(|i| start + i)?;
            return Some(PathBuf::from(&line[start..end]));
        }
    }
    None
}

fn get_linked_libraries(binary: &Path, interpreter: &Option<PathBuf>) -> Vec<LinkedLibrary> {
    // Try ldd on the binary itself first (works for ELF binaries)
    if let Ok(o) = Command::new("ldd").arg(binary).output() {
        if o.status.success() {
            let libs = parse_ldd_output(&String::from_utf8_lossy(&o.stdout));
            if !libs.is_empty() {
                return libs;
            }
        }
    }

    // For scripts, fall back to the interpreter's libraries
    if let Some(interp) = interpreter {
        if let Ok(o) = Command::new("ldd").arg(interp).output() {
            if o.status.success() {
                return parse_ldd_output(&String::from_utf8_lossy(&o.stdout));
            }
        }
    }

    Vec::new()
}

/// Parse ldd output into structured library information.
pub fn parse_ldd_output(text: &str) -> Vec<LinkedLibrary> {
    let mut libs = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("linux-vdso")
            || line.contains("statically linked")
        {
            continue;
        }

        if let Some((name_part, rest)) = line.split_once("=>") {
            let name = name_part.trim().to_string();
            let path = rest
                .trim()
                .split_whitespace()
                .next()
                .filter(|p| p.starts_with('/'))
                .map(PathBuf::from);
            libs.push(LinkedLibrary { name, path });
        } else if line.starts_with('/') {
            let path_str = line.split_whitespace().next().unwrap_or("");
            libs.push(LinkedLibrary {
                name: PathBuf::from(path_str)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                path: Some(PathBuf::from(path_str)),
            });
        }
    }
    libs
}

fn infer_capabilities(
    binary_path: &Path,
    binary_type: &BinaryType,
    interpreter: &Option<PathBuf>,
    libraries: &[LinkedLibrary],
    notes: &mut Vec<String>,
) -> (Vec<PathBuf>, Vec<PathBuf>, bool, Vec<String>) {
    let mut fs_read = Vec::new();
    let mut fs_write = Vec::new();
    let mut needs_network = false;
    let mut env_vars = vec![
        "PATH".to_string(),
        "HOME".to_string(),
        "TERM".to_string(),
        "LANG".to_string(),
        "USER".to_string(),
    ];

    // The binary itself needs to be readable
    fs_read.push(binary_path.to_path_buf());

    // Interpreter
    if let Some(interp) = interpreter {
        fs_read.push(interp.clone());
        if let Some(parent) = interp.parent() {
            fs_read.push(parent.to_path_buf());
        }
    }

    // Library directories
    let mut lib_dirs: Vec<PathBuf> = Vec::new();
    for lib in libraries {
        if let Some(path) = &lib.path {
            if let Some(dir) = path.parent() {
                if !lib_dirs.contains(&dir.to_path_buf()) {
                    lib_dirs.push(dir.to_path_buf());
                }
            }
        }
    }
    fs_read.extend(lib_dirs);

    // Standard system paths for dynamic linking
    for sys_path in ["/usr/lib", "/usr/lib64", "/lib", "/lib64", "/etc/ld.so.cache"] {
        let p = PathBuf::from(sys_path);
        if p.exists() && !fs_read.contains(&p) {
            fs_read.push(p);
        }
    }

    // Type-specific inference
    match binary_type {
        BinaryType::Script {
            interpreter: interp,
        } => {
            let interp_name = interp
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();

            if interp_name.contains("node") {
                notes.push("Node.js application detected — granting broad capabilities".into());
                needs_network = true;
                env_vars.extend(
                    [
                        "NODE_PATH",
                        "NODE_OPTIONS",
                        "NODE_ENV",
                        "npm_config_cache",
                        "XDG_CONFIG_HOME",
                        "XDG_DATA_HOME",
                        "XDG_CACHE_HOME",
                        "SHELL",
                        "COLORTERM",
                        "FORCE_COLOR",
                    ]
                    .iter()
                    .map(|s| s.to_string()),
                );

                // Node needs access to its module tree
                if let Some(parent) = binary_path.parent() {
                    fs_read.push(parent.to_path_buf());
                    // Walk up to find node_modules directories
                    let mut dir = parent.to_path_buf();
                    for _ in 0..10 {
                        let nm = dir.join("node_modules");
                        if nm.exists() {
                            fs_read.push(nm);
                        }
                        let pkg = dir.join("package.json");
                        if pkg.exists() {
                            // Found package root — include the whole directory
                            fs_read.push(dir.clone());
                            break;
                        }
                        if !dir.pop() {
                            break;
                        }
                    }
                }

                fs_write.push(PathBuf::from("/tmp"));
            } else if interp_name.contains("python") {
                notes.push("Python application detected".into());
                env_vars.extend(
                    ["PYTHONPATH", "VIRTUAL_ENV"]
                        .iter()
                        .map(|s| s.to_string()),
                );
            } else if interp_name.contains("bash") || interp_name.contains("sh") {
                notes.push("Shell script detected".into());
            }
        }
        BinaryType::Elf => {
            for lib in libraries {
                if lib.name.contains("ssl")
                    || lib.name.contains("crypto")
                    || lib.name.contains("curl")
                    || lib.name.contains("nghttp")
                {
                    needs_network = true;
                    notes.push(format!("network library: {}", lib.name));
                    break;
                }
            }
        }
        _ => {}
    }

    // Common paths
    if !fs_write.contains(&PathBuf::from("/tmp")) {
        fs_write.push(PathBuf::from("/tmp"));
    }
    fs_read.push(PathBuf::from("/dev"));
    fs_read.push(PathBuf::from("/proc/self"));
    // /etc for NSS, resolv.conf, etc.
    fs_read.push(PathBuf::from("/etc"));

    // Deduplicate
    fs_read.sort();
    fs_read.dedup();
    fs_write.sort();
    fs_write.dedup();
    env_vars.sort();
    env_vars.dedup();

    (fs_read, fs_write, needs_network, env_vars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_shebang_direct() {
        let result = parse_shebang("/usr/bin/python3");
        assert_eq!(result, Some(PathBuf::from("/usr/bin/python3")));
    }

    #[test]
    fn test_parse_shebang_env() {
        let result = parse_shebang("/usr/bin/env sh");
        assert!(result.is_some());
    }

    #[test]
    fn test_parse_shebang_empty() {
        assert_eq!(parse_shebang(""), None);
    }

    #[test]
    fn test_detect_elf() {
        let true_path = PathBuf::from("/bin/true");
        if true_path.exists() {
            let analysis = analyze(&true_path).unwrap();
            assert!(
                matches!(analysis.binary_type, BinaryType::Elf),
                "/bin/true should be ELF, got {:?}",
                analysis.binary_type
            );
        }
    }

    #[test]
    fn test_analyze_missing_binary() {
        let result = analyze(Path::new("/nonexistent/binary"));
        assert!(result.is_err());
    }

    #[test]
    fn test_linked_libraries_populated() {
        let ls_path = PathBuf::from("/bin/ls");
        if ls_path.exists() {
            let analysis = analyze(&ls_path).unwrap();
            // /bin/ls is dynamically linked on most Linux systems
            if !analysis.notes.iter().any(|n| n.contains("statically")) {
                assert!(
                    !analysis.linked_libraries.is_empty(),
                    "dynamically linked binary should have libraries"
                );
            }
        }
    }

    #[test]
    fn test_inferred_fs_read_includes_binary() {
        let true_path = PathBuf::from("/bin/true");
        if true_path.exists() {
            let canonical = true_path.canonicalize().unwrap();
            let analysis = analyze(&true_path).unwrap();
            assert!(
                analysis.inferred_fs_read.contains(&canonical),
                "inferred fs_read should include the binary itself"
            );
        }
    }

    #[test]
    fn test_parse_ldd_output() {
        let sample = "\tlinux-vdso.so.1 (0x00007ffd1a3d7000)\n\
                       \tlibc.so.6 => /lib/x86_64-linux-gnu/libc.so.6 (0x00007f1234)\n\
                       \t/lib64/ld-linux-x86-64.so.2 (0x00007f5678)\n";
        let libs = parse_ldd_output(sample);
        assert_eq!(libs.len(), 2, "should skip linux-vdso");
        assert_eq!(libs[0].name, "libc.so.6");
        assert_eq!(
            libs[0].path,
            Some(PathBuf::from("/lib/x86_64-linux-gnu/libc.so.6"))
        );
    }

    #[test]
    fn test_env_always_includes_basics() {
        let true_path = PathBuf::from("/bin/true");
        if true_path.exists() {
            let analysis = analyze(&true_path).unwrap();
            assert!(analysis.inferred_env.contains(&"PATH".to_string()));
            assert!(analysis.inferred_env.contains(&"HOME".to_string()));
            assert!(analysis.inferred_env.contains(&"TERM".to_string()));
        }
    }
}
