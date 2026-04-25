use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub whisper_binary_path: String,
    pub models_dir: String,
    pub selected_model_id: String,
    pub cleanup_enabled: bool,
    pub ollama_url: String,
    pub ollama_model: String,
    pub language: String,
}

impl AppConfig {
    fn default_for(app: &AppHandle) -> anyhow::Result<Self> {
        let app_data = app
            .path()
            .app_data_dir()
            .map_err(|error| anyhow::anyhow!("unable to resolve app data directory: {error}"))?;
        let models_dir = app_data.join("models");

        Ok(Self {
            whisper_binary_path: "whisper-cli".to_string(),
            models_dir: models_dir.to_string_lossy().to_string(),
            selected_model_id: crate::models::recommended_model_id(),
            cleanup_enabled: true,
            ollama_url: "http://localhost:11434".to_string(),
            ollama_model: "llama3.2:3b".to_string(),
            language: "auto".to_string(),
        })
    }
}

pub struct ConfigStore {
    app: AppHandle,
    path: PathBuf,
}

impl ConfigStore {
    pub fn new(app: &AppHandle) -> anyhow::Result<Self> {
        let app_data = app
            .path()
            .app_config_dir()
            .map_err(|error| anyhow::anyhow!("unable to resolve config directory: {error}"))?;
        fs::create_dir_all(&app_data)?;
        Ok(Self {
            app: app.clone(),
            path: app_data.join("settings.json"),
        })
    }

    pub fn load(&self) -> anyhow::Result<AppConfig> {
        if !self.path.exists() {
            let config = AppConfig::default_for(&self.app)?;
            self.save(&config)?;
            return Ok(config);
        }

        let raw = fs::read_to_string(&self.path)?;
        Ok(serde_json::from_str(&raw)?)
    }

    pub fn save(&self, config: &AppConfig) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, serde_json::to_string_pretty(config)?)?;
        Ok(())
    }
}
