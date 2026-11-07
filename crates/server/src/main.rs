#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let address = std::env::var("COMICGPU_BIND").unwrap_or_else(|_| "127.0.0.1:8501".into());
    let token = std::env::var("COMICGPU_TOKEN").unwrap_or_else(|_| comicgpu_server::new_token());
    anyhow::ensure!(
        token.len() >= 16,
        "COMICGPU_TOKEN moet minimaal 16 tekens bevatten"
    );
    let listener = comicgpu_server::TcpListener::bind(&address).await?;
    println!(
        "ComicGPU start-URL: http://{}/?token={token}",
        listener.local_addr()?
    );
    comicgpu_server::serve(listener, token).await
}
