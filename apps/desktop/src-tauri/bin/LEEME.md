# yt-dlp empaquetado

Tauri copia estos binarios dentro del instalador (`externalBin` en
`tauri.conf.json`) y los deja junto al ejecutable, ya sin el sufijo. Ahí es
donde `YtDlp::detect()` los encuentra.

**El nombre tiene que llevar el triple del objetivo.** Es cosa de Tauri, no
nuestra: sin el sufijo no lo reconoce y el empaquetado falla.

    yt-dlp-x86_64-pc-windows-msvc.exe     Windows
    yt-dlp-x86_64-apple-darwin            Mac Intel
    yt-dlp-aarch64-apple-darwin           Mac Apple Silicon
    yt-dlp-x86_64-unknown-linux-gnu       Linux

Solo está el de Windows en el repositorio; los demás los descarga el workflow
de release al compilar, para no meter 70 MB de binarios en git.

Sin yt-dlp la aplicación arranca igual, pero cae al acuñador y cada pista se
corta a ~48 segundos. No es opcional para una versión que se reparta.
