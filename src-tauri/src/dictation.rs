use std::{
    path::PathBuf,
    sync::{mpsc, Mutex},
    thread,
};

use tauri::AppHandle;

use crate::{audio::Recorder, cleanup, config::AppConfig, paste, transcription};

pub struct DictationController {
    tx: Mutex<mpsc::Sender<RecorderCommand>>,
}

impl DictationController {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || recorder_worker(rx));

        Self { tx: Mutex::new(tx) }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .lock()
            .expect("recorder channel lock poisoned")
            .send(RecorderCommand::Start(reply_tx))?;
        reply_rx.recv()?
    }

    pub async fn stop_transcribe_clean_and_paste(
        &self,
        config: &AppConfig,
        app: &AppHandle,
    ) -> anyhow::Result<()> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .lock()
            .expect("recorder channel lock poisoned")
            .send(RecorderCommand::Stop(reply_tx))?;
        let path = reply_rx.recv()??;
        let Some(path) = path else {
            return Ok(());
        };

        let result = async {
            let raw = transcription::transcribe_file(config, path.clone()).await?;
            let cleaned = cleanup::clean_or_fallback(config, &raw).await?;
            paste::paste_text(app, &cleaned).await
        }
        .await;

        let _ = std::fs::remove_file(&path);
        result
    }
}

enum RecorderCommand {
    Start(mpsc::Sender<anyhow::Result<()>>),
    Stop(mpsc::Sender<anyhow::Result<Option<PathBuf>>>),
}

fn recorder_worker(rx: mpsc::Receiver<RecorderCommand>) {
    let mut recorder = Recorder::new();
    while let Ok(command) = rx.recv() {
        match command {
            RecorderCommand::Start(reply) => {
                let _ = reply.send(recorder.start());
            }
            RecorderCommand::Stop(reply) => {
                let _ = reply.send(recorder.stop());
            }
        }
    }
}
