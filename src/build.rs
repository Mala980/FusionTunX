use std::fs;
use std::path::Path;

fn main() {
    let dist_dir = Path::new("dash/dist");
    if !dist_dir.exists() {
        fs::create_dir_all(dist_dir.join("assets")).ok();
        fs::write(
            dist_dir.join("index.html"),
            "<!doctype html><html><head><title>FusionTunX</title></head><body><div id=\"root\"></div></body></html>",
        ).ok();
    }
}
