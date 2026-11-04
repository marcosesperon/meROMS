// ScreenScraper developer credentials built into the app (as Gamelist Manager does), so users
// only enter their own account. They are never in the repository:
// - GitHub Actions: secrets SS_DEVID / SS_DEVPASSWORD -> env MEROMS_SS_DEVID / MEROMS_SS_DEVPASSWORD
// - Locally: those environment variables or the file src-tauri/screenscraper.env (ignored by git;
//   see screenscraper.env.example)
// Without them the app still builds and users enter them in Settings > Advanced.
fn main() {
    println!("cargo:rerun-if-env-changed=MEROMS_SS_DEVID");
    println!("cargo:rerun-if-env-changed=MEROMS_SS_DEVPASSWORD");
    println!("cargo:rerun-if-changed=screenscraper.env");

    let from_file = std::fs::read_to_string("screenscraper.env").unwrap_or_default();
    for key in ["MEROMS_SS_DEVID", "MEROMS_SS_DEVPASSWORD"] {
        let value = std::env::var(key).ok().filter(|v| !v.trim().is_empty()).or_else(|| {
            from_file
                .lines()
                .filter_map(|line| line.split_once('='))
                .find(|(k, _)| k.trim() == key)
                .map(|(_, v)| v.trim().trim_matches('"').to_string())
                .filter(|v| !v.is_empty())
        });
        if let Some(value) = value {
            println!("cargo:rustc-env={}={}", key, value);
        }
    }

    tauri_build::build()
}
