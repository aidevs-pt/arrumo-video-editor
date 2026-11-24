/// Whether the FFmpeg and ffprobe that Arrumo renders with start, checked once per launch.
#[tauri::command]
pub async fn media_tools_status() -> arrumo_workspaces::MediaToolsStatus {
    tauri::async_runtime::spawn_blocking(arrumo_workspaces::media_tools_status)
        .await
        .unwrap_or(arrumo_workspaces::MediaToolsStatus {
            ffmpeg: false,
            ffprobe: false,
        })
}
