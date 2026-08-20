mod browse;
mod drop;
mod logger;

use clap::Parser;
use qrcode::render::unicode;
use qrcode::QrCode;
use rust_embed::RustEmbed;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use warp::http::header::CONTENT_TYPE;
use warp::http::{Response, StatusCode};
use warp::path::Tail;
use warp::Filter;

// The SvelteKit app in web/build, baked into the binary at compile time.
#[derive(RustEmbed)]
#[folder = "web/build"]
struct Web;

// Ask the routing table which interface reaches the internet; UDP connect sends nothing.
fn local_ip() -> IpAddr {
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|socket| {
            socket.connect("8.8.8.8:80")?;
            socket.local_addr()
        })
        .map(|addr| addr.ip())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
}

fn qr_code(url: &str) -> String {
    match QrCode::new(url) {
        Ok(code) => code.render::<unicode::Dense1x2>().quiet_zone(true).build(),
        Err(_) => String::new(),
    }
}

fn serve_embedded(path: &str) -> Response<Vec<u8>> {
    let path = if path.is_empty() { "index.html" } else { path };
    match Web::get(path).or_else(|| Web::get("index.html")) {
        Some(file) => Response::builder()
            .header(CONTENT_TYPE, file.metadata.mimetype())
            .body(file.data.into_owned())
            .unwrap(),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Vec::new())
            .unwrap(),
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = 8000)]
    port: u16,
    #[arg(short, long, default_value_t = false)]
    silent: bool,
}

#[tokio::main]
async fn main() {
    let version = "0.2.3";

    // Parse command line arguments
    let args = Args::parse();

    // Get the current directory
    let current_dir = std::env::current_dir().expect("Failed to get current directory");

    // Convert the current directory to a string
    let current_path = String::from(current_dir.to_string_lossy().as_ref());

    // Create a Warp filter to handle requests for static assets
    let static_dir = warp::fs::dir(current_path);

    // File-drop relay shared by every browser on the network, plus a listing of this directory
    let api = drop::routes(drop::Drops::default()).or(browse::routes(current_dir.clone()));

    // CORS
    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["HEAD", "CONNECT", "TRACE", "GET", "POST", "PUT", "PATCH", "OPTIONS", "DELETE"]);

    // Create a Warp filter for serving the main HTML file
    let index_html = warp::any().and(warp::fs::file("index.html"));

    // Fall back to the embedded web app when the directory has nothing to serve
    let embedded = warp::path::tail().map(|tail: Tail| serve_embedded(tail.as_str()));

    // ...but keep the drop UI and its assets reachable even when the directory has its own index.html
    let drop_ui = warp::path("_drop")
        .and(warp::path::tail())
        .map(|_| serve_embedded("index.html"));
    let drop_assets = warp::path("_app")
        .and(warp::path::tail())
        .map(|tail: Tail| serve_embedded(&format!("_app/{}", tail.as_str())));

    // Create the address tuple
    let ip_address = [0, 0, 0, 0];
    let addr = (ip_address, args.port);
    let ip = ip_address.iter().map(|&octet| octet.to_string()).collect::<Vec<_>>().join(".");

    // The address other devices on the network can actually reach
    let url = format!("http://{}:{}", local_ip(), args.port);

    // Print the listening address
    println!("{}", format!(r#"
  ___ ___ _____  _____ ____
 (_-</ -_) __/ |/ / -_) __/
/___/\__/_/  |___/\__/_/ (v{})

Listen on http://{}:{}
Network   {}

{}"#, version, ip, addr.1, url, qr_code(&url)));

    // Start the Warp server with only the static assets filter
    if args.silent {
        let routes = api.or(drop_ui).or(drop_assets).or(static_dir).or(index_html).or(embedded).with(cors);
        warp::serve(routes).run(addr).await;
    } else {
        logger::setup_logging();
        let routes = api.or(drop_ui).or(drop_assets).or(static_dir).or(index_html).or(embedded).with(cors).with(logger::log());
        warp::serve(routes).run(addr).await;
    };
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_and_ip() {
        assert!(!local_ip().is_unspecified());
        assert!(qr_code("http://192.168.0.10:8000").contains('█'));
        // 3KB overflows every QR version, so it must degrade instead of panicking
        assert_eq!(qr_code(&"x".repeat(3000)), "");
    }
}
