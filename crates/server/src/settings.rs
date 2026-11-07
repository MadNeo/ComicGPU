use super::*;
use serde::{Deserialize, Serialize};
use std::{io::Write, path::PathBuf};
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub output_directory: String,
    pub workers: usize,
    pub profile: String,
    pub gpu: bool,
}
fn location() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join(".config")
        });
    base.join("ComicGPU").join("settings.json")
}
pub fn load() -> Settings {
    std::fs::read(location())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Settings {
            workers: 1,
            profile: "digital".into(),
            ..Default::default()
        })
}
pub async fn get_settings() -> Json<Settings> {
    Json(load())
}
pub async fn save_settings(Json(settings): Json<Settings>) -> ApiResult<Json<Settings>> {
    if !(1..=4).contains(&settings.workers)
        || !["digital", "scan", "manga"].contains(&settings.profile.as_str())
    {
        return Err(error("Ongeldige instellingen"));
    }
    if !settings.output_directory.is_empty() {
        let directory = std::path::Path::new(&settings.output_directory);
        if !directory.is_absolute() || !directory.is_dir() {
            return Err(error("Kies een bestaande absolute uitvoermap"));
        }
        tempfile::NamedTempFile::new_in(directory).map_err(error)?;
    }
    let path = location();
    std::fs::create_dir_all(path.parent().unwrap()).map_err(error)?;
    let mut file = tempfile::NamedTempFile::new_in(path.parent().unwrap()).map_err(error)?;
    file.write_all(&serde_json::to_vec_pretty(&settings).map_err(error)?)
        .map_err(error)?;
    file.persist(path).map_err(error)?;
    Ok(Json(settings))
}
pub async fn choose_folder() -> ApiResult<Json<serde_json::Value>> {
    #[cfg(windows)]
    {
        let folder = tokio::task::spawn_blocking(|| {
            rfd::FileDialog::new()
                .set_title("Uitvoermap kiezen")
                .pick_folder()
        })
        .await
        .map_err(error)?;
        return Ok(Json(
            serde_json::json!({"path":folder.map(|p| p.to_string_lossy().into_owned())}),
        ));
    }
    #[cfg(not(windows))]
    Err(error("Vul het pad naar een map op de server in"))
}
pub fn save_output(bytes: &[u8], filename: &str) -> anyhow::Result<Option<PathBuf>> {
    let settings = load();
    if settings.output_directory.is_empty() {
        return Ok(None);
    }
    let name = filename.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    let base = std::path::Path::new(&name)
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("comic");
    let mut file = tempfile::NamedTempFile::new_in(&settings.output_directory)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    let destination = PathBuf::from(settings.output_directory)
        .join(format!("{base}_verbeterd_{}.cbz", new_token()));
    file.persist_noclobber(&destination)?;
    Ok(Some(destination))
}
