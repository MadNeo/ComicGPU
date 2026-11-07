#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // Bind synchronously so the UI always receives the actual free port.
            let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
            let address = listener.local_addr()?;
            listener.set_nonblocking(true)?;
            let token = comicgpu_server::new_token();
            let url = format!("http://{address}/?token={token}");
            tauri::async_runtime::spawn(async move {
                let listener = tokio_listener(listener);
                if let Err(error) = comicgpu_server::serve(listener, token).await {
                    eprintln!("{error}");
                }
            });
            tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::External(url.parse()?),
            )
            .title("ComicGPU")
            .inner_size(1360.0, 900.0)
            .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("ComicGPU kon niet starten");
}
fn tokio_listener(listener: std::net::TcpListener) -> comicgpu_server::TcpListener {
    comicgpu_server::TcpListener::from_std(listener).expect("Listener initialiseren")
}
