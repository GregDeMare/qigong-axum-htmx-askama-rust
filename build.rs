fn main() {
    // Re-embed migrations when files in migrations/ change.
    println!("cargo:rerun-if-changed=migrations");
}
