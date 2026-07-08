use directories_next::BaseDirs;
use eyre::{Context, eyre};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Debug)]
pub struct CacheEntry {
    pub path: PathBuf,
    pub file_name: String,
    pub len: u64,
    pub modified: Option<SystemTime>,
    pub kind: CacheKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheKind {
    Image,
    Video,
    Other,
}

impl CacheKind {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
            Self::Other => "other",
        }
    }
}

/// # Errors
///
/// Returns an error if the platform app data directory cannot be resolved.
pub fn default_cache_dir() -> eyre::Result<PathBuf> {
    let base_dirs = BaseDirs::new().ok_or_else(|| eyre!("could not resolve app data directory"))?;
    Ok(base_dirs
        .config_dir()
        .join("Discord")
        .join("Cache")
        .join("Cache_Data"))
}

/// # Errors
///
/// Returns an error if the cache directory cannot be read.
pub fn scan_cache_dir(path: &Path) -> eyre::Result<Vec<CacheEntry>> {
    let entries = fs::read_dir(path)
        .wrap_err_with(|| format!("failed to read cache directory {}", path.display()))?;
    let mut out = Vec::new();

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;
        if !metadata.is_file() {
            continue;
        }

        let file_name = path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into(),
        );

        out.push(CacheEntry {
            kind: sniff_kind(&path),
            path,
            file_name,
            len: metadata.len(),
            modified: metadata.modified().ok(),
        });
    }

    out.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| a.file_name.cmp(&b.file_name))
    });
    Ok(out)
}

#[must_use]
pub fn sniff_kind(path: &Path) -> CacheKind {
    let Ok(bytes) = read_prefix(path, 64 * 1024) else {
        return CacheKind::Other;
    };

    if bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(&[0xff, 0xd8, 0xff])
        || bytes.starts_with(b"GIF87a")
        || bytes.starts_with(b"GIF89a")
        || bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP")
    {
        return CacheKind::Image;
    }

    if bytes.len() > 12 && bytes.get(4..8) == Some(b"ftyp")
        || bytes.starts_with(b"\x1a\x45\xdf\xa3")
        || bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"AVI ")
    {
        return CacheKind::Video;
    }

    CacheKind::Other
}

/// # Errors
///
/// Returns an error if the file cannot be read or decoded as an image.
pub fn decode_image(path: &Path) -> eyre::Result<image::DynamicImage> {
    let bytes = fs::read(path)?;
    Ok(image::load_from_memory(&bytes)?)
}

fn read_prefix(path: &Path, max_len: usize) -> eyre::Result<Vec<u8>> {
    use std::io::Read;

    let mut file = fs::File::open(path)?;
    let mut bytes = vec![0; max_len];
    let read = file.read(&mut bytes)?;
    bytes.truncate(read);
    Ok(bytes)
}
