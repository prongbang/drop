// Browse the directory the server was started in, from the web app.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use warp::http::StatusCode;
use warp::Filter;

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
    warp::path!("_api" / "dir")
        .and(warp::get())
        .and(warp::query::<Query>())
        .then(move |query: Query| {
            let root = root.clone();
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
        })
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
