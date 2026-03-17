use std::process::Command;
use tempfile::tempdir;

fn restack_binary() -> String {
    let output = Command::new("cargo")
        .args(["build"])
        .output()
        .expect("cargo build");
    assert!(output.status.success(), "build failed: {:?}", output.stderr);

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let binary_path = std::path::Path::new(&manifest_dir).join("target/debug/restack");
    binary_path.to_string_lossy().to_string()
}

#[test]
fn test_repo_add_cli_always_discovers() {
    let binary = restack_binary();
    let workspace = tempdir().expect("create temp dir");

    let repo_dir = tempdir().expect("create repo dir");
    std::fs::create_dir(repo_dir.path().join(".git")).expect("create .git");

    let init_output = Command::new(&binary)
        .arg("init")
        .current_dir(workspace.path())
        .output()
        .expect("restack init");
    assert!(
        init_output.status.success(),
        "init failed: {:?}",
        init_output.stderr
    );

    let add_output = Command::new(&binary)
        .args(["add", "--json", repo_dir.path().to_str().unwrap()])
        .current_dir(workspace.path())
        .output()
        .expect("restack add");

    assert!(
        add_output.status.success(),
        "add failed: {:?}",
        add_output.stderr
    );

    let stdout = String::from_utf8_lossy(&add_output.stdout);
    // add now always discovers topics
    assert!(
        stdout.contains("discovery"),
        "output should contain discovery field"
    );
}

#[test]
fn test_repo_add_cli_with_feature_branch() {
    let binary = restack_binary();
    let workspace = tempdir().expect("create temp dir");

    let repo_dir = tempdir().expect("create repo dir");
    let repo_path = repo_dir.path();

    std::process::Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "--allow-empty", "-m", "initial"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["checkout", "-b", "feature/cli-test"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let init_output = Command::new(&binary)
        .arg("init")
        .current_dir(workspace.path())
        .output()
        .expect("restack init");
    assert!(init_output.status.success());

    let add_output = Command::new(&binary)
        .args(["add", "--json", repo_dir.path().to_str().unwrap()])
        .current_dir(workspace.path())
        .output()
        .expect("restack add");

    assert!(
        add_output.status.success(),
        "add failed: {:?}",
        add_output.stderr
    );

    let stdout = String::from_utf8_lossy(&add_output.stdout);
    assert!(
        stdout.contains("discovery"),
        "output should contain discovery field"
    );
    assert!(
        stdout.contains("discovered"),
        "output should contain discovered count"
    );
}
