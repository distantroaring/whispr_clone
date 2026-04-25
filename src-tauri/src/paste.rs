use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tokio::process::Command;

pub fn write_clipboard(app: &AppHandle, text: &str) -> anyhow::Result<()> {
    app.clipboard().write_text(text.to_string())?;
    Ok(())
}

pub async fn paste_text(app: &AppHandle, text: &str) -> anyhow::Result<()> {
    write_clipboard(app, text)?;

    #[cfg(target_os = "macos")]
    {
        Command::new("osascript")
            .arg("-e")
            .arg("tell application \"System Events\" to keystroke \"v\" using command down")
            .status()
            .await?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg("Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait('^v')")
            .status()
            .await?;
    }

    Ok(())
}
