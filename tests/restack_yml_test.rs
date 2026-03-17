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

fn init_git_repo(repo_path: &std::path::Path) {
    Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .expect("git init");
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(repo_path)
        .output()
        .expect("git config email");
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(repo_path)
        .output()
        .expect("git config name");
    Command::new("git")
        .args(["commit", "--allow-empty", "-m", "initial"])
        .current_dir(repo_path)
        .output()
        .expect("git commit");
}

#[test]
fn test_repo_add_with_restack_yml_creates_envs() {
    let binary = restack_binary();
    let workspace = tempdir().expect("create temp dir");
    let repo_dir = tempdir().expect("create repo dir");
    let repo_path = repo_dir.path();

    init_git_repo(repo_path);

    std::fs::write(
        repo_path.join(".restack.yml"),
        r#"version: "1"
environments:
  - name: dev
    branch: develop
  - name: staging
    branch: staging
"#,
    )
    .expect("write .restack.yml");

    let init_output = Command::new(&binary)
        .arg("init")
        .current_dir(workspace.path())
        .output()
        .expect("restack init");
    assert!(init_output.status.success(), "init failed");

    let add_output = Command::new(&binary)
        .args(["add", repo_path.to_str().unwrap()])
        .current_dir(workspace.path())
        .output()
        .expect("restack add");

    assert!(
        add_output.status.success(),
        "repo add failed: {:?}",
        String::from_utf8_lossy(&add_output.stderr)
    );

    let stdout = String::from_utf8_lossy(&add_output.stdout);
    assert!(
        stdout.contains("env_reconcile"),
        "should have env_reconcile field"
    );
    assert!(stdout.contains("dev"), "should have dev env");
    assert!(stdout.contains("staging"), "should have staging env");
}

#[test]
fn test_repo_add_without_restack_yml_uses_defaults() {
    let binary = restack_binary();
    let workspace = tempdir().expect("create temp dir");
    let repo_dir = tempdir().expect("create repo dir");
    let repo_path = repo_dir.path();

    init_git_repo(repo_path);

    let init_output = Command::new(&binary)
        .arg("init")
        .current_dir(workspace.path())
        .output()
        .expect("restack init");
    assert!(init_output.status.success());

    let add_output = Command::new(&binary)
        .args(["add", repo_path.to_str().unwrap()])
        .current_dir(workspace.path())
        .output()
        .expect("restack add");

    assert!(
        add_output.status.success(),
        "repo add failed: {:?}",
        String::from_utf8_lossy(&add_output.stderr)
    );

    let stdout = String::from_utf8_lossy(&add_output.stdout);
    assert!(
        stdout.contains("env_reconcile"),
        "should have env_reconcile (default .restack.yml generated)"
    );
    assert!(
        !stdout.contains("env_config_error"),
        "should NOT have env_config_error"
    );

    // Verify default .restack.yml was created
    assert!(
        repo_path.join(".restack.yml").exists(),
        "should have created .restack.yml"
    );
}

#[test]
fn test_yaml_version_validation_error() {
    let binary = restack_binary();
    let workspace = tempdir().expect("create temp dir");
    let repo_dir = tempdir().expect("create repo dir");
    let repo_path = repo_dir.path();

    init_git_repo(repo_path);

    std::fs::write(
        repo_path.join(".restack.yml"),
        r#"version: "2"
environments:
  - dev
"#,
    )
    .expect("write .restack.yml");

    let init_output = Command::new(&binary)
        .arg("init")
        .current_dir(workspace.path())
        .output()
        .expect("restack init");
    assert!(init_output.status.success());

    let add_output = Command::new(&binary)
        .args(["add", repo_path.to_str().unwrap()])
        .current_dir(workspace.path())
        .output()
        .expect("restack add");

    assert!(add_output.status.success());

    let stdout = String::from_utf8_lossy(&add_output.stdout);
    assert!(
        stdout.contains("env_config_error"),
        "should have env_config_error"
    );
    assert!(stdout.contains("version"), "error should mention version");
    assert!(
        stdout.contains("Fix:"),
        "error should contain Fix: suggestion"
    );
}

#[test]
fn test_yaml_production_branch_collision() {
    let binary = restack_binary();
    let workspace = tempdir().expect("create temp dir");
    let repo_dir = tempdir().expect("create repo dir");
    let repo_path = repo_dir.path();

    init_git_repo(repo_path);

    Command::new("git")
        .args(["checkout", "-b", "main"])
        .current_dir(repo_path)
        .output()
        .expect("git checkout main");

    std::fs::write(
        repo_path.join(".restack.yml"),
        r#"version: "1"
environments:
  - name: dev
    branch: main
"#,
    )
    .expect("write .restack.yml");

    let init_output = Command::new(&binary)
        .arg("init")
        .current_dir(workspace.path())
        .output()
        .expect("restack init");
    assert!(init_output.status.success());

    let add_output = Command::new(&binary)
        .args(["add", repo_path.to_str().unwrap()])
        .current_dir(workspace.path())
        .output()
        .expect("restack add");

    assert!(add_output.status.success());

    let stdout = String::from_utf8_lossy(&add_output.stdout);
    assert!(
        stdout.contains("env_config_error"),
        "should have env_config_error"
    );
    assert!(
        stdout.contains("production branch"),
        "error should mention production branch"
    );
}

#[test]
fn test_yaml_duplicate_branch_error() {
    let binary = restack_binary();
    let workspace = tempdir().expect("create temp dir");
    let repo_dir = tempdir().expect("create repo dir");
    let repo_path = repo_dir.path();

    init_git_repo(repo_path);

    std::fs::write(
        repo_path.join(".restack.yml"),
        r#"version: "1"
environments:
  - name: dev
    branch: develop
  - name: develop
    branch: develop
"#,
    )
    .expect("write .restack.yml");

    let init_output = Command::new(&binary)
        .arg("init")
        .current_dir(workspace.path())
        .output()
        .expect("restack init");
    assert!(init_output.status.success());

    let add_output = Command::new(&binary)
        .args(["add", repo_path.to_str().unwrap()])
        .current_dir(workspace.path())
        .output()
        .expect("restack add");

    assert!(add_output.status.success());

    let stdout = String::from_utf8_lossy(&add_output.stdout);
    assert!(
        stdout.contains("env_config_error"),
        "should have env_config_error"
    );
    assert!(
        stdout.contains("Duplicate branch"),
        "error should mention duplicate branch"
    );
}

#[test]
fn test_yaml_invalid_syntax_error() {
    let binary = restack_binary();
    let workspace = tempdir().expect("create temp dir");
    let repo_dir = tempdir().expect("create repo dir");
    let repo_path = repo_dir.path();

    init_git_repo(repo_path);

    std::fs::write(
        repo_path.join(".restack.yml"),
        r#"version: "1"
environments:
  - name: dev
    branch: [invalid
"#,
    )
    .expect("write .restack.yml");

    let init_output = Command::new(&binary)
        .arg("init")
        .current_dir(workspace.path())
        .output()
        .expect("restack init");
    assert!(init_output.status.success());

    let add_output = Command::new(&binary)
        .args(["add", repo_path.to_str().unwrap()])
        .current_dir(workspace.path())
        .output()
        .expect("restack add");

    assert!(add_output.status.success());

    let stdout = String::from_utf8_lossy(&add_output.stdout);
    assert!(
        stdout.contains("env_config_error"),
        "should have env_config_error"
    );
    assert!(
        stdout.contains("YAML syntax error"),
        "error should mention YAML syntax error"
    );
}

#[test]
fn test_auto_reconcile_on_env_list() {
    let binary = restack_binary();
    let workspace = tempdir().expect("create temp dir");
    let repo_dir = tempdir().expect("create repo dir");
    let repo_path = repo_dir.path();

    init_git_repo(repo_path);

    std::fs::write(
        repo_path.join(".restack.yml"),
        r#"version: "1"
environments:
  - dev
"#,
    )
    .expect("write .restack.yml");

    let init_output = Command::new(&binary)
        .arg("init")
        .current_dir(workspace.path())
        .output()
        .expect("restack init");
    assert!(init_output.status.success());

    let add_output = Command::new(&binary)
        .args(["add", "--name", "test-repo", repo_path.to_str().unwrap()])
        .current_dir(workspace.path())
        .output()
        .expect("restack add");
    assert!(
        add_output.status.success(),
        "repo add failed: {:?}",
        String::from_utf8_lossy(&add_output.stderr)
    );

    std::fs::write(
        repo_path.join(".restack.yml"),
        r#"version: "1"
environments:
  - dev
  - staging
"#,
    )
    .expect("update .restack.yml");

    let list_output = Command::new(&binary)
        .args(["integration", "list", "--repo", "test-repo"])
        .current_dir(workspace.path())
        .output()
        .expect("restack integration list");

    assert!(
        list_output.status.success(),
        "env list failed: {:?}",
        String::from_utf8_lossy(&list_output.stderr)
    );

    let stdout = String::from_utf8_lossy(&list_output.stdout);
    assert!(
        stdout.contains("staging"),
        "should have staging env after reconcile"
    );
}

#[test]
fn test_no_reconcile_flag_skips_reconciliation() {
    let binary = restack_binary();
    let workspace = tempdir().expect("create temp dir");
    let repo_dir = tempdir().expect("create repo dir");
    let repo_path = repo_dir.path();

    init_git_repo(repo_path);

    std::fs::write(
        repo_path.join(".restack.yml"),
        r#"version: "1"
environments:
  - dev
"#,
    )
    .expect("write .restack.yml");

    let init_output = Command::new(&binary)
        .arg("init")
        .current_dir(workspace.path())
        .output()
        .expect("restack init");
    assert!(init_output.status.success());

    let add_output = Command::new(&binary)
        .args(["add", "--name", "test-repo", repo_path.to_str().unwrap()])
        .current_dir(workspace.path())
        .output()
        .expect("restack add");
    assert!(
        add_output.status.success(),
        "repo add failed: {:?}",
        String::from_utf8_lossy(&add_output.stderr)
    );

    std::fs::write(
        repo_path.join(".restack.yml"),
        r#"version: "1"
environments:
  - dev
  - staging
"#,
    )
    .expect("update .restack.yml");

    let list_output = Command::new(&binary)
        .args([
            "--no-reconcile",
            "integration",
            "list",
            "--repo",
            "test-repo",
        ])
        .current_dir(workspace.path())
        .output()
        .expect("restack integration list --no-reconcile");

    assert!(
        list_output.status.success(),
        "env list failed: {:?}",
        String::from_utf8_lossy(&list_output.stderr)
    );

    let stdout = String::from_utf8_lossy(&list_output.stdout);
    assert!(
        !stdout.contains("staging"),
        "should NOT have staging env (no reconcile)"
    );
}

#[test]
fn test_restack_yml_exclude_patterns() {
    let binary = restack_binary();
    let workspace = tempdir().expect("create temp dir");
    let repo_dir = tempdir().expect("create repo dir");
    let repo_path = repo_dir.path();

    init_git_repo(repo_path);

    std::fs::write(
        repo_path.join(".restack.yml"),
        r#"version: "1"
environments:
  - dev
exclude_patterns:
  - "dependabot/*"
  - "renovate/*"
"#,
    )
    .expect("write .restack.yml");

    let init_output = Command::new(&binary)
        .arg("init")
        .current_dir(workspace.path())
        .output()
        .expect("restack init");
    assert!(init_output.status.success());

    let add_output = Command::new(&binary)
        .args(["add", "--name", "test-repo", repo_path.to_str().unwrap()])
        .current_dir(workspace.path())
        .output()
        .expect("restack add");
    assert!(
        add_output.status.success(),
        "add failed: {:?}",
        String::from_utf8_lossy(&add_output.stderr)
    );

    // Verify the repo was added successfully with the config
    let list_output = Command::new(&binary)
        .args(["list"])
        .current_dir(workspace.path())
        .output()
        .expect("restack list");

    assert!(list_output.status.success());
    let stdout = String::from_utf8_lossy(&list_output.stdout);
    assert!(stdout.contains("test-repo"), "should list the added repo");
}
