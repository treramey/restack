use std::process::Command;
use tempfile::tempdir;

fn restack_cmd() -> Command {
    Command::new(env!("CARGO_MANIFEST_DIR").to_string() + "/target/debug/restack")
}

#[test]
fn test_repo_add_dot_uses_dir_name() {
    let workspace = tempdir().unwrap();
    let repo_dir = workspace.path().join("my-cool-project");
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

    let init_output = restack_cmd()
        .args(["init"])
        .current_dir(workspace.path())
        .output()
        .expect("restack init");
    assert!(init_output.status.success(), "init failed");

    let add_output = restack_cmd()
        .args(["add", "."])
        .current_dir(&repo_dir)
        .output()
        .expect("restack add .");

    assert!(
        add_output.status.success(),
        "add . should succeed: {:?}",
        String::from_utf8_lossy(&add_output.stderr)
    );

    let stdout = String::from_utf8_lossy(&add_output.stdout);
    assert!(
        stdout.contains("my-cool-project"),
        "repo name should be 'my-cool-project' not 'unnamed', got: {}",
        stdout
    );
    assert!(
        !stdout.contains("unnamed"),
        "repo name should NOT be 'unnamed', got: {}",
        stdout
    );
}
