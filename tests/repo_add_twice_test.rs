use std::process::Command;
use tempfile::tempdir;

fn restack_cmd() -> Command {
    Command::new(env!("CARGO_MANIFEST_DIR").to_string() + "/target/debug/restack")
}

#[test]
fn test_repo_add_twice_succeeds_with_note() {
    // With idempotent behavior, adding a repo twice should succeed
    // with a note that it was already tracked

    let workspace = tempdir().unwrap();
    let repo_dir = tempdir().unwrap();

    // Initialize git repo
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

    // Initialize restack workspace
    let init_output = restack_cmd()
        .args(["init"])
        .current_dir(workspace.path())
        .output()
        .expect("restack init");
    assert!(init_output.status.success(), "init failed");

    // First add should succeed
    let add_output = restack_cmd()
        .args(["add", "--json", repo_dir.path().to_str().unwrap()])
        .current_dir(workspace.path())
        .output()
        .expect("restack add");

    assert!(
        add_output.status.success(),
        "first add should succeed: {:?}",
        String::from_utf8_lossy(&add_output.stderr)
    );

    // Second add should also succeed (idempotent) with note
    let add_output2 = restack_cmd()
        .args(["add", "--json", repo_dir.path().to_str().unwrap()])
        .current_dir(workspace.path())
        .output()
        .expect("restack add 2");

    assert!(
        add_output2.status.success(),
        "second add should succeed (idempotent): {:?}",
        String::from_utf8_lossy(&add_output2.stderr)
    );

    let stdout = String::from_utf8_lossy(&add_output2.stdout);
    assert!(
        stdout.contains("already tracked") || stdout.contains("note"),
        "should indicate repo was already tracked: {}",
        stdout
    );
}

#[test]
fn test_repo_add_with_trailing_slash_is_idempotent() {
    let workspace = tempdir().unwrap();
    let repo_dir = tempdir().unwrap();

    // Initialize git repo
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

    // Initialize restack workspace
    let init_output = restack_cmd()
        .args(["init"])
        .current_dir(workspace.path())
        .output()
        .expect("restack init");
    assert!(init_output.status.success(), "init failed");

    // Add with trailing slash
    let repo_path_with_slash = format!("{}/", repo_dir.path().to_str().unwrap());
    let add_output = restack_cmd()
        .args(["add", "--json", &repo_path_with_slash])
        .current_dir(workspace.path())
        .output()
        .expect("restack add with slash");
    assert!(
        add_output.status.success(),
        "repo add with trailing slash should succeed: {:?}",
        String::from_utf8_lossy(&add_output.stderr)
    );

    // Try to add again without trailing slash - should succeed idempotently
    let add_output2 = restack_cmd()
        .args(["add", "--json", repo_dir.path().to_str().unwrap()])
        .current_dir(workspace.path())
        .output()
        .expect("restack add without slash");

    assert!(
        add_output2.status.success(),
        "second add should succeed (idempotent): {:?}",
        String::from_utf8_lossy(&add_output2.stderr)
    );

    let stdout = String::from_utf8_lossy(&add_output2.stdout);
    assert!(
        stdout.contains("already tracked") || stdout.contains("note"),
        "should indicate repo was already tracked: {}",
        stdout
    );
}

#[test]
fn test_repo_add_relative_path_from_parent_dir_is_idempotent() {
    // Simulate the user's scenario:
    // - Work in /workspace/work
    // - Clone repo to /workspace/work/gitworkflow-cli-test
    // - Try to add with relative path "gitworkflow-cli-test/"

    let workspace = tempdir().unwrap();
    let repo_dir = workspace.path().join("gitworkflow-cli-test");
    std::fs::create_dir(&repo_dir).unwrap();

    // Initialize git repo inside the subdirectory
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();

    // Initialize restack workspace in the parent directory
    let init_output = restack_cmd()
        .args(["init"])
        .current_dir(workspace.path())
        .output()
        .expect("restack init");
    assert!(init_output.status.success(), "init failed");

    // Add with relative path including trailing slash
    let add_output = restack_cmd()
        .args(["add", "--json", "gitworkflow-cli-test/"])
        .current_dir(workspace.path())
        .output()
        .expect("restack add with relative path");

    assert!(
        add_output.status.success(),
        "repo add with relative path should succeed: {:?}",
        String::from_utf8_lossy(&add_output.stderr)
    );

    // Try to add again - should succeed idempotently
    let add_output2 = restack_cmd()
        .args(["add", "--json", "gitworkflow-cli-test/"])
        .current_dir(workspace.path())
        .output()
        .expect("restack add second time");

    assert!(
        add_output2.status.success(),
        "second add should succeed (idempotent): {:?}",
        String::from_utf8_lossy(&add_output2.stderr)
    );

    let stdout = String::from_utf8_lossy(&add_output2.stdout);
    assert!(
        stdout.contains("already tracked") || stdout.contains("note"),
        "should indicate repo was already tracked: {}",
        stdout
    );
}

#[test]
fn test_repo_add_parent_dir_then_child_dir_both_succeed() {
    // Verify that parent and child directories are treated as separate repos

    let workspace = tempdir().unwrap();
    let parent_dir = workspace.path().join("work");
    let child_dir = parent_dir.join("gitworkflow-cli-test");
    std::fs::create_dir_all(&child_dir).unwrap();

    // Initialize git repos
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&parent_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&parent_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&parent_dir)
        .output()
        .unwrap();

    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&child_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&child_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&child_dir)
        .output()
        .unwrap();

    // Initialize restack workspace
    let init_output = restack_cmd()
        .args(["init"])
        .current_dir(workspace.path())
        .output()
        .expect("restack init");
    assert!(init_output.status.success(), "init failed");

    // Add parent directory (work/)
    let add_output = restack_cmd()
        .args(["add", "--json", parent_dir.to_str().unwrap()])
        .current_dir(workspace.path())
        .output()
        .expect("restack add parent");
    assert!(
        add_output.status.success(),
        "adding parent dir should succeed: {:?}",
        String::from_utf8_lossy(&add_output.stderr)
    );

    // Try to add child directory (work/gitworkflow-cli-test/)
    // This SHOULD succeed because it's a different path
    let add_output2 = restack_cmd()
        .args(["add", "--json", child_dir.to_str().unwrap()])
        .current_dir(workspace.path())
        .output()
        .expect("restack add child");

    // This should succeed - they're different repos at different paths
    assert!(
        add_output2.status.success(),
        "adding child dir should also succeed since it's a different path: {:?}",
        String::from_utf8_lossy(&add_output2.stderr)
    );
}
