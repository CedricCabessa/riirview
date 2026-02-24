use std::path::Path;
use std::{env, fs};
use vergen_git2::{Emitter, Git2Builder};

pub fn main() {
    extract_help();
    generate_empty_state_assets();
    generate_no_results_state_assets();

    let git2 = Git2Builder::default()
        .describe(true, true, None)
        .build()
        .unwrap();
    Emitter::default()
        .add_instructions(&git2)
        .unwrap()
        .emit()
        .unwrap();
}

fn extract_help() {
    let readme = fs::read_to_string("README.md").unwrap();
    let mut keymap = "".to_string();
    let mut chapter_found = false;
    let mut array_found = false;
    for line in readme.split('\n') {
        if line.starts_with("## Keymap") {
            chapter_found = true;
        } else if chapter_found && line.starts_with("|") {
            array_found = true;
        } else if array_found && !line.starts_with("|") {
            break;
        }
        if array_found {
            let len = line.len();
            keymap.push('\n');
            keymap.push_str(&line[1..len - 1]);
        }
    }
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("keymap.rs");
    fs::write(&dest_path, format!("const KEYMAP: &str = r#\"{keymap}\"#;")).unwrap();
}

fn generate_empty_state_assets() {
    let arts = collect_asset_paths("assets/art", "ans");
    let taglines = collect_asset_paths("assets/taglines", "txt");

    let mut code = generate_const_array("ARTS", "&[u8]", "include_bytes", &arts);
    code.push('\n');
    code.push_str(&generate_const_array(
        "TAGLINES",
        "&str",
        "include_str",
        &taglines,
    ));

    write_asset_module("empty_state.rs", &code, &["assets/art", "assets/taglines"]);
}

fn generate_no_results_state_assets() {
    let arts = collect_asset_paths("assets/no-results-art", "ans");
    let code = generate_const_array("ARTS", "&[u8]", "include_bytes", &arts);
    write_asset_module("no_results_state.rs", &code, &["assets/no-results-art"]);
}

/// Collect sorted file paths from a directory, filtered by extension.
fn collect_asset_paths(dir: &str, ext: &str) -> Vec<String> {
    let mut paths: Vec<String> = fs::read_dir(dir)
        .unwrap_or_else(|_| panic!("{dir} directory must exist"))
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|e| e == ext))
        .map(|e| e.path().display().to_string())
        .collect();
    paths.sort();
    paths
}

/// Generate an `include_bytes!` or `include_str!` array constant.
fn generate_const_array(name: &str, typ: &str, macro_name: &str, paths: &[String]) -> String {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let includes: String = paths
        .iter()
        .map(|p| format!("    {macro_name}!(\"{manifest_dir}/{p}\"),"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("pub const {name}: &[{typ}] = &[\n{includes}\n];\n")
}

/// Write generated code to `$OUT_DIR/{filename}` and emit `rerun-if-changed` for each directory.
fn write_asset_module(filename: &str, code: &str, watched_dirs: &[&str]) {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join(filename);
    fs::write(&dest_path, code).unwrap();
    for dir in watched_dirs {
        println!("cargo:rerun-if-changed={dir}");
    }
}
