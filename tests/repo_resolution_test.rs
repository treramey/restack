use std::process::Command;
use tempfile::tempdir;

fn restack_cmd() -> Command {
    Command::new(env!("CARGO_MANIFEST_DIR").to_string() + "/target/debug/restack")
}

#[test]
fn test_topic_list_auto_detects_from_repo_dir() {
    let workspace = tempdir().unwrap();
    let repo_dir = tempdir().unwrap();

    std::process::Command::new("git")
        .args(["init"])
        .current_dir(repo_dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(repo_dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(repo_dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "--allow-empty", "-m", "initial"])
        .current_dir(repo_dir.path())
        .output()
        .unwrap();

    let init_output = restack_cmd()
        .args(["init"])
        .current_dir(workspace.path())
        .output()
        .expect("restack init");
    assert!(init_output.status.success());

    let add_output = restack_cmd()
        .args([
            "repo",
            "add",
            "--name",
            "my-api",
            repo_dir.path().to_str().unwrap(),
        ])
        .current_dir(workspace.path())
        .output()
        .expect("restack repo add");
    assert!(add_output.status.success());

    let list_output = restack_cmd()
        .args(["topic", "list"])
        .current_dir(repo_dir.path())
        .output()
        .expect("restack topic list");

    assert!(
        list_output.status.success(),
        "topic list should succeed from repo dir: {:?}",
        String::from_utf8_lossy(&list_output.stderr)
    );
    let stdout = String::from_utf8_lossy(&list_output.stdout);
    assert!(stdout.contains("[]"), "should return empty topics array");
}

#[test]
fn test_topic_list_with_short_name() {
    let workspace = tempdir().unwrap();
    let repo_dir = tempdir().unwrap();

    std::process::Command::new("git")
        .args(["init"])
        .current_dir(repo_dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(repo_dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(repo_dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "--allow-empty", "-m", "initial"])
        .current_dir(repo_dir.path())
        .output()
        .unwrap();

    restack_cmd()
        .args(["init"])
        .current_dir(workspace.path())
        .output()
        .expect("restack init");

    restack_cmd()
        .args([
            "repo",
            "add",
            "--name",
            "my-api",
            repo_dir.path().to_str().unwrap(),
        ])
        .current_dir(workspace.path())
        .output()
        .expect("restack repo add");

    let list_output = restack_cmd()
        .args(["topic", "list", "--repo", "my-api"])
        .current_dir(workspace.path())
        .output()
        .expect("restack topic list");

    assert!(
        list_output.status.success(),
        "topic list should succeed with short name: {:?}",
        String::from_utf8_lossy(&list_output.stderr)
    );
}

#[test]
fn test_error_when_not_in_repo_and_no_repo_arg() {
    let workspace = tempdir().unwrap();

    restack_cmd()
        .args(["init"])
        .current_dir(workspace.path())
        .output()
        .expect("restack init");

    let list_output = restack_cmd()
        .args(["topic", "list"])
        .current_dir(workspace.path())
        .output()
        .expect("restack topic list");

    // When not in a repo and no --repo specified, currently returns empty array
    // (lists all repos' topics - empty since no repos added)
    // This test verifies it doesn't crash and returns valid JSON
    assert!(
        list_output.status.success(),
        "should succeed (returns empty array for no repos): {:?}",
        String::from_utf8_lossy(&list_output.stderr)
    );
    let stdout = String::from_utf8_lossy(&list_output.stdout);
    assert!(
        stdout.contains("[]"),
        "should return empty topics array: {}",
        stdout
    );
}
