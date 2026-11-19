//! Platform-neutral file backend. Android implements this over SAF/JNI.

use std::fs::File;
use std::io;

#[derive(Debug, Clone)]
pub struct FileMeta {
    pub name: String,
    pub size: u64,
    /// Unix seconds.
    pub mtime: u64,
    pub mime: String,
}

pub struct Opened {
    pub file: File,
    /// `None` when the provider cannot report a size (then the body is chunked, like Ktor did).
    pub len: Option<u64>,
    pub mime: String,
}

pub struct Created {
    /// Actual stored name; a provider may pick a different one than requested.
    pub name: String,
    pub file: File,
}

/// All calls are blocking and are invoked from `spawn_blocking`.
pub trait Fs: Send + Sync {
    fn list(&self) -> Result<Vec<FileMeta>, String>;
    fn open_read(&self, name: &str) -> Result<Opened, String>;
    fn create(&self, name: &str) -> Result<Created, String>;
    fn delete(&self, name: &str) -> Result<(), String>;
}

/// Host-side hooks (notifications, IP prompts). Android overrides these via JNI.
pub trait Hooks: Send + Sync {
    fn approve_ip(&self, _ip: &str) -> bool {
        true
    }
    fn on_change(&self) {}
}

pub struct NoHooks;

impl Hooks for NoHooks {}

/// Rejects names that are not a single path segment.
pub fn safe_name(name: &str) -> Result<&str, String> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') {
        return Err(format!("invalid file name: {name:?}"));
    }
    Ok(name)
}

/// Mirrors KtorServer.kt: `replace(Regex("""(^\s+|\s+$|^\.\.|[\\/])"""), "_")`.
pub fn sanitize_name(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 1);
    let rest = match raw.strip_prefix("..") {
        Some(rest) => {
            out.push('_');
            rest
        }
        None => raw,
    };
    let lead_trimmed = rest.trim_start();
    if lead_trimmed.len() != rest.len() {
        out.push('_');
    }
    let end_trimmed = lead_trimmed.trim_end();
    out.push_str(end_trimmed);
    if end_trimmed.len() != lead_trimmed.len() {
        out.push('_');
    }
    out.chars()
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect()
}

/// Mirrors FileUtils.generateUniqueFileName: `name.ext`, then `name_2.ext`, `name_3.ext`...
pub fn unique_name(desired: &str, taken: impl Fn(&str) -> bool) -> String {
    let (base, ext) = match desired.rfind('.') {
        Some(i) => (&desired[..i], &desired[i + 1..]),
        None => (desired, ""),
    };
    let plain = format!("{base}.{ext}");
    if !taken(&plain) {
        return plain;
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}_{n}.{ext}");
        if !taken(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// Mirrors FileUtils.formatFileSize.
pub fn format_size(size: u64) -> String {
    if size == 0 {
        return "0 B".into();
    }
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let groups = ((size as f64).log10() / 1024f64.log10()) as usize;
    let groups = groups.min(UNITS.len() - 1);
    format!(
        "{:.1} {}",
        size as f64 / 1024f64.powi(groups as i32),
        UNITS[groups]
    )
}

/// A directory on the local filesystem (host testing, and the Linux reference build).
pub struct LocalFs {
    root: std::path::PathBuf,
}

impl LocalFs {
    pub fn new(root: impl Into<std::path::PathBuf>) -> io::Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root)?;
        Ok(Self {
            root: root.canonicalize()?,
        })
    }

    fn path(&self, name: &str) -> Result<std::path::PathBuf, String> {
        Ok(self.root.join(safe_name(name)?))
    }
}

impl Fs for LocalFs {
    fn list(&self) -> Result<Vec<FileMeta>, String> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&self.root).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let meta = match entry.metadata() {
                Ok(m) if m.is_file() => m,
                _ => continue,
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            out.push(FileMeta {
                mime: mime_guess::from_path(&name)
                    .first_or_octet_stream()
                    .to_string(),
                name,
                size: meta.len(),
                mtime,
            });
        }
        Ok(out)
    }

    fn open_read(&self, name: &str) -> Result<Opened, String> {
        let path = self.path(name)?;
        let file = File::open(&path).map_err(|_| format!("File not found: {name}"))?;
        let len = file.metadata().ok().map(|m| m.len()).filter(|&n| n > 0);
        Ok(Opened {
            file,
            len,
            mime: mime_guess::from_path(&path)
                .first_or_octet_stream()
                .to_string(),
        })
    }

    fn create(&self, name: &str) -> Result<Created, String> {
        let name = safe_name(name)?;
        let final_name = unique_name(name, |candidate| self.root.join(candidate).exists());
        let path = self.root.join(&final_name);
        let file = File::create(&path).map_err(|e| format!("Failed to create file: {e}"))?;
        Ok(Created {
            name: final_name,
            file,
        })
    }

    fn delete(&self, name: &str) -> Result<(), String> {
        let path = self.path(name)?;
        if !path.exists() {
            return Err(format!("File not found: {name}"));
        }
        std::fs::remove_file(&path).map_err(|e| format!("Failed to delete file: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_like_kotlin() {
        assert_eq!(sanitize_name("  a b  "), "_a b_");
        assert_eq!(sanitize_name("../../etc/passwd"), "__.._etc_passwd");
        assert_eq!(sanitize_name("dir/file.txt"), "dir_file.txt");
        assert_eq!(sanitize_name("win\\path"), "win_path");
        assert_eq!(sanitize_name("normal.txt"), "normal.txt");
        assert_eq!(sanitize_name("..x"), "_x");
    }

    #[test]
    fn unique_names() {
        let mut taken: Vec<String> = vec![];
        assert_eq!(unique_name("a.txt", |c| taken.iter().any(|t| t == c)), "a.txt");
        taken.push("a.txt".into());
        assert_eq!(unique_name("a.txt", |c| taken.iter().any(|t| t == c)), "a_2.txt");
        taken.push("a_2.txt".into());
        assert_eq!(
            unique_name("archive.tar.gz", |c| taken.iter().any(|t| t == c)),
            "archive.tar.gz"
        );
        assert_eq!(unique_name("noext", |_| false), "noext.");
    }

    #[test]
    fn sizes() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512.0 B");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(5 * 1024 * 1024 * 1024), "5.0 GB");
    }

    #[test]
    fn rejects_traversal() {
        assert!(safe_name("../x").is_err());
        assert!(safe_name("a/b").is_err());
        assert!(safe_name("..").is_err());
        assert!(safe_name("ok.txt").is_ok());
    }
}
