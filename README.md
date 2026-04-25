# Whispr Clone

A free, open-source, local-first dictation utility for macOS Apple Silicon and Windows.

## Scope

- Tauri + React + TypeScript desktop app.
- Menu bar/system tray utility with a modern minimal settings window.
- Hold-to-talk dictation:
  - macOS: `Option + Space`
  - Windows: `Alt + Space`
- Local transcription through `whisper.cpp`.
- Local AI cleanup through Ollama at `http://localhost:11434`.
- Immediate paste into the previously focused app.
- Audio file transcription from settings: choose a file, transcribe, copy.
- No transcript history. Temporary files are deleted after transcription.
- Apache-2.0 licensed.

## Requirements

- Node.js with npm, pnpm, yarn, or another JS package manager.
- Rust stable toolchain.
- Platform requirements from Tauri.
- Ollama installed separately for cleanup.
- `whisper.cpp` binary and model files configured in the app settings.

## Development

```bash
npm install
npm run tauri:dev
```

Install `whisper.cpp` separately and point **Whisper binary** at `whisper-cli`.
The app downloads model files into its app data folder from the settings window.

## Build

```bash
npm run tauri:build
```

The Tauri bundler is configured for macOS `.dmg` and Windows `.msi`/NSIS installers.

## Current Implementation Notes

- Hold-to-talk registration is wired through Tauri global shortcut.
- Microphone audio is captured to a temporary WAV file and deleted after paste.
- `whisper.cpp` execution expects a local `whisper-cli` compatible binary.
- Ollama cleanup falls back to raw transcript if Ollama is unavailable.
- Audio-file transcription copies the cleaned result to the clipboard.
