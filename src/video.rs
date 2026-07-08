use eyre::{Context, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

/// # Errors
///
/// Returns an error if ffprobe is unavailable or cannot inspect the file.
pub fn probe(path: &Path) -> eyre::Result<String> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=format_name,duration,size,bit_rate:stream=index,codec_type,codec_name,width,height,duration",
            "-of",
            "default=noprint_wrappers=1",
        ])
        .arg(path)
        .output()
        .wrap_err("failed to run ffprobe")?;

    if !output.status.success() {
        bail!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// # Errors
///
/// Returns an error if ffmpeg is unavailable or cannot extract a thumbnail.
pub fn extract_thumbnail(input: &Path, output: &Path) -> eyre::Result<()> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let status = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(input)
        .args([
            "-frames:v",
            "1",
            "-vf",
            "scale='min(960,iw)':-1",
            "-f",
            "image2",
        ])
        .arg(output)
        .status()
        .wrap_err("failed to run ffmpeg")?;

    if !status.success() {
        bail!("ffmpeg failed with status {status}");
    }

    Ok(())
}

#[must_use]
pub fn thumbnail_path_for(input: &Path) -> PathBuf {
    let file_name = input
        .file_name()
        .map_or_else(|| "cache-file".into(), |name| name.to_string_lossy());
    std::env::temp_dir()
        .join("discord-cache-explorer")
        .join("thumbnails")
        .join(format!("{file_name}.png"))
}
