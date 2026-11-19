//! HTTP surface, byte-for-byte compatible with the Ktor implementation
//! that shipped at v0.6.1 (file `server/KtorServer.kt` in the upstream repo).

use crate::fs::{format_size, sanitize_name, Fs, Hooks};
use axum::{
    body::{Body, Bytes},
    extract::{ConnectInfo, Multipart, Path, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use base64::Engine;
use chrono::TimeZone;
use futures_util::{stream::poll_fn, StreamExt};
use tokio::io::AsyncWriteExt;
use serde::Serialize;
use std::{collections::HashMap, io::Write, net::SocketAddr, sync::Arc};

pub struct AppState {
    pub fs: Arc<dyn Fs>,
    pub hooks: Arc<dyn Hooks>,
    pub password: Option<String>,
    pub assets: HashMap<String, Vec<u8>>,
}

pub fn app(state: Arc<AppState>) -> Router {
    let guarded = Router::new()
        .route("/api/ping", get(ping))
        .route("/api/files", get(files))
        .route("/api/download/{name}", get(download))
        .route("/api/zip", get(zip_all).post(zip_selected))
        .route("/api/upload", axum::routing::post(upload))
        .route("/api/delete", axum::routing::post(delete_api))
        .route(
            "/{name}",
            get(download)
                .put(put_file)
                .delete(delete_raw),
        )
        .route_layer(middleware::from_fn_with_state(state.clone(), basic_auth));

    Router::new()
        .route("/", get(root))
        .route("/assets/{file}", get(asset_route))
        .merge(guarded)
        .layer(middleware::from_fn_with_state(state.clone(), ip_gate))
        .layer(
            tower_http::cors::CorsLayer::new()
                .allow_origin(tower_http::cors::AllowOrigin::mirror_request())
                .allow_credentials(true)
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::PUT,
                    axum::http::Method::DELETE,
                    axum::http::Method::OPTIONS,
                ])
                .allow_headers([
                    header::CONTENT_TYPE,
                    header::AUTHORIZATION,
                    HeaderName::from_static("x-file-name"),
                ]),
        )
        .with_state(state)
}

pub async fn serve(listener: std::net::TcpListener, state: Arc<AppState>) -> std::io::Result<()> {
    serve_with_shutdown(listener, state, std::future::pending()).await
}

pub async fn serve_with_shutdown(
    listener: std::net::TcpListener,
    state: Arc<AppState>,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    listener.set_nonblocking(true)?;
    let listener = tokio::net::TcpListener::from_std(listener)?;
    axum::serve(
        listener,
        app(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await
}

pub fn bind(port: u16) -> std::io::Result<std::net::TcpListener> {
    std::net::TcpListener::bind(("0.0.0.0", port))
}


// --- helpers ---

async fn call<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    match tokio::task::spawn_blocking(f).await {
        Ok(result) => result,
        Err(e) => Err(e.to_string()),
    }
}

fn text(status: u16, body: impl Into<String>) -> Response {
    let status = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, body.into()).into_response()
}

fn asset(st: &AppState, name: &str) -> Response {
    match st.assets.get(name) {
        Some(bytes) => {
            let mime = mime_guess::from_path(name).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.to_string())],
                bytes.clone(),
            )
                .into_response()
        }
        None => text(404, "404: Page Not Found"),
    }
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn content_disposition(name: &str) -> HeaderValue {
    let quoted = format!("attachment; filename=\"{}\"", name.replace('"', "_"));
    HeaderValue::from_str(&quoted)
        .unwrap_or_else(|_| HeaderValue::from_static("attachment"))
}

// --- middleware ---

async fn ip_gate(
    State(st): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let ip = addr.ip().to_string();
    let hooks = st.hooks.clone();
    match tokio::task::spawn_blocking(move || hooks.approve_ip(&ip)).await {
        Ok(true) => next.run(req).await,
        _ => text(403, "Access denied by host device."),
    }
}

async fn basic_auth(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    req: Request<Body>,
    next: Next,
) -> Response {
    let Some(expected) = st.password.as_deref() else {
        return next.run(req).await;
    };
    let supplied = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Basic "))
        .and_then(|b64| base64::engine::general_purpose::STANDARD.decode(b64.trim()).ok())
        .and_then(|raw| String::from_utf8(raw).ok())
        .and_then(|creds| creds.split_once(':').map(|(_, pass)| pass.to_string()));

    if supplied.as_deref() == Some(expected) {
        next.run(req).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Basic realm=\"Transfer\"")],
            "Unauthorized",
        )
            .into_response()
    }
}

// --- handlers ---

async fn ping() -> &'static str {
    "pong"
}

async fn root(State(st): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let is_curl = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ua| ua.to_lowercase().contains("curl"));
    if is_curl {
        let fs = st.fs.clone();
        return match call(move || fs.list()).await {
            Ok(list) => text(200, list.into_iter().map(|f| f.name).collect::<Vec<_>>().join("\n")),
            Err(e) => text(500, format!("Error listing files: {e}")),
        };
    }
    asset(&st, "index.html")
}

async fn asset_route(State(st): State<Arc<AppState>>, Path(file): Path<String>) -> Response {
    asset(&st, &file)
}

#[derive(Serialize)]
struct FileInfo {
    name: String,
    size: u64,
    #[serde(rename = "formattedSize")]
    formatted_size: String,
    #[serde(rename = "lastModified")]
    last_modified: String,
    #[serde(rename = "type")]
    mime: String,
    #[serde(rename = "downloadUrl")]
    download_url: String,
}

#[derive(Serialize)]
struct FileListResponse {
    files: Vec<FileInfo>,
}

async fn files(State(st): State<Arc<AppState>>) -> Response {
    let fs = st.fs.clone();
    match call(move || fs.list()).await {
        Ok(list) => {
            let files = list
                .into_iter()
                .map(|f| FileInfo {
                    download_url: format!("/api/download/{}", url_encode(&f.name)),
                    formatted_size: format_size(f.size),
                    last_modified: chrono::Local
                        .timestamp_opt(f.mtime as i64, 0)
                        .single()
                        .unwrap_or_else(|| chrono::Local.timestamp_opt(0, 0).unwrap())
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string(),
                    mime: if f.mime.is_empty() {
                        "unknown".into()
                    } else {
                        f.mime
                    },
                    name: f.name,
                    size: f.size,
                })
                .collect();
            axum::Json(FileListResponse { files }).into_response()
        }
        Err(e) => axum::Json(serde_json::json!({
            "error": format!("Error listing files: {e}")
        }))
        .into_response(),
    }
}

async fn download(State(st): State<Arc<AppState>>, Path(name): Path<String>) -> Response {
    let fs = st.fs.clone();
    let requested = name.clone();
    let opened = match call(move || fs.open_read(&requested)).await {
        Ok(o) => o,
        Err(e) => return text(404, format!("File not found: {name} ({e})")),
    };

    let file = tokio::fs::File::from_std(opened.file);
    let mut resp = Body::from_stream(tokio_util::io::ReaderStream::with_capacity(
        file,
        256 * 1024,
    ))
    .into_response();
    let headers = resp.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&opened.mime)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    if let Some(len) = opened.len {
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from(len));
    }
    headers.insert(header::CONTENT_DISPOSITION, content_disposition(&name));
    resp
}

/// Buffers the zip output and hands it to the HTTP body over a channel.
struct ChanWriter {
    tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    buf: Vec<u8>,
}

impl ChanWriter {
    fn drain(&mut self) -> std::io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        self.tx
            .blocking_send(std::mem::take(&mut self.buf))
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "client went away"))
    }
}

impl Drop for ChanWriter {
    fn drop(&mut self) {
        let _ = self.drain();
    }
}

impl Write for ChanWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.buf.extend_from_slice(bytes);
        if self.buf.len() >= 256 * 1024 {
            self.drain()?;
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.drain()
    }
}

async fn zip_all(State(st): State<Arc<AppState>>) -> Response {
    zip_response(st, None).await
}

async fn zip_selected(State(st): State<Arc<AppState>>, body: String) -> Response {
    let pairs: Vec<(String, String)> = serde_urlencoded::from_str(&body).unwrap_or_default();
    let names: Vec<String> = pairs
        .into_iter()
        .filter(|(k, _)| k == "f")
        .map(|(_, v)| v)
        .collect();
    if names.is_empty() {
        return text(422, "No files found to zip");
    }
    zip_response(st, Some(names)).await
}

async fn zip_response(st: Arc<AppState>, wanted: Option<Vec<String>>) -> Response {
    let fs = st.fs.clone();
    let fs_list = fs.clone();
    let list = match call(move || fs_list.list()).await {
        Ok(l) => l,
        Err(e) => return text(500, format!("Error listing files: {e}")),
    };
    let chosen: Vec<String> = match wanted {
        Some(names) => names
            .into_iter()
            .filter(|n| list.iter().any(|f| &f.name == n))
            .collect(),
        None => list.into_iter().map(|f| f.name).collect(),
    };
    if chosen.is_empty() {
        return text(422, "No files found to zip");
    }

    let zip_name = format!(
        "transfer_{}.zip",
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    );
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(16);
    let hooks = st.hooks.clone();

    tokio::task::spawn_blocking(move || {
        let mut zip = zip::ZipWriter::new_stream(ChanWriter {
            tx: tx.clone(),
            buf: Vec::with_capacity(256 * 1024),
        });
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(true);
        for name in &chosen {
            let opened = match fs.open_read(name) {
                Ok(o) => o,
                Err(_) => continue,
            };
            if zip.start_file(name.as_str(), options).is_err() {
                continue;
            }
            let mut reader = opened.file;
            let _ = std::io::copy(&mut reader, &mut zip);
        }
        if let Ok(mut writer) = zip.finish() {
            let _ = writer.flush();
        }
        drop(tx);
        hooks.on_change();
    });

    let stream = poll_fn(move |cx| {
        rx.poll_recv(cx)
            .map(|chunk| chunk.map(|b| Ok::<Bytes, std::io::Error>(Bytes::from(b))))
    });
    let mut resp = Body::from_stream(stream).into_response();
    let headers = resp.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/zip"),
    );
    headers.insert(header::CONTENT_DISPOSITION, content_disposition(&zip_name));
    resp
}

async fn write_body(body: Body, file: std::fs::File) -> Result<u64, String> {
    let mut out = tokio::fs::File::from_std(file);
    let mut stream = body.into_data_stream();
    let mut total = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        out.write_all(&chunk).await.map_err(|e| e.to_string())?;
        total += chunk.len() as u64;
    }
    out.flush().await.map_err(|e| e.to_string())?;
    Ok(total)
}

async fn upload(State(st): State<Arc<AppState>>, mut multipart: Multipart) -> Response {
    let mut uploaded: Vec<String> = Vec::new();
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => return text(400, format!("Upload error: {e}")),
        };
        let Some(raw_name) = field.file_name().map(|s| s.to_string()) else {
            continue;
        };
        let fs = st.fs.clone();
        let hooks = st.hooks.clone();
        let desired = sanitize_name(&raw_name);
        let created = match call(move || fs.create(&desired)).await {
            Ok(c) => c,
            Err(e) => return text(500, format!("Upload error: {e}")),
        };
        let mut field = field;
        let mut out = tokio::fs::File::from_std(created.file);
        let mut failed = None;
        while let Ok(Some(chunk)) = field.chunk().await {
            if let Err(e) = out.write_all(&chunk).await {
                failed = Some(e.to_string());
                break;
            }
        }
        if let Some(e) = failed {
            return text(500, format!("Upload error: {e}"));
        }
        let _ = out.flush().await;
        hooks.on_change();
        uploaded.push(created.name);
    }
    if uploaded.is_empty() {
        return text(400, "No files were uploaded or upload failed.");
    }
    text(200, format!("Successfully uploaded: {}", uploaded.join(", ")))
}

async fn put_file(
    State(st): State<Arc<AppState>>,
    Path(name): Path<String>,
    body: Body,
) -> Response {
    let doomed = name.clone();
    let fs = st.fs.clone();
    let _ = call(move || fs.delete(&doomed)).await;

    let fs = st.fs.clone();
    let hooks = st.hooks.clone();
    let desired = sanitize_name(&name);
    let created = match call(move || fs.create(&desired)).await {
        Ok(c) => c,
        Err(e) => return text(500, format!("Error during PUT upload: {e}")),
    };
    match write_body(body, created.file).await {
        Ok(_) => {
            hooks.on_change();
            text(201, format!("File '{}' uploaded via PUT.", created.name))
        }
        Err(e) => text(500, format!("Error during PUT upload: {e}")),
    }
}

async fn delete_file(st: &AppState, name: &str) -> Result<(), String> {
    let fs = st.fs.clone();
    let target = name.to_string();
    call(move || fs.delete(&target)).await?;
    st.hooks.on_change();
    Ok(())
}

async fn delete_api(State(st): State<Arc<AppState>>, body: String) -> Response {
    let name = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("filename").and_then(|f| f.as_str()).map(String::from))
        .unwrap_or_default();
    if name.is_empty() {
        return axum::Json(serde_json::json!({ "error": "Filename not provided." })).into_response();
    }
    match delete_file(&st, &name).await {
        Ok(()) => axum::Json(serde_json::json!({
            "message": format!("File '{name}' deleted.")
        }))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

async fn delete_raw(State(st): State<Arc<AppState>>, Path(name): Path<String>) -> Response {
    match delete_file(&st, &name).await {
        Ok(()) => text(200, format!("File '{name}' deleted.")),
        Err(e) => text(500, format!("Error: {e}")),
    }
}
