//! Resolves an installed application's icon from the local system.
//! Icons are copied into the app data directory so the timeline can show them
//! without contacting the network.

use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};

/// Returns a `data:` URL for the application icon, or `None` when the system
/// has no icon we can read.
pub fn data_url(cache_dir: &Path, app_name: &str, app_id: Option<&str>) -> Option<String> {
    let key = cache_key(app_name, app_id);
    let cached_png = cache_dir.join(format!("{key}.png"));
    let cached_svg = cache_dir.join(format!("{key}.svg"));
    if let Some(url) = read_cached(&cached_png, "image/png") {
        return Some(url);
    }
    if let Some(url) = read_cached(&cached_svg, "image/svg+xml") {
        return Some(url);
    }
    let (bytes, mime, ext) = locate(app_name, app_id)?;
    let _ = fs::create_dir_all(cache_dir);
    let dest = cache_dir.join(format!("{key}.{ext}"));
    fs::write(&dest, &bytes).ok()?;
    Some(to_data_url(&bytes, mime))
}

fn cache_key(app_name: &str, app_id: Option<&str>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(app_name.to_lowercase().as_bytes());
    hasher.update(app_id.unwrap_or("").as_bytes());
    hex_encode(&hasher.finalize())[..16].to_string()
}

fn read_cached(path: &Path, mime: &str) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    (!bytes.is_empty()).then(|| to_data_url(&bytes, mime))
}

fn to_data_url(bytes: &[u8], mime: &str) -> String {
    format!("data:{mime};base64,{}", base64_encode(bytes))
}

fn locate(app_name: &str, app_id: Option<&str>) -> Option<(Vec<u8>, &'static str, &'static str)> {
    #[cfg(target_os = "macos")]
    {
        if let Some(bytes) = macos_icon(app_name, app_id) {
            return Some((bytes, "image/png", "png"));
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(found) = linux_icon(app_name, app_id) {
            return Some(found);
        }
    }
    #[cfg(target_os = "windows")]
    {
        let _ = (app_name, app_id);
    }
    let _ = (app_name, app_id);
    None
}

#[cfg(target_os = "macos")]
fn macos_icon(app_name: &str, app_id: Option<&str>) -> Option<Vec<u8>> {
    let app = app_id
        .and_then(|id| app_bundle(Path::new(id)).map(|p| p.to_path_buf()))
        .or_else(|| app_by_name(app_name))?;
    let resources = app.join("Contents/Resources");
    let mut icns: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(&resources) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("icns") {
                icns.push(p);
            }
        }
    }
    icns.sort();
    for path in icns {
        if let Ok(bytes) = fs::read(&path) {
            if let Some(png) = largest_embedded_png(&bytes) {
                return Some(png);
            }
        }
        if let Some(png) = sips_png(&path) {
            return Some(png);
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn app_by_name(app_name: &str) -> Option<std::path::PathBuf> {
    let file = format!("{app_name}.app");
    let mut roots = vec![
        std::path::PathBuf::from("/Applications"),
        std::path::PathBuf::from("/System/Applications"),
        std::path::PathBuf::from("/System/Applications/Utilities"),
    ];
    if let Ok(home) = std::env::var("HOME") {
        roots.push(std::path::PathBuf::from(home).join("Applications"));
    }
    for root in roots {
        let direct = root.join(&file);
        if direct.is_dir() {
            return Some(direct);
        }
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("app") {
                continue;
            }
            if path
                .file_stem()
                .and_then(|s| s.to_str())
                .is_some_and(|stem| stem.eq_ignore_ascii_case(app_name))
            {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn sips_png(icns: &Path) -> Option<Vec<u8>> {
    let out = std::env::temp_dir().join(format!(
        "recall-icon-{}-{}.png",
        std::process::id(),
        icns.file_stem().and_then(|s| s.to_str()).unwrap_or("icon")
    ));
    let ok = std::process::Command::new("sips")
        .args(["-s", "format", "png"])
        .arg(icns)
        .arg("--out")
        .arg(&out)
        .status()
        .ok()?
        .success();
    if !ok {
        return None;
    }
    let bytes = fs::read(&out).ok()?;
    let _ = fs::remove_file(&out);
    (!bytes.is_empty()).then_some(bytes)
}

#[cfg(target_os = "macos")]
fn app_bundle(path: &Path) -> Option<&Path> {
    let mut current = Some(path);
    while let Some(p) = current {
        if p.extension().and_then(|e| e.to_str()) == Some("app") {
            return Some(p);
        }
        current = p.parent();
    }
    None
}

/// Apple `.icns` files often embed PNG images. Returns the largest one.
#[cfg(any(test, target_os = "macos"))]
pub fn largest_embedded_png(bytes: &[u8]) -> Option<Vec<u8>> {
    const SIG: &[u8] = b"\x89PNG\r\n\x1a\n";
    const IEND: &[u8] = b"IEND";
    let mut best: Option<Vec<u8>> = None;
    let mut start = 0;
    while let Some(rel) = bytes[start..].windows(SIG.len()).position(|w| w == SIG) {
        let pos = start + rel;
        let end = bytes[pos..]
            .windows(IEND.len())
            .position(|w| w == IEND)
            .map(|i| pos + i + IEND.len() + 4)?;
        if end <= bytes.len() {
            let chunk = bytes[pos..end].to_vec();
            if best.as_ref().map(|b| chunk.len() > b.len()).unwrap_or(true) {
                best = Some(chunk);
            }
        }
        start = pos + SIG.len();
    }
    best
}

#[cfg(target_os = "linux")]
fn linux_icon(
    app_name: &str,
    app_id: Option<&str>,
) -> Option<(Vec<u8>, &'static str, &'static str)> {
    let stem = app_id
        .and_then(|id| {
            Path::new(id)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| app_name.to_string());
    let mut names = vec![
        desktop_icon_name(&stem).unwrap_or_else(|| stem.clone()),
        stem.clone(),
        app_name.to_lowercase().replace(' ', "-"),
        app_name.to_lowercase().replace(' ', ""),
    ];
    names.sort();
    names.dedup();
    for icon_name in names {
        let absolute = std::path::PathBuf::from(&icon_name);
        if absolute.is_absolute() {
            if let Some(found) = read_icon_file(&absolute) {
                return Some(found);
            }
            continue;
        }
        if let Some(found) = find_named_icon(&icon_name) {
            return Some(found);
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn find_named_icon(icon_name: &str) -> Option<(Vec<u8>, &'static str, &'static str)> {
    for dir in data_dirs() {
        for candidate in [
            dir.join(format!("icons/hicolor/256x256/apps/{icon_name}.png")),
            dir.join(format!("icons/hicolor/128x128/apps/{icon_name}.png")),
            dir.join(format!("icons/hicolor/48x48/apps/{icon_name}.png")),
            dir.join(format!("icons/hicolor/scalable/apps/{icon_name}.svg")),
            dir.join(format!("pixmaps/{icon_name}.png")),
            dir.join(format!("pixmaps/{icon_name}.svg")),
        ] {
            if let Some(found) = read_icon_file(&candidate) {
                return Some(found);
            }
        }
        let themes = dir.join("icons");
        let Ok(entries) = fs::read_dir(&themes) else {
            continue;
        };
        for theme in entries.flatten() {
            for size in ["256x256", "128x128", "64x64", "48x48", "scalable"] {
                for ext in ["png", "svg"] {
                    let candidate = theme
                        .path()
                        .join(size)
                        .join("apps")
                        .join(format!("{icon_name}.{ext}"));
                    if let Some(found) = read_icon_file(&candidate) {
                        return Some(found);
                    }
                }
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn data_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = vec![
        std::path::PathBuf::from("/usr/share"),
        std::path::PathBuf::from("/usr/local/share"),
    ];
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(std::path::PathBuf::from(home).join(".local/share"));
    }
    dirs
}

#[cfg(target_os = "linux")]
fn desktop_icon_name(stem: &str) -> Option<String> {
    let mut roots = vec![
        std::path::PathBuf::from("/usr/share/applications"),
        std::path::PathBuf::from("/usr/local/share/applications"),
    ];
    if let Ok(home) = std::env::var("HOME") {
        roots.push(std::path::PathBuf::from(home).join(".local/share/applications"));
    }
    for root in roots {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            if !desktop_matches(&text, stem) {
                continue;
            }
            if let Some(icon) = desktop_field(&text, "Icon") {
                return Some(icon);
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn desktop_matches(text: &str, stem: &str) -> bool {
    let needle = stem.to_lowercase();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("Exec=") else {
            continue;
        };
        let token = rest.split_whitespace().next().unwrap_or("");
        let base = Path::new(token)
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase());
        if base.as_deref() == Some(needle.as_str()) || token.to_lowercase().contains(&needle) {
            return true;
        }
    }
    false
}

#[cfg(target_os = "linux")]
fn desktop_field(text: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    text.lines()
        .find_map(|line| line.strip_prefix(&prefix).map(|v| v.trim().to_string()))
        .filter(|v| !v.is_empty())
}

#[cfg(target_os = "linux")]
fn read_icon_file(path: &Path) -> Option<(Vec<u8>, &'static str, &'static str)> {
    let bytes = fs::read(path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    match path.extension().and_then(|e| e.to_str()) {
        Some("svg") => Some((bytes, "image/svg+xml", "svg")),
        Some("png") => Some((bytes, "image/png", "png")),
        _ => None,
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::largest_embedded_png;

    #[cfg(target_os = "linux")]
    #[test]
    fn finds_a_local_chrome_icon_when_installed() {
        let url = super::data_url(
            std::env::temp_dir().as_path(),
            "google-chrome",
            Some("/usr/bin/google-chrome"),
        );
        if std::path::Path::new("/usr/bin/google-chrome").exists()
            || std::path::Path::new("/usr/bin/google-chrome-stable").exists()
        {
            assert!(url.unwrap_or_default().starts_with("data:image/"));
        }
    }

    #[test]
    fn extracts_the_larger_embedded_png() {
        let png = b"\x89PNG\r\n\x1a\nIHDR....IEND\x00\x00\x00\x00";
        let bigger = b"\x89PNG\r\n\x1a\nIHDRxxxxxxxxIEND\x00\x00\x00\x00";
        let mut blob = b"icns".to_vec();
        blob.extend_from_slice(png);
        blob.extend_from_slice(bigger);
        let found = largest_embedded_png(&blob).unwrap();
        assert!(found.starts_with(b"\x89PNG"));
        assert!(found.len() > png.len());
    }
}
