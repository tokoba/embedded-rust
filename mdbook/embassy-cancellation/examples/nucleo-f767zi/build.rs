//! build

/// カレントディレクトリーから memory.x を参照
fn main() {
  println!("cargo:rustc-link-search=.");
  println!("cargo:rerun-if-changed=memory.x");
}
