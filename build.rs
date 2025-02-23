use std::path::Path;
use std::{env, fs};
use vergen_git2::{Emitter, Git2Builder};

pub fn main() {
    extract_help();

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
