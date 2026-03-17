use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use tempfile::tempdir;

static BINARY: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    let output = Command::new("cargo")
        .args(["build"])
        .output()
        .expect("cargo build");
    assert!(
        output.status.success(),
        "build failed: {:?}",
        output.stderr
    );
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    std::path::Path::new(&manifest_dir)
        .join("target/debug/restack")
        .to_string_lossy()
        .to_string()
});

fn setup_workspace() -> tempfile::TempDir {
    let workspace = tempdir().expect("create temp dir");
    let binary = &*BINARY;

    let init = Command::new(&binary)
        .arg("init")
        .current_dir(workspace.path())
        .output()
        .expect("restack init");
    assert!(init.status.success(), "init failed: {:?}", init.stderr);

    workspace
}

#[test]
fn test_serve_responds_to_list_command() {
    let workspace = setup_workspace();
    let binary = BINARY.as_str();

    let mut child = Command::new(&binary)
        .arg("serve")
        .current_dir(workspace.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn restack serve");

    let stdin = child.stdin.as_mut().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);

    // Send a list request
    writeln!(stdin, r#"{{"id":"t1","args":["list"]}}"#).expect("write request");
    stdin.flush().expect("flush");

    let mut response = String::new();
    reader.read_line(&mut response).expect("read response");

    let resp: serde_json::Value = serde_json::from_str(&response).expect("parse JSON");
    assert_eq!(resp["id"], "t1");
    assert!(resp["result"].is_array(), "result should be an array");
    assert!(resp.get("error").is_none() || resp["error"].is_null());

    // Close stdin → serve exits cleanly
    drop(child.stdin.take());
    let status = child.wait().expect("wait");
    assert!(status.success(), "serve should exit cleanly on EOF");
}

#[test]
fn test_serve_returns_error_for_invalid_command() {
    let workspace = setup_workspace();
    let binary = BINARY.as_str();

    let mut child = Command::new(&binary)
        .arg("serve")
        .current_dir(workspace.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn restack serve");

    let stdin = child.stdin.as_mut().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);

    // Send a request with an unknown command
    writeln!(stdin, r#"{{"id":"t2","args":["nonexistent-cmd"]}}"#).expect("write");
    stdin.flush().expect("flush");

    let mut response = String::new();
    reader.read_line(&mut response).expect("read response");

    let resp: serde_json::Value = serde_json::from_str(&response).expect("parse JSON");
    assert_eq!(resp["id"], "t2");
    assert!(resp["error"].is_string(), "should return an error");

    drop(child.stdin.take());
    child.wait().expect("wait");
}

#[test]
fn test_serve_handles_multiple_requests() {
    let workspace = setup_workspace();
    let binary = BINARY.as_str();

    let mut child = Command::new(&binary)
        .arg("serve")
        .current_dir(workspace.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn restack serve");

    let stdin = child.stdin.as_mut().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);

    // Send multiple requests on the same connection
    for i in 1..=3 {
        writeln!(stdin, r#"{{"id":"m{}","args":["list"]}}"#, i).expect("write");
    }
    stdin.flush().expect("flush");

    // Read all three responses
    for i in 1..=3 {
        let mut response = String::new();
        reader.read_line(&mut response).expect("read response");

        let resp: serde_json::Value = serde_json::from_str(&response).expect("parse JSON");
        assert_eq!(resp["id"], format!("m{}", i));
        assert!(resp["result"].is_array());
    }

    drop(child.stdin.take());
    let status = child.wait().expect("wait");
    assert!(status.success());
}

#[test]
fn test_serve_handles_malformed_json() {
    let workspace = setup_workspace();
    let binary = BINARY.as_str();

    let mut child = Command::new(&binary)
        .arg("serve")
        .current_dir(workspace.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn restack serve");

    let stdin = child.stdin.as_mut().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);

    // Send malformed JSON
    writeln!(stdin, "not valid json").expect("write");
    stdin.flush().expect("flush");

    let mut response = String::new();
    reader.read_line(&mut response).expect("read response");

    let resp: serde_json::Value = serde_json::from_str(&response).expect("parse JSON");
    assert_eq!(resp["id"], "unknown");
    assert!(resp["error"].is_string());

    // Serve should still be alive — send a valid request
    writeln!(stdin, r#"{{"id":"ok","args":["list"]}}"#).expect("write");
    stdin.flush().expect("flush");

    let mut response2 = String::new();
    reader.read_line(&mut response2).expect("read response");

    let resp2: serde_json::Value = serde_json::from_str(&response2).expect("parse JSON");
    assert_eq!(resp2["id"], "ok");
    assert!(resp2["result"].is_array());

    drop(child.stdin.take());
    child.wait().expect("wait");
}

#[test]
fn test_serve_skips_empty_lines() {
    let workspace = setup_workspace();
    let binary = BINARY.as_str();

    let mut child = Command::new(&binary)
        .arg("serve")
        .current_dir(workspace.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn restack serve");

    let stdin = child.stdin.as_mut().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);

    // Send empty lines then a valid request
    writeln!(stdin).expect("write");
    writeln!(stdin, "   ").expect("write");
    writeln!(stdin, r#"{{"id":"after_empty","args":["list"]}}"#).expect("write");
    stdin.flush().expect("flush");

    let mut response = String::new();
    reader.read_line(&mut response).expect("read response");

    let resp: serde_json::Value = serde_json::from_str(&response).expect("parse JSON");
    assert_eq!(resp["id"], "after_empty");
    assert!(resp["result"].is_array());

    drop(child.stdin.take());
    child.wait().expect("wait");
}
