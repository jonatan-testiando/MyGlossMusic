# Descarga el sidecar de yt-dlp con el nombre que espera `externalBin` de Tauri.
# Ejecutar desde la raiz del repo. Repetirlo actualiza a la ultima version.
$dest = "apps/desktop/src-tauri/bin/yt-dlp-x86_64-pc-windows-msvc.exe"
New-Item -ItemType Directory -Force (Split-Path $dest) | Out-Null
Invoke-WebRequest -Uri "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe" -OutFile $dest
& $dest --version
