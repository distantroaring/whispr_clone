import { invoke } from "@tauri-apps/api/core";

export type AppConfig = {
  whisperBinaryPath: string;
  modelsDir: string;
  selectedModelId: string;
  cleanupEnabled: boolean;
  ollamaUrl: string;
  ollamaModel: string;
  language: string;
};

export type WhisperModel = {
  id: string;
  name: string;
  description: string;
  fileName: string;
  downloadUrl: string;
  installed: boolean;
  recommended: boolean;
};

export type OllamaStatus = {
  available: boolean;
  message: string;
};

export const api = {
  getConfig: () => invoke<AppConfig>("get_config"),
  saveConfig: (config: AppConfig) => invoke<AppConfig>("save_config", { config }),
  listModels: () => invoke<WhisperModel[]>("list_models"),
  recommendModel: () => invoke<string>("recommend_model"),
  downloadModel: (modelId: string) =>
    invoke<WhisperModel[]>("download_model", { modelId }),
  revealModelsFolder: () => invoke<void>("reveal_models_folder"),
  checkOllama: () => invoke<OllamaStatus>("check_ollama"),
  transcribeAudioFile: (path: string) =>
    invoke<string>("transcribe_audio_file", { path }),
  startDictation: () => invoke<void>("start_dictation"),
  stopDictation: () => invoke<void>("stop_dictation"),
};
