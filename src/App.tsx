import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Check,
  Clipboard,
  Download,
  FolderOpen,
  Mic,
  RefreshCw,
  Settings,
  Sparkles,
  Upload,
  Wand2,
} from "lucide-react";
import { api, AppConfig, OllamaStatus, WhisperModel } from "./lib/api";

const defaultConfig: AppConfig = {
  whisperBinaryPath: "whisper-cli",
  modelsDir: "",
  selectedModelId: "base",
  cleanupEnabled: true,
  ollamaUrl: "http://localhost:11434",
  ollamaModel: "llama3.2:3b",
  language: "auto",
};

function App() {
  const [config, setConfig] = useState<AppConfig>(defaultConfig);
  const [models, setModels] = useState<WhisperModel[]>([]);
  const [ollama, setOllama] = useState<OllamaStatus | null>(null);
  const [busy, setBusy] = useState("");
  const [message, setMessage] = useState("Ready");
  const [dictating, setDictating] = useState(false);
  const [fileTranscript, setFileTranscript] = useState("");

  const selectedModel = useMemo(
    () => models.find((model) => model.id === config.selectedModelId),
    [models, config.selectedModelId],
  );

  useEffect(() => {
    void refresh();
  }, []);

  async function refresh() {
    setBusy("refresh");
    try {
      const [nextConfig, nextModels, nextOllama] = await Promise.all([
        api.getConfig(),
        api.listModels(),
        api.checkOllama(),
      ]);
      setConfig(nextConfig);
      setModels(nextModels);
      setOllama(nextOllama);
      setMessage("Ready");
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy("");
    }
  }

  async function save(nextConfig: AppConfig) {
    setConfig(nextConfig);
    try {
      const saved = await api.saveConfig(nextConfig);
      setConfig(saved);
      setMessage("Settings saved");
    } catch (error) {
      setMessage(String(error));
    }
  }

  async function downloadModel(modelId: string) {
    setBusy(`download:${modelId}`);
    setMessage("Downloading model");
    try {
      const nextModels = await api.downloadModel(modelId);
      setModels(nextModels);
      setMessage("Model installed");
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy("");
    }
  }

  async function selectFile() {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: "Audio",
          extensions: ["wav", "mp3", "m4a", "flac", "ogg", "aac"],
        },
      ],
    });
    if (typeof selected !== "string") return;

    setBusy("file");
    setFileTranscript("");
    setMessage("Transcribing file");
    try {
      const transcript = await api.transcribeAudioFile(selected);
      setFileTranscript(transcript);
      setMessage("Copied to clipboard");
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy("");
    }
  }

  async function toggleDictation() {
    try {
      if (dictating) {
        setBusy("dictation");
        setMessage("Transcribing");
        await api.stopDictation();
        setDictating(false);
        setMessage("Pasted");
      } else {
        await api.startDictation();
        setDictating(true);
        setMessage("Listening");
      }
    } catch (error) {
      setDictating(false);
      setMessage(String(error));
    } finally {
      setBusy("");
    }
  }

  return (
    <main className="app-shell">
      <header className="topbar">
        <div className="brand">
          <div className="brand-mark">
            <Mic size={20} />
          </div>
          <div>
            <h1>Whispr Clone</h1>
            <p>Local dictation</p>
          </div>
        </div>

        <button className="icon-button" onClick={refresh} disabled={busy === "refresh"} title="Refresh">
          <RefreshCw size={18} />
        </button>
      </header>

      <section className="status-band">
        <StatusPill
          active={Boolean(selectedModel?.installed)}
          icon={<Download size={15} />}
          label={selectedModel?.installed ? `${selectedModel.name} installed` : "Model missing"}
        />
        <StatusPill
          active={Boolean(ollama?.available)}
          icon={<Sparkles size={15} />}
          label={ollama?.message ?? "Ollama unchecked"}
        />
        <StatusPill
          active={config.cleanupEnabled}
          icon={<Wand2 size={15} />}
          label={config.cleanupEnabled ? "Cleanup on" : "Cleanup off"}
        />
      </section>

      <section className="hero-panel">
        <div>
          <span className="eyebrow">Option + Space / Alt + Space</span>
          <h2>{dictating ? "Listening" : "Hold-to-talk is ready"}</h2>
          <p>{message}</p>
        </div>
        <button className={`record-button ${dictating ? "recording" : ""}`} onClick={toggleDictation}>
          <Mic size={20} />
          {dictating ? "Stop" : "Test"}
        </button>
      </section>

      <div className="content-grid">
        <section className="panel">
          <PanelTitle icon={<Settings size={18} />} title="Settings" />

          <label className="field">
            <span>Whisper binary</span>
            <input
              value={config.whisperBinaryPath}
              onChange={(event) =>
                save({ ...config, whisperBinaryPath: event.target.value })
              }
            />
          </label>

          <label className="field">
            <span>Language</span>
            <select
              value={config.language}
              onChange={(event) => save({ ...config, language: event.target.value })}
            >
              <option value="auto">Auto</option>
              <option value="en">English</option>
              <option value="bn">Bangla (বাংলা)</option>
            </select>
          </label>

          <label className="switch-row">
            <span>
              <strong>AI cleanup</strong>
              <small>Ollama before paste</small>
            </span>
            <input
              type="checkbox"
              checked={config.cleanupEnabled}
              onChange={(event) =>
                save({ ...config, cleanupEnabled: event.target.checked })
              }
            />
          </label>

          <div className="two-fields">
            <label className="field">
              <span>Ollama URL</span>
              <input
                value={config.ollamaUrl}
                onChange={(event) => save({ ...config, ollamaUrl: event.target.value })}
              />
            </label>
            <label className="field">
              <span>Model</span>
              <input
                value={config.ollamaModel}
                onChange={(event) => save({ ...config, ollamaModel: event.target.value })}
              />
            </label>
          </div>
        </section>

        <section className="panel">
          <PanelTitle icon={<Download size={18} />} title="Whisper Models" />
          <div className="models-list">
            {models.map((model) => (
              <div className="model-row" key={model.id}>
                <div>
                  <strong>
                    {model.name}
                    {model.recommended && <em>Recommended</em>}
                  </strong>
                  <small>{model.description}</small>
                </div>
                <div className="model-actions">
                  <button
                    className={config.selectedModelId === model.id ? "selected-button" : "ghost-button"}
                    onClick={() => save({ ...config, selectedModelId: model.id })}
                  >
                    <Check size={16} />
                  </button>
                  <button
                    className="ghost-button"
                    disabled={model.installed || busy === `download:${model.id}`}
                    onClick={() => downloadModel(model.id)}
                  >
                    {model.installed ? <Check size={16} /> : <Download size={16} />}
                  </button>
                </div>
              </div>
            ))}
          </div>
          <button className="secondary-button" onClick={() => api.revealModelsFolder()}>
            <FolderOpen size={16} />
            Models folder
          </button>
        </section>

        <section className="panel span-two">
          <PanelTitle icon={<Upload size={18} />} title="Audio File" />
          <div className="file-tool">
            <button className="primary-button" onClick={selectFile} disabled={busy === "file"}>
              <Upload size={17} />
              Choose file
            </button>
            <div className="transcript-preview">
              {fileTranscript || "Copied text appears here after transcription."}
            </div>
            <div className="clipboard-note">
              <Clipboard size={15} />
              Clipboard output
            </div>
          </div>
        </section>
      </div>
    </main>
  );
}

function PanelTitle({ icon, title }: { icon: React.ReactNode; title: string }) {
  return (
    <div className="panel-title">
      {icon}
      <h3>{title}</h3>
    </div>
  );
}

function StatusPill({
  active,
  icon,
  label,
}: {
  active: boolean;
  icon: React.ReactNode;
  label: string;
}) {
  return (
    <div className={`status-pill ${active ? "active" : ""}`}>
      {icon}
      <span>{label}</span>
    </div>
  );
}

export default App;
