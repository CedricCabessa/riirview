use vergen_git2::{Emitter, Git2Builder};

pub fn main() {
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
