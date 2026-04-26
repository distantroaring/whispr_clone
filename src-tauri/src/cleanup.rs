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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionDebug {
    pub raw: String,
    pub cleaned: String,
    pub final_text: String,
    pub cleanup_used: bool,
    pub fallback_reason: Option<String>,
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

pub async fn clean_with_debug(
    config: &AppConfig,
    transcript: &str,
) -> anyhow::Result<TranscriptionDebug> {
    let raw = transcript.trim().to_string();

    if raw.is_empty() {
        return Ok(debug_result(&raw, "", "", false, Some("Empty transcript")));
    }

    if !config.cleanup_enabled {
        return Ok(debug_result(
            &raw,
            "",
            &raw,
            false,
            Some("AI cleanup disabled"),
        ));
    }

    if config.language == "bn" {
        let raw_has_bengali = raw.chars().any(is_bengali_char);
        let raw_has_arabic = raw.chars().any(is_arabic_char);
        let raw_looks_english = looks_like_english_sentence(&raw);
        match clean_bangla_text(config, transcript).await {
            Ok(cleaned) => {
                let cleaned = cleaned.trim().to_string();
                if raw_looks_english && !raw_has_bengali {
                    return Ok(debug_result(
                        &raw,
                        &cleaned,
                        &raw,
                        false,
                        Some("Raw Whisper text looks English; Bangla cleanup translation was rejected"),
                    ));
                }
                if raw_has_arabic && !raw_has_bengali {
                    let final_text = if should_use_cleaned_text(config, &cleaned) {
                        cleaned.as_str()
                    } else {
                        &raw
                    };
                    return Ok(debug_result(
                        &raw,
                        &cleaned,
                        final_text,
                        should_use_cleaned_text(config, &cleaned),
                        Some("Low confidence: Whisper returned Arabic/Urdu script while Bangla was selected"),
                    ));
                }
                if should_use_cleaned_text(config, &cleaned) {
                    return Ok(debug_result(&raw, &cleaned, &cleaned, true, None));
                }
                return Ok(debug_result(
                    &raw,
                    &cleaned,
                    &raw,
                    false,
                    Some("Bangla cleanup did not produce Bengali script"),
                ));
            }
            Err(error) => {
                return Ok(debug_result(
                    &raw,
                    "",
                    &raw,
                    false,
                    Some(&format!("Bangla cleanup failed: {error}")),
                ));
            }
        }
    }

    match clean_text(config, transcript).await {
        Ok(cleaned) if should_use_cleaned_text(config, &cleaned) => {
            let cleaned = cleaned.trim().to_string();
            Ok(debug_result(&raw, &cleaned, &cleaned, true, None))
        }
        Ok(cleaned) if config.language == "bn" && needs_bengali_script_conversion(&cleaned) => {
            match convert_to_bengali_script(config, &cleaned).await {
                Ok(converted) if should_use_cleaned_text(config, &converted) => Ok(debug_result(
                    &raw,
                    &cleaned,
                    converted.trim(),
                    true,
                    Some("Converted cleaned text to Bengali script"),
                )),
                _ => Ok(debug_result(
                    &raw,
                    &cleaned,
                    &raw,
                    false,
                    Some("Bengali script conversion failed"),
                )),
            }
        }
        Ok(cleaned) => {
            if config.language == "bn" && needs_bengali_script_conversion(transcript) {
                return Ok(debug_result(
                    &raw,
                    &cleaned,
                    &raw,
                    false,
                    Some("Raw Bangla transcript needs Bengali script conversion"),
                ));
            }
            Ok(debug_result(
                &raw,
                &cleaned,
                &raw,
                false,
                Some("Cleanup result was rejected"),
            ))
        }
        Err(error) => Ok(debug_result(
            &raw,
            "",
            &raw,
            false,
            Some(&format!("Cleanup failed: {error}")),
        )),
    }
}

fn debug_result(
    raw: &str,
    cleaned: &str,
    final_text: &str,
    cleanup_used: bool,
    fallback_reason: Option<&str>,
) -> TranscriptionDebug {
    TranscriptionDebug {
        raw: raw.to_string(),
        cleaned: cleaned.to_string(),
        final_text: final_text.to_string(),
        cleanup_used,
        fallback_reason: fallback_reason.map(ToString::to_string),
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
        "You are correcting Bangla dictation.\n\nTask:\nConvert Bangla speech into correct, natural Bengali script.\n\nRules:\n- The input is intended to be Bangla speech recognized imperfectly by Whisper.\n- It may appear in Romanized Bangla, Bengali script with mistakes, Arabic/Urdu script, or mixed script.\n- Preserve the speaker's intended meaning.\n- Fix obvious ASR mistakes, word boundaries, punctuation, and spelling.\n- Use modern standard Bengali spelling.\n- Keep common English technical words only when the speaker clearly used English, but write Bangla words in Bengali script.\n- If the input is clear English, return the English text unchanged. Do not translate English into Bangla.\n- Do not add new information.\n- Do not replace unclear text with a common Bangla phrase.\n- Do not copy or imitate the examples unless the input clearly says the same thing.\n- If the input is too unclear to recover, return exactly: [unclear]\n- Return only the corrected text or [unclear].\n\nExamples:\nami bhalo achi -> আমি ভালো আছি।\najke ami office e jabo -> আজকে আমি অফিসে যাবো।\namar nam rafi -> আমার নাম রাফি।\nami ekta email likhte chai -> আমি একটা ইমেইল লিখতে চাই।\nYou can hear me -> You can hear me\n\nInput:\n{text}\n\nCorrect text:"
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
    if cleaned.eq_ignore_ascii_case("[unclear]") {
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

fn looks_like_english_sentence(text: &str) -> bool {
    let words: Vec<String> = text
        .split(|char: char| !char.is_ascii_alphabetic())
        .filter(|word| !word.is_empty())
        .map(|word| word.to_ascii_lowercase())
        .collect();

    if words.len() < 3 {
        return false;
    }

    let english_hits = words
        .iter()
        .filter(|word| {
            matches!(
                word.as_str(),
                "a" | "an"
                    | "and"
                    | "are"
                    | "be"
                    | "can"
                    | "could"
                    | "do"
                    | "for"
                    | "from"
                    | "have"
                    | "hear"
                    | "hello"
                    | "i"
                    | "in"
                    | "is"
                    | "it"
                    | "me"
                    | "my"
                    | "of"
                    | "on"
                    | "please"
                    | "that"
                    | "the"
                    | "this"
                    | "to"
                    | "was"
                    | "we"
                    | "what"
                    | "with"
                    | "you"
                    | "your"
            )
        })
        .count();

    english_hits >= 2
}
