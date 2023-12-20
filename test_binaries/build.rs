include!("src/lib.rs");
fn main() {
    let _output = std::process::Command::new("git")
        .args([
            "clone",
            "https://github.com/zingolabs/test_binaries.git",
            crate::path_to_test_binaries()
                .to_str()
                .expect("to get str from PathBuf"),
        ])
        .output()
        .expect("Failed to git clone test binaries.");
}
