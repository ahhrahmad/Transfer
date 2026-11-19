//! Linux reference build: same core the Android app links, backed by a plain directory.

use std::{collections::HashMap, path::PathBuf, sync::Arc};
use transfer_core::{AppState, LocalFs};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let mut dir: Option<PathBuf> = None;
    let mut assets_dir: Option<PathBuf> = None;
    let mut port: u16 = 8080;
    let mut password: Option<String> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dir" => dir = args.next().map(PathBuf::from),
            "--assets" => assets_dir = args.next().map(PathBuf::from),
            "--port" => port = args.next().ok_or("--port needs a value")?.parse()?,
            "--password" => password = args.next(),
            "-h" | "--help" => {
                println!("usage: transfer-core --dir DIR [--assets DIR] [--port 8080] [--password PW]");
                return Ok(());
            }
            other => return Err(format!("unknown argument: {other}").into()),
        }
    }

    let dir = dir.ok_or("--dir is required")?;
    let mut assets = HashMap::new();
    if let Some(assets_dir) = assets_dir {
        for entry in std::fs::read_dir(&assets_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                assets.insert(
                    entry.file_name().to_string_lossy().into_owned(),
                    std::fs::read(entry.path())?,
                );
            }
        }
    }

    let state = Arc::new(AppState {
        fs: Arc::new(LocalFs::new(&dir)?),
        hooks: Arc::new(transfer_core::fs::NoHooks),
        password,
        assets,
    });

    let listener = transfer_core::http::bind(port)?;
    println!("serving {} on port {port}", dir.display());
    transfer_core::http::serve(listener, state).await?;
    Ok(())
}
