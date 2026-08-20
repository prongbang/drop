// AirDrop-style relay: any browser on the LAN uploads a file, every browser sees it.
use percent_encoding::percent_decode_str;
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use warp::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use warp::http::{Response, StatusCode};
use warp::hyper::body::Bytes;
use warp::Filter;

// ponytail: drops live in RAM and die with the process; write them to a temp dir if you need bigger files
const MAX_FILE_BYTES: u64 = 100 * 1024 * 1024;
const MAX_FILES: usize = 20;

struct Drop {
    id: u64,
    name: String,
    mime: String,
    data: Bytes,
}

#[derive(Serialize)]
struct DropInfo {
    id: u64,
    name: String,
    size: usize,
    mime: String,
}

#[derive(Clone, Default)]
pub struct Drops {
    files: Arc<Mutex<Vec<Drop>>>,
    next_id: Arc<AtomicU64>,
}

// Browsers send the name percent-encoded; keep only a plain, header-safe file name.
fn safe_name(raw: Option<String>) -> String {
    let decoded = raw
        .map(|raw| percent_decode_str(&raw).decode_utf8_lossy().into_owned())
        .unwrap_or_default();
    let name: String = decoded
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control() && *c != '"')
        .take(120)
        .collect();
    if name.trim().is_empty() {
        "file".to_string()
    } else {
        name
    }
}

pub fn routes(
    drops: Drops,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let state = warp::any().map(move || drops.clone());

    let send = warp::path!("_api" / "send")
        .and(warp::post())
        .and(warp::header::optional::<String>("x-filename"))
        .and(warp::header::optional::<String>("content-type"))
        .and(warp::body::content_length_limit(MAX_FILE_BYTES))
        .and(warp::body::bytes())
        .and(state.clone())
        .map(|name, mime: Option<String>, data: Bytes, drops: Drops| {
            let id = drops.next_id.fetch_add(1, Ordering::Relaxed);
            let mut files = drops.files.lock().unwrap();
            files.push(Drop {
                id,
                name: safe_name(name),
                mime: mime.unwrap_or_else(|| "application/octet-stream".to_string()),
                data,
            });
            let overflow = files.len().saturating_sub(MAX_FILES);
            files.drain(..overflow);
            warp::reply::json(&id)
        });

    let list = warp::path!("_api" / "files")
        .and(warp::get())
        .and(state.clone())
        .map(|drops: Drops| {
            let files = drops.files.lock().unwrap();
            let info: Vec<DropInfo> = files
                .iter()
                .rev()
                .map(|f| DropInfo {
                    id: f.id,
                    name: f.name.clone(),
                    size: f.data.len(),
                    mime: f.mime.clone(),
                })
                .collect();
            warp::reply::json(&info)
        });

    let download = warp::path!("_api" / "files" / u64)
        .and(warp::get())
        .and(state)
        .map(|id: u64, drops: Drops| {
            let files = drops.files.lock().unwrap();
            match files.iter().find(|f| f.id == id) {
                Some(file) => Response::builder()
                    .header(CONTENT_TYPE, &file.mime)
                    .header(
                        CONTENT_DISPOSITION,
                        format!("attachment; filename=\"{}\"", file.name),
                    )
                    .body(file.data.clone())
                    .unwrap(),
                None => Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Bytes::new())
                    .unwrap(),
            }
        });

    send.or(list).or(download)
}

#[cfg(test)]
mod tests {
    use super::safe_name;

    #[test]
    fn names_are_stripped_to_something_safe() {
        assert_eq!(safe_name(Some("a%20b.txt".into())), "a b.txt");
        assert_eq!(safe_name(Some("../../etc/passwd".into())), "passwd");
        assert_eq!(safe_name(Some("bad\"\nname".into())), "badname");
        assert_eq!(safe_name(None), "file");
    }
}
