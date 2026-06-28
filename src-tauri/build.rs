fn main() {
    let manifest_dir = std::path::PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by Cargo"),
    );
    let dist_web = manifest_dir.join("..").join("dist-web");
    let fallback_web = manifest_dir.join("web-assets-placeholder");
    let web_assets_dir = if dist_web.exists() {
        &dist_web
    } else {
        &fallback_web
    };
    println!(
        "cargo:rustc-env=CC_SWITCH_WEB_ASSETS_DIR={}",
        web_assets_dir.display()
    );
    println!("cargo:rerun-if-changed={}", dist_web.display());
    println!("cargo:rerun-if-changed={}", fallback_web.display());

    #[cfg(feature = "desktop")]
    tauri_build::build();
}
