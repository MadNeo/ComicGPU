use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use comicgpu_core::{Book, Options};
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
pub use tokio::net::TcpListener;
use tokio::sync::Semaphore;

pub fn new_token() -> String {
    uuid::Uuid::new_v4().to_string()
}
#[derive(Clone)]
struct AppState {
    books: Arc<Mutex<HashMap<String, Arc<Book>>>>,
    gate: Arc<Semaphore>,
    token: String,
}
#[derive(Deserialize)]
struct Auth {
    token: Option<String>,
}
async fn authenticate(
    State(state): State<AppState>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    if request.uri().path().starts_with("/api/") {
        let header_token = request
            .headers()
            .get("x-comicgpu-token")
            .and_then(|h| h.to_str().ok());
        let query_token = Query::<Auth>::try_from_uri(request.uri())
            .ok()
            .and_then(|q| q.0.token);
        if header_token != Some(state.token.as_str())
            && query_token.as_deref() != Some(state.token.as_str())
        {
            return (
                StatusCode::UNAUTHORIZED,
                "Toegangscode ontbreekt. Open de volledige start-URL.",
            )
                .into_response();
        }
    }
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::REFERRER_POLICY, "no-referrer".parse().unwrap());
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
}
type ApiResult<T> = Result<T, (StatusCode, String)>;
fn error(error: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, error.to_string())
}
fn book(state: &AppState, id: &str) -> ApiResult<Arc<Book>> {
    state.books.lock().map_err(error)?.get(id).cloned().ok_or((
        StatusCode::NOT_FOUND,
        "Comic niet gevonden. Upload opnieuw.".into(),
    ))
}
async fn upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> ApiResult<Json<serde_json::Value>> {
    let permit = state.gate.clone().acquire_owned().await.map_err(error)?;
    let field = multipart
        .next_field()
        .await
        .map_err(error)?
        .ok_or_else(|| error("Bestand ontbreekt"))?;
    let name = field.file_name().unwrap_or("comic.cbz").to_string();
    let data = field.bytes().await.map_err(error)?;
    let parsed = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        comicgpu_core::read_book(&data, &name)
    })
    .await
    .map_err(error)?
    .map_err(error)?;
    let id = new_token();
    let response = serde_json::json!({"id":id,"pages":parsed.pages.iter().map(|p|&p.info).collect::<Vec<_>>(),"warnings":parsed.warnings});
    let mut books = state.books.lock().map_err(error)?;
    let used: usize = books
        .values()
        .flat_map(|b| &b.pages)
        .map(|p| p.data.len())
        .sum();
    let added: usize = parsed.pages.iter().map(|p| p.data.len()).sum();
    if used + added > 1024 * 1024 * 1024 {
        return Err(error(
            "Werkruimte vol (1 GB). Verwijder eerst verwerkte comics.",
        ));
    }
    books.insert(id, Arc::new(parsed));
    Ok(Json(response))
}
async fn remove(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult<StatusCode> {
    state.books.lock().map_err(error)?.remove(&id);
    Ok(StatusCode::NO_CONTENT)
}
async fn original(
    State(state): State<AppState>,
    Path((id, index)): Path<(String, usize)>,
) -> ApiResult<Response> {
    let book = book(&state, &id)?;
    let data = book
        .pages
        .get(index)
        .ok_or_else(|| error("Pagina ontbreekt"))?
        .data
        .clone();
    let permit = state.gate.clone().acquire_owned().await.map_err(error)?;
    let bytes = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        comicgpu_core::png(comicgpu_core::decode(&data)?)
    })
    .await
    .map_err(error)?
    .map_err(error)?;
    Ok(([(header::CONTENT_TYPE, "image/png")], bytes).into_response())
}
async fn preview(
    State(state): State<AppState>,
    Path((id, index)): Path<(String, usize)>,
    Json(options): Json<Options>,
) -> ApiResult<Response> {
    let book = book(&state, &id)?;
    let data = book
        .pages
        .get(index)
        .ok_or_else(|| error("Pagina ontbreekt"))?
        .data
        .clone();
    let permit = state.gate.clone().acquire_owned().await.map_err(error)?;
    let bytes = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        comicgpu_core::png(comicgpu_core::restore(
            comicgpu_core::decode(&data)?,
            &options,
        )?)
    })
    .await
    .map_err(error)?
    .map_err(error)?;
    Ok(([(header::CONTENT_TYPE, "image/png")], bytes).into_response())
}
#[derive(Deserialize)]
struct Export {
    options: Vec<Options>,
    workers: usize,
    allow_unreviewed: bool,
}
async fn export(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<Export>,
) -> ApiResult<Response> {
    let book = book(&state, &id)?;
    let permit = state.gate.clone().acquire_owned().await.map_err(error)?;
    let bytes = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        comicgpu_core::export_book(
            &book,
            &request.options,
            request.workers,
            request.allow_unreviewed,
        )
    })
    .await
    .map_err(error)?
    .map_err(error)?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/vnd.comicbook+zip"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=verbeterd.cbz",
            ),
        ],
        Bytes::from(bytes),
    )
        .into_response())
}
async fn capabilities() -> Json<serde_json::Value> {
    let gpu = tokio::task::spawn_blocking(comicgpu_core::gpu::description)
        .await
        .ok()
        .flatten();
    Json(
        serde_json::json!({"gpu":gpu,"cpus":std::thread::available_parallelism().map(|n|n.get()).unwrap_or(1)}),
    )
}
pub async fn serve(listener: TcpListener, token: String) -> anyhow::Result<()> {
    let state = AppState {
        books: Arc::new(Mutex::new(HashMap::new())),
        gate: Arc::new(Semaphore::new(1)),
        token,
    };
    let app = Router::new()
        .route(
            "/",
            get(|| async { Html(include_str!("../../../ui/index.html")) }),
        )
        .route(
            "/app.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../../../ui/app.js"),
                )
            }),
        )
        .route(
            "/style.css",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/css")],
                    include_str!("../../../ui/style.css"),
                )
            }),
        )
        .route("/api/capabilities", get(capabilities))
        .route("/api/books", post(upload))
        .route("/api/books/{id}", delete(remove))
        .route("/api/books/{id}/pages/{index}", get(original).post(preview))
        .route("/api/books/{id}/export", post(export))
        .layer(DefaultBodyLimit::max(513 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(), authenticate))
        .with_state(state);
    axum::serve(listener, app).await?;
    Ok(())
}
