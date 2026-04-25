use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::config::AppConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhisperModel {
    pub id: String,
    pub name: String,
    pub description: String,
    pub file_name: String,
    pub download_url: String,
    pub installed: bool,
    pub recommended: bool,
}

pub fn recommended_model_id() -> String {
    let memory_gb = approximate_memory_gb();
    if memory_gb >= 16 {
        "small".to_string()
    } else {
        "base".to_string()
    }
}

pub fn available_models(config: &AppConfig) -> Vec<WhisperModel> {
    let recommended = recommended_model_id();
    model_catalog()
        .into_iter()
        .map(|mut model| {
            model.installed = Path::new(&config.models_dir)
                .join(&model.file_name)
                .exists();
            model.recommended = model.id == recommended;
            model
        })
        .collect()
}

pub fn ensure_models_dir(config: &AppConfig) -> anyhow::Result<()> {
    fs::create_dir_all(&config.models_dir)?;
    Ok(())
}

pub async fn download_model(config: &AppConfig, model_id: &str) -> anyhow::Result<()> {
    ensure_models_dir(config)?;
    let model = model_catalog()
        .into_iter()
        .find(|candidate| candidate.id == model_id)
        .ok_or_else(|| anyhow::anyhow!("unknown model '{model_id}'"))?;
    let target = Path::new(&config.models_dir).join(&model.file_name);
    if target.exists() {
        return Ok(());
    }

    let bytes = reqwest::get(&model.download_url)
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    fs::write(target, bytes)?;
    Ok(())
}

pub async fn reveal_folder(path: PathBuf) -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).status().await?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer").arg(path).status().await?;
    }

    Ok(())
}

pub fn selected_model_path(config: &AppConfig) -> anyhow::Result<String> {
    let model = model_catalog()
        .into_iter()
        .find(|candidate| candidate.id == config.selected_model_id)
        .ok_or_else(|| anyhow::anyhow!("unknown model '{}'", config.selected_model_id))?;
    Ok(Path::new(&config.models_dir)
        .join(model.file_name)
        .to_string_lossy()
        .to_string())
}

fn model_catalog() -> Vec<WhisperModel> {
    vec![
        WhisperModel {
            id: "base".to_string(),
            name: "Base".to_string(),
            description: "Fast default for lighter Windows CPU machines.".to_string(),
            file_name: "ggml-base.bin".to_string(),
            download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin"
                .to_string(),
            installed: false,
            recommended: false,
        },
        WhisperModel {
            id: "small".to_string(),
            name: "Small".to_string(),
            description: "Balanced quality and speed for Apple Silicon and stronger CPUs."
                .to_string(),
            file_name: "ggml-small.bin".to_string(),
            download_url:
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin"
                    .to_string(),
            installed: false,
            recommended: false,
        },
        WhisperModel {
            id: "medium".to_string(),
            name: "Medium".to_string(),
            description: "Better accuracy, slower on CPU-only Windows machines.".to_string(),
            file_name: "ggml-medium.bin".to_string(),
            download_url:
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin"
                    .to_string(),
            installed: false,
            recommended: false,
        },
    ]
}

fn approximate_memory_gb() -> u64 {
    8
}
