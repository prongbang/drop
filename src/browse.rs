// Browse the directory the server was started in, from the web app.
use serde::{Deserialize, Serialize};
use bytes::Buf;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::AsyncWriteExt;
use tokio::fs::OpenOptions;
use futures_util::{Stream, StreamExt};
use warp::http::StatusCode;
use warp::{Filter, Reply};

#[derive(Deserialize)]
struct Query {
    path: Option<String>,
}

#[derive(Serialize)]
struct Entry {
    name: String,
    size: u64,
    dir: bool,
}

#[derive(Serialize)]
struct Listing {
    path: String,
    entries: Vec<Entry>,
}

static UPLOAD_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Deserialize)]
struct UploadQuery {
    path: Option<String>,
    name: String,
}

// Keep the request inside the served directory, symlinks included.
fn resolve(root: &Path, rel: &str) -> Option<PathBuf> {
    let target = root.join(rel.trim_start_matches('/')).canonicalize().ok()?;
    target.starts_with(root.canonicalize().ok()?).then_some(target)
}

async fn read_dir(root: PathBuf, rel: String) -> Option<Vec<Entry>> {
    let mut dir = tokio::fs::read_dir(resolve(&root, &rel)?).await.ok()?;
    let mut entries = Vec::new();
    while let Ok(Some(item)) = dir.next_entry().await {
        let meta = match item.metadata().await {
            Ok(meta) => meta,
            Err(_) => continue, // broken symlink or vanished mid-listing
        };
        entries.push(Entry {
            name: item.file_name().to_string_lossy().into_owned(),
            size: meta.len(),
            dir: meta.is_dir(),
        });
    }
    entries.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Some(entries)
}

pub fn routes(
    root: PathBuf,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let listing_root = root.clone();
    let listing = warp::path!("api" / "dir")
        .and(warp::get())
        .and(warp::query::<Query>())
        .then(move |query: Query| {
            let root = listing_root.clone();
            async move {
                let rel = query.path.unwrap_or_default();
                match read_dir(root, rel.clone()).await {
                    Some(entries) => warp::reply::with_status(
                        warp::reply::json(&Listing { path: rel, entries }),
                        StatusCode::OK,
                    ),
                    None => warp::reply::with_status(
                        warp::reply::json(&Listing { path: rel, entries: Vec::new() }),
                        StatusCode::NOT_FOUND,
                    ),
                }
            }
        });

    let upload = warp::path!("api" / "files")
        .and(warp::post())
        .and(warp::query::<UploadQuery>())
        .and(warp::body::stream())
        .then(move |query, body| upload_file(root.clone(), query, body));

    listing.or(upload)
}

async fn upload_file(
    root: PathBuf,
    query: UploadQuery,
    mut body: impl Stream<Item = Result<impl Buf, warp::Error>> + Unpin + Send + Sync,
) -> warp::reply::Response {
    let rel = query.path.unwrap_or_default();
    let Some(dir) = resolve(&root, &rel) else {
        return warp::reply::with_status("Invalid destination", StatusCode::BAD_REQUEST).into_response();
    };
    // Store only a plain file name inside the selected directory.
    let name = Path::new(&query.name).file_name().filter(|name| !name.is_empty());
    let Some(name) = name else {
        return warp::reply::with_status("Invalid file name", StatusCode::BAD_REQUEST).into_response();
    };
    let destination = dir.join(name);
    let (temp_path, mut file) = loop {
        let id = UPLOAD_ID.fetch_add(1, Ordering::Relaxed);
        let temp_path = dir.join(format!(".server-upload-{}-{id}.tmp", std::process::id()));
        match OpenOptions::new().write(true).create_new(true).open(&temp_path).await {
            Ok(file) => break (temp_path, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return warp::reply::with_status("Could not write file", StatusCode::INTERNAL_SERVER_ERROR).into_response(),
        }
    };
    while let Some(chunk) = body.next().await {
        let Ok(mut chunk) = chunk else {
            drop(file);
            let _ = tokio::fs::remove_file(temp_path).await;
            return warp::reply::with_status("Could not read upload", StatusCode::BAD_REQUEST).into_response();
        };
        while chunk.remaining() > 0 {
            let part = chunk.chunk();
            if file.write_all(part).await.is_err() {
                drop(file);
                let _ = tokio::fs::remove_file(temp_path).await;
                return warp::reply::with_status("Could not write file", StatusCode::INTERNAL_SERVER_ERROR).into_response();
            }
            let len = part.len();
            chunk.advance(len);
        }
    }
    if file.flush().await.is_err() {
        drop(file);
        let _ = tokio::fs::remove_file(temp_path).await;
        return warp::reply::with_status("Could not write file", StatusCode::INTERNAL_SERVER_ERROR).into_response();
    }
    drop(file);
    match tokio::fs::rename(&temp_path, destination).await {
        Ok(()) => warp::reply::with_status("Uploaded", StatusCode::CREATED).into_response(),
        Err(_) => {
            let _ = tokio::fs::remove_file(temp_path).await;
            warp::reply::with_status("Could not write file", StatusCode::INTERNAL_SERVER_ERROR).into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::resolve;
    use std::path::Path;

    #[test]
    fn escapes_are_refused() {
        let root = Path::new("src");
        assert!(resolve(root, "main.rs").is_some());
        assert!(resolve(root, "").is_some());
        assert!(resolve(root, "../Cargo.toml").is_none());
        assert!(resolve(root, "/etc/passwd").is_none());
        assert!(resolve(root, "nope.rs").is_none());
    }
}
