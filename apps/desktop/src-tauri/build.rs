fn main() {
    tauri_build::build();

    // Re-run if the authoring guide changes (used via include_str! in prompts.rs).
    let guide = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("playbook-authoring-guide.md");
    println!("cargo::rerun-if-changed={}", guide.display());
}
