use std::process::Command;

// Build the SvelteKit app so web/build is fresh before rust-embed reads it.
fn main() {
    for path in ["web/src", "web/static", "web/package.json", "web/vite.config.ts"] {
        println!("cargo:rerun-if-changed={}", path);
    }

    for args in [["install"].as_slice(), ["run", "build"].as_slice()] {
        match Command::new("bun").args(args).current_dir("web").status() {
            Ok(status) if status.success() => {}
            // web/build is committed, so a missing bun only means "not rebuilt".
            Ok(status) => panic!("bun {} failed: {}", args.join(" "), status),
            Err(e) => {
                println!("cargo:warning=skipping web build (bun {}): {}", args.join(" "), e);
                return;
            }
        }
    }
}
