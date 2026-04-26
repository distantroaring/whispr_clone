mod audio;
mod cleanup;
mod config;
mod dictation;
mod models;
mod paste;
mod transcription;

use std::sync::Arc;

use config::AppConfig;
use dictation::DictationController;
use serde::Serialize;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, State,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[derive(Clone)]
pub struct AppState {
    config: Arc<config::ConfigStore>,
    dictation: Arc<DictationController>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptionOutput {
    text: String,
    debug: cleanup::TranscriptionDebug,
}

#[tauri::command]
async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    state
        .inner()
        .config
        .load()
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn save_config(config: AppConfig, state: State<'_, AppState>) -> Result<AppConfig, String> {
    state
        .inner()
        .config
        .save(&config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

#[tauri::command]
async fn list_models(state: State<'_, AppState>) -> Result<Vec<models::WhisperModel>, String> {
    let config = state
        .inner()
        .config
        .load()
        .map_err(|error| error.to_string())?;
    Ok(models::available_models(&config))
}

#[tauri::command]
async fn recommend_model() -> Result<String, String> {
    Ok(models::recommended_model_id())
}

#[tauri::command]
async fn reveal_models_folder(state: State<'_, AppState>) -> Result<(), String> {
    let config = state
        .inner()
        .config
        .load()
        .map_err(|error| error.to_string())?;
    models::ensure_models_dir(&config).map_err(|error| error.to_string())?;
    models::reveal_folder(config.models_dir.into())
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn download_model(
    model_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<models::WhisperModel>, String> {
    let config = state
        .inner()
        .config
        .load()
        .map_err(|error| error.to_string())?;
    models::download_model(&config, &model_id)
        .await
        .map_err(|error| error.to_string())?;
    Ok(models::available_models(&config))
}

#[tauri::command]
async fn check_ollama(state: State<'_, AppState>) -> Result<cleanup::OllamaStatus, String> {
    let config = state
        .inner()
        .config
        .load()
        .map_err(|error| error.to_string())?;
    cleanup::check_ollama(&config)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn transcribe_audio_file(
    path: String,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<TranscriptionOutput, String> {
    let config = state
        .inner()
        .config
        .load()
        .map_err(|error| error.to_string())?;
    let raw = transcription::transcribe_file(&config, path.into())
        .await
        .map_err(|error| error.to_string())?;
    let debug = cleanup::clean_with_debug(&config, &raw)
        .await
        .map_err(|error| error.to_string())?;
    paste::write_clipboard(&app, &debug.final_text).map_err(|error| error.to_string())?;
    Ok(TranscriptionOutput {
        text: debug.final_text.clone(),
        debug,
    })
}

#[tauri::command]
async fn start_dictation(state: State<'_, AppState>) -> Result<(), String> {
    state
        .inner()
        .dictation
        .start()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn stop_dictation(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Option<cleanup::TranscriptionDebug>, String> {
    let config = state
        .inner()
        .config
        .load()
        .map_err(|error| error.to_string())?;
    state
        .inner()
        .dictation
        .stop_transcribe_clean_and_paste(&config, &app)
        .await
        .map_err(|error| error.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    let expected = Shortcut::new(Some(Modifiers::ALT), Code::Space);
                    if shortcut != &expected {
                        return;
                    }

                    let state = app.state::<AppState>().inner().clone();
                    let app_handle = app.clone();
                    match event.state {
                        ShortcutState::Pressed => {
                            tauri::async_runtime::spawn(async move {
                                let _ = state.dictation.start().await;
                            });
                        }
                        ShortcutState::Released => {
                            tauri::async_runtime::spawn(async move {
                                if let Ok(config) = state.config.load() {
                                    if let Ok(Some(debug)) = state
                                        .dictation
                                        .stop_transcribe_clean_and_paste(&config, &app_handle)
                                        .await
                                    {
                                        if config.language == "bn" && config.bangla_debug_enabled {
                                            let _ = app_handle.emit("bangla-debug", &debug);
                                        }
                                    }
                                }
                            });
                        }
                    }
                })
                .build(),
        )
        .setup(|app| {
            let config_store = Arc::new(config::ConfigStore::new(app.handle())?);
            let state = AppState {
                config: config_store.clone(),
                dictation: Arc::new(DictationController::new()),
            };
            app.manage(state);

            let settings_i = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings_i, &quit_i])?;
            TrayIconBuilder::new()
                .icon(tray_icon())
                .menu(&menu)
                .show_menu_on_left_click(true)
                .build(app)?;

            app.on_menu_event(|app, event| match event.id().as_ref() {
                "settings" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "quit" => app.exit(0),
                _ => {}
            });

            let shortcut = Shortcut::new(Some(Modifiers::ALT), Code::Space);
            app.global_shortcut().register(shortcut)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            list_models,
            recommend_model,
            reveal_models_folder,
            download_model,
            check_ollama,
            transcribe_audio_file,
            start_dictation,
            stop_dictation
        ])
        .run(tauri::generate_context!())
        .expect("failed to run app");
}

fn tray_icon() -> Image<'static> {
    let mut rgba = Vec::with_capacity(64 * 64 * 4);
    for y in 0..64 {
        for x in 0..64 {
            let dx = x as f32 - 32.0;
            let dy = y as f32 - 32.0;
            let inside = (dx * dx + dy * dy).sqrt() < 28.0;
            let mic = inside
                && ((dx.abs() < 6.0 && dy.abs() < 19.0) || (dy.abs() < 4.0 && dx.abs() < 18.0));
            let (r, g, b, a) = if mic {
                (249, 251, 245, 255)
            } else if inside {
                (24, 61, 54, 255)
            } else {
                (0, 0, 0, 0)
            };
            rgba.extend_from_slice(&[r, g, b, a]);
        }
    }
    Image::new_owned(rgba, 64, 64)
}
