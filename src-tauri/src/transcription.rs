use std::path::PathBuf;

use std::io::ErrorKind;

use tokio::process::Command;

use crate::{config::AppConfig, models};

pub async fn transcribe_file(config: &AppConfig, audio_path: PathBuf) -> anyhow::Result<String> {
    let model_path = models::selected_model_path(config)?;
    if !std::path::Path::new(&model_path).exists() {
        anyhow::bail!("selected Whisper model is not installed");
    }

    let mut command = Command::new(&config.whisper_binary_path);
    command
        .arg("-m")
        .arg(model_path)
        .arg("-f")
        .arg(&audio_path)
        .arg("-l")
        .arg(whisper_language(&config.language));

    if config.language == "bn" {
        command
            .arg("--prompt")
            .arg("বাংলা ভাষা। বাংলা লিপিতে লিখুন। উদাহরণ: আমি আজকে ভালো আছি। ধন্যবাদ।");
    }

    let output = command
        .arg("-nt")
        .output()
        .await
        .map_err(|error| {
            if error.kind() == ErrorKind::NotFound {
                anyhow::anyhow!(
                    "Whisper binary not found. Install whisper.cpp with `brew install whisper-cpp`, then set the Whisper binary path to `whisper-cli` or the full path shown by `which whisper-cli`."
                )
            } else {
                anyhow::anyhow!("failed to run whisper.cpp: {error}")
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("whisper.cpp failed: {}", stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn whisper_language(language: &str) -> &str {
    match language {
        "bn" => "bengali",
        value => value,
    }
}
