use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::config::AppConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaStatus {
    pub available: bool,
    pub message: String,
}

#[derive(Serialize)]
struct GenerateRequest<'a> {
    model: &'a str,
    prompt: String,
    stream: bool,
    options: serde_json::Value,
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
}

pub async fn check_ollama(config: &AppConfig) -> anyhow::Result<OllamaStatus> {
    let url = format!("{}/api/tags", config.ollama_url.trim_end_matches('/'));
    let response = Client::new().get(url).send().await;
    match response {
        Ok(value) if value.status().is_success() => Ok(OllamaStatus {
            available: true,
            message: "Ollama is running".to_string(),
        }),
        Ok(value) => Ok(OllamaStatus {
            available: false,
            message: format!("Ollama responded with {}", value.status()),
        }),
        Err(_) => Ok(OllamaStatus {
            available: false,
            message: "Ollama is not reachable".to_string(),
        }),
    }
}

pub async fn clean_or_fallback(config: &AppConfig, transcript: &str) -> anyhow::Result<String> {
    if !config.cleanup_enabled || transcript.trim().is_empty() {
        return Ok(transcript.trim().to_string());
    }

    if config.language == "bn" {
        if let Ok(cleaned) = clean_bangla_text(config, transcript).await {
            if should_use_cleaned_text(config, &cleaned) {
                return Ok(cleaned.trim().to_string());
            }
        }
        return Ok(transcript.trim().to_string());
    }

    match clean_text(config, transcript).await {
        Ok(cleaned) if should_use_cleaned_text(config, &cleaned) => Ok(cleaned.trim().to_string()),
        Ok(cleaned) if config.language == "bn" && needs_bengali_script_conversion(&cleaned) => {
            match convert_to_bengali_script(config, &cleaned).await {
                Ok(converted) if should_use_cleaned_text(config, &converted) => {
                    Ok(converted.trim().to_string())
                }
                _ => Ok(transcript.trim().to_string()),
            }
        }
        _ => {
            if config.language == "bn" && needs_bengali_script_conversion(transcript) {
                return Ok(transcript.trim().to_string());
            }
            Ok(transcript.trim().to_string())
        }
    }
}

async fn clean_text(config: &AppConfig, transcript: &str) -> anyhow::Result<String> {
    let url = format!("{}/api/generate", config.ollama_url.trim_end_matches('/'));
    let language_instruction = match config.language.as_str() {
        "bn" => "The user selected Bangla. Return Bengali script only for Bangla speech. Do not use Arabic, Urdu, Hindi, Devanagari, Romanized Bangla, or English transliteration.",
        "en" => "The user selected English. Return natural English text.",
        _ => "Preserve the detected language and script. If the transcript is Bangla, return Bengali script, not Romanized Bangla.",
    };
    let prompt = format!(
        "Clean this dictation transcript before paste. Fix punctuation, casing, grammar, and obvious speech artifacts. {language_instruction} Return only the cleaned text.\n\nTranscript:\n{transcript}"
    );

    let response = Client::new()
        .post(url)
        .json(&GenerateRequest {
            model: &config.ollama_model,
            prompt,
            stream: false,
            options: json!({
                "temperature": 0.1,
                "top_p": 0.8
            }),
        })
        .send()
        .await?
        .error_for_status()?
        .json::<GenerateResponse>()
        .await?;

    Ok(response.response)
}

async fn clean_bangla_text(config: &AppConfig, text: &str) -> anyhow::Result<String> {
    let url = format!("{}/api/generate", config.ollama_url.trim_end_matches('/'));
    let prompt = format!(
        "You are correcting Bangla dictation.\n\nTask:\nConvert the input into correct, natural Bengali script.\n\nRules:\n- The input is Bangla speech recognized imperfectly by Whisper.\n- It may appear in Romanized Bangla, Bengali script with mistakes, Arabic/Urdu script, or mixed script.\n- Preserve the speaker's intended meaning.\n- Fix obvious ASR mistakes, word boundaries, punctuation, and spelling.\n- Use modern standard Bengali spelling.\n- Keep common English technical words only when the speaker clearly used English, but write Bangla words in Bengali script.\n- Do not translate the whole sentence into English.\n- Do not add new information.\n- Return only the corrected Bengali text.\n\nExamples:\nami bhalo achi -> আমি ভালো আছি।\napni kemon achen -> আপনি কেমন আছেন?\najke ami office e jabo -> আজকে আমি অফিসে যাবো।\namar nam rafi -> আমার নাম রাফি।\nami ekta email likhte chai -> আমি একটা ইমেইল লিখতে চাই।\n\nInput:\n{text}\n\nCorrect Bengali:"
    );

    let response = Client::new()
        .post(url)
        .json(&GenerateRequest {
            model: &config.ollama_model,
            prompt,
            stream: false,
            options: json!({
                "temperature": 0.05,
                "top_p": 0.7,
                "repeat_penalty": 1.05
            }),
        })
        .send()
        .await?
        .error_for_status()?
        .json::<GenerateResponse>()
        .await?;

    Ok(response.response)
}

async fn convert_to_bengali_script(config: &AppConfig, text: &str) -> anyhow::Result<String> {
    let url = format!("{}/api/generate", config.ollama_url.trim_end_matches('/'));
    let prompt = format!(
        "Convert Bangla speech text into Bengali script only.\n\nRules:\n- The input may be Romanized Bangla, English-looking Bangla, Arabic script, or Urdu script.\n- Transliterate the Bangla sounds into natural Bengali letters.\n- Do not translate to English.\n- Do not explain.\n- Do not use Roman, Arabic, Urdu, Hindi, or Devanagari letters.\n- Return only Bengali text.\n\nExamples:\nami bhalo achi -> আমি ভালো আছি\napni kemon achen -> আপনি কেমন আছেন\najke ami office e jabo -> আজকে আমি অফিসে যাবো\namar nam rafi -> আমার নাম রাফি\n\nInput:\n{text}\n\nBengali script:"
    );

    let response = Client::new()
        .post(url)
        .json(&GenerateRequest {
            model: &config.ollama_model,
            prompt,
            stream: false,
            options: json!({
                "temperature": 0.05,
                "top_p": 0.7
            }),
        })
        .send()
        .await?
        .error_for_status()?
        .json::<GenerateResponse>()
        .await?;

    Ok(response.response)
}

fn should_use_cleaned_text(config: &AppConfig, cleaned: &str) -> bool {
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        return false;
    }

    if config.language == "bn" {
        let bengali_chars = cleaned
            .chars()
            .filter(|char| is_bengali_char(*char))
            .count();
        let arabic_chars = cleaned.chars().filter(|char| is_arabic_char(*char)).count();
        return bengali_chars > 0 && arabic_chars == 0;
    }

    true
}

fn needs_bengali_script_conversion(text: &str) -> bool {
    let bengali_chars = text.chars().filter(|char| is_bengali_char(*char)).count();
    let arabic_chars = text.chars().filter(|char| is_arabic_char(*char)).count();
    let latin_chars = text.chars().filter(|char| is_latin_char(*char)).count();
    (arabic_chars > 0 || latin_chars > 0) && bengali_chars == 0
}

fn is_bengali_char(char: char) -> bool {
    ('\u{0980}'..='\u{09FF}').contains(&char)
}

fn is_arabic_char(char: char) -> bool {
    ('\u{0600}'..='\u{06FF}').contains(&char)
        || ('\u{0750}'..='\u{077F}').contains(&char)
        || ('\u{08A0}'..='\u{08FF}').contains(&char)
}

fn is_latin_char(char: char) -> bool {
    char.is_ascii_alphabetic()
}
