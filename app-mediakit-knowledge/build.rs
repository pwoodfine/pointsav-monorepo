// Tell Cargo to recompile when any file in static/ changes, so that
// rust_embed picks up CSS / font / JS edits without a manual cargo clean.
fn main() {
    println!("cargo:rerun-if-changed=static/");
}
