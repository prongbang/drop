mod browse;
mod drop;
mod logger;

use clap::Parser;
use qrcode::render::unicode;
use qrcode::QrCode;
use rust_embed::RustEmbed;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::process::Command as ProcessCommand;
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
    #[command(subcommand)]
    command: Option<SubCommand>,
    #[arg(short, long, default_value_t = 8000)]
    port: u16,
    #[arg(short, long, default_value_t = false)]
    silent: bool,
}

#[derive(clap::Subcommand, Debug)]
enum SubCommand {
    /// Update Drop to the latest published version
    Update,
}

fn check_for_update() {
    let current = env!("CARGO_PKG_VERSION");
    if let Some(latest) = latest_release().filter(|latest| is_newer(latest, current)) {
        println!("\nA new Drop version is available: v{latest} (current: v{current}). Run `drop update` to install it.\n");
    }
}

fn latest_release() -> Option<String> {
    let output = ProcessCommand::new("curl")
        .args(["-fsSL", "--max-time", "3", "https://api.github.com/repos/prongbang/drop/releases/latest"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let release: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    release["tag_name"]
        .as_str()
        .map(|tag| tag.trim_start_matches('v').to_owned())
}

fn is_newer(candidate: &str, current: &str) -> bool {
    let parse = |version: &str| {
        version
            .split('.')
            .map(str::parse::<u64>)
            .collect::<Result<Vec<_>, _>>()
    };
    match (parse(candidate), parse(current)) {
        (Ok(candidate), Ok(current)) => candidate > current,
        _ => false,
    }
}

fn update() -> Result<(), String> {
    let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
    let platform = match (os, arch) {
        ("macos", "aarch64") => "darwin-arm64",
        ("macos", "x86_64") => "darwin-x86_64",
        _ => return Err(format!("Prebuilt Drop binary is not available for {os}/{arch}")),
    };

    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let parent = executable.parent().ok_or("Cannot locate the installed Drop directory")?;
    let filename = executable.file_name().ok_or("Cannot locate the installed Drop filename")?;
    let destination = parent.join(filename);
    let temporary = parent.join(format!(".drop-update-{}", std::process::id()));
    let version = latest_release().ok_or("Could not find the latest GitHub release. Try again later.")?;
    let url = format!("https://raw.githubusercontent.com/prongbang/drop/{version}/bin/drop-{platform}");
    let status = ProcessCommand::new("curl")
        .args(["-fsSL", &url, "-o"])
        .arg(&temporary)
        .status()
        .map_err(|error| format!("Could not run curl: {error}"))?;
    if !status.success() {
        let _ = std::fs::remove_file(&temporary);
        return Err("Download failed. Check your network and try again.".into());
    }
    let install = ProcessCommand::new("install")
        .arg("-m")
        .arg("755")
        .arg(&temporary)
        .arg(&destination)
        .status()
        .map_err(|error| format!("Could not install the update: {error}"))?;
    let _ = std::fs::remove_file(&temporary);
    if !install.success() {
        return Err(format!("Could not replace {}. Check write permissions.", destination.display()));
    }
    println!("Updated Drop successfully. Run `drop --version` to see the installed version.");
    Ok(())
}

#[tokio::main]
async fn main() {
    // Parse command line arguments
    let args = Args::parse();
    if let Some(SubCommand::Update) = args.command {
        if let Err(error) = update() {
            eprintln!("Update failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    let version = env!("CARGO_PKG_VERSION");

    // Get the current directory
    let current_dir = std::env::current_dir().expect("Failed to get current directory");

    // Convert the current directory to a string
    let current_path = String::from(current_dir.to_string_lossy().as_ref());

    // Create a Warp filter to handle requests for static assets
    let static_dir = warp::fs::dir(current_path);

    // Peer-to-peer file transfers, plus a listing of this directory
    let api = drop::routes(drop::Hub::default()).or(browse::routes(current_dir.clone()));

    // CORS
    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["HEAD", "CONNECT", "TRACE", "GET", "POST", "PUT", "PATCH", "OPTIONS", "DELETE"]);

    // Create a Warp filter for serving the main HTML file
    let index_html = warp::any().and(warp::fs::file("index.html"));

    // Fall back to the embedded web app when the directory has nothing to serve
    let embedded = warp::path::tail().map(|tail: Tail| serve_embedded(tail.as_str()));

    // ...but keep the drop UI and its assets reachable even when the directory has its own index.html
    let drop_ui = warp::path("drop")
        .and(warp::path::tail())
        .map(|_| serve_embedded("index.html"));
    let drop_assets = warp::path("app")
        .and(warp::path::tail())
        .map(|tail: Tail| serve_embedded(&format!("app/{}", tail.as_str())));

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
    tokio::task::spawn_blocking(check_for_update);

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
