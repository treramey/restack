use std::process::Command;
use tempfile::tempdir;

fn restack_cmd() -> Command {
    Command::new(env!("CARGO_MANIFEST_DIR").to_string() + "/target/debug/restack")
}

#[test]
fn test_repo_add_after_external_delete_and_reclone() {
    // This reproduces the exact bug scenario:
    // 1. Add repo to restack
    // 2. Delete repo externally (rm -rf)
    // 3. Clone repo again
    // 4. Try to add repo -> should succeed with idempotent behavior

    let workspace = tempdir().unwrap();
    let repo_parent = workspace.path().join("repos");
    std::fs::create_dir(&repo_parent).unwrap();

    // Create initial repo
    let repo_dir = repo_parent.join("test-repo");
    std::fs::create_dir(&repo_dir).unwrap();

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

    // Initialize restack workspace
    let init_output = restack_cmd()
        .args(["init"])
        .current_dir(workspace.path())
        .output()
        .expect("restack init");
    assert!(init_output.status.success(), "init failed");

    // Add the repo
    let add_output = restack_cmd()
        .args(["add", "--json", repo_dir.to_str().unwrap()])
        .current_dir(workspace.path())
        .output()
        .expect("restack add");
    assert!(
        add_output.status.success(),
        "first add should succeed: {:?}",
        String::from_utf8_lossy(&add_output.stderr)
    );

    // Delete repo externally (simulating rm -rf)
    std::fs::remove_dir_all(&repo_dir).unwrap();

    // Recreate/re-clone the repo (simulating git clone)
    std::fs::create_dir(&repo_dir).unwrap();
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

    // Try to add again - with the fix, this should now succeed
    let add_output2 = restack_cmd()
        .args(["add", "--json", repo_dir.to_str().unwrap()])
        .current_dir(workspace.path())
        .output()
        .expect("restack add after reclone");

    // After fix: should succeed with note that it was already tracked
    assert!(
        add_output2.status.success(),
        "second add should succeed after fix: {:?}",
        String::from_utf8_lossy(&add_output2.stderr)
    );

    let stdout = String::from_utf8_lossy(&add_output2.stdout);
    assert!(
        stdout.contains("already tracked") || stdout.contains("note"),
        "output should indicate repo was already tracked: {}",
        stdout
    );
}
