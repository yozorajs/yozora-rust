use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct TestDirectory(PathBuf);

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
#[ignore = "requires Neovim 0.11 or later in PATH"]
fn native_client_round_trips_document_features() {
    let directory = TestDirectory(std::env::temp_dir().join(format!(
        "yozora-neovim-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    )));
    fs::create_dir(&directory.0).unwrap();
    let output = Command::new("nvim")
        .args(["--clean", "--headless", "-n", "-i", "NONE", "-l"])
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/neovim.lua"))
        .arg(env!("CARGO_BIN_EXE_yozora-lsp"))
        .arg(directory.0.join("workspace space 中"))
        .current_dir(&directory.0)
        .env("XDG_CONFIG_HOME", directory.0.join("config"))
        .env("XDG_DATA_HOME", directory.0.join("data"))
        .env("XDG_STATE_HOME", directory.0.join("state"))
        .env("XDG_CACHE_HOME", directory.0.join("cache"))
        .output()
        .expect("failed to start Neovim; this test requires nvim 0.11 or later in PATH");
    assert!(
        output.status.success(),
        "Neovim integration failed: {}\n{}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
