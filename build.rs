use std::path::Path;
use std::{env, fs};
use vergen_git2::{Emitter, Git2Builder};

pub fn main() {
    extract_help();
    generate_empty_state_assets();

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
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("empty_state.rs");

    let mut arts: Vec<String> = fs::read_dir("assets/art")
        .expect("assets/art directory must exist")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "ans"))
        .map(|e| e.path().display().to_string())
        .collect();
    arts.sort();

    let mut taglines: Vec<String> = fs::read_dir("assets/taglines")
        .expect("assets/taglines directory must exist")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "txt"))
        .map(|e| e.path().display().to_string())
        .collect();
    taglines.sort();

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    let art_includes: String = arts
        .iter()
        .map(|p| format!("    include_bytes!(\"{manifest_dir}/{p}\"),"))
        .collect::<Vec<_>>()
        .join("\n");

    let tagline_includes: String = taglines
        .iter()
        .map(|p| format!("    include_str!(\"{manifest_dir}/{p}\"),"))
        .collect::<Vec<_>>()
        .join("\n");

    let code = format!(
        "pub const ARTS: &[&[u8]] = &[\n{art_includes}\n];\n\n\
         pub const TAGLINES: &[&str] = &[\n{tagline_includes}\n];\n"
    );

    fs::write(&dest_path, code).unwrap();

    println!("cargo:rerun-if-changed=assets/art");
    println!("cargo:rerun-if-changed=assets/taglines");
}
