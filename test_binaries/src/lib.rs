pub fn path_to_test_binaries() -> std::path::PathBuf {
    dbg!(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin"))
}
