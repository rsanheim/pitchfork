mod common;

use common::TestEnv;
use pitchfork_cli::pitchfork_toml;
use std::fs;
use std::time::Duration;

// ============================================================================
// Worktree isolation E2E tests
//
// These cover the real-world layout that motivated the feature: multiple
// checkouts (git worktrees) of one project whose directories share the same
// leaf name (e.g. ~/src/myproj and ~/.codex/worktrees/c015/myproj), all
// sharing a single supervisor.
// ============================================================================

const WT_CONFIG_A: &str = r#"
namespace = "myproj"
namespace_per_worktree = true

[daemons.api]
run = "sh -c 'echo hello from checkout A && sleep 60'"
"#;

const WT_CONFIG_B: &str = r#"
namespace = "myproj"
namespace_per_worktree = true

[daemons.api]
run = "sh -c 'echo hello from checkout B && sleep 60'"
"#;

/// Two checkouts with the same leaf dir name run the same daemon side by side
/// under one supervisor, with separate namespaces and logs, and `stop -l`
/// only stops the current checkout's daemon.
#[test]
fn test_namespace_per_worktree_same_leaf_checkouts() {
    let env = TestEnv::new();
    env.ensure_binary_exists().unwrap();

    // Same leaf name "myproj" in both checkouts — the colliding layout.
    let checkout_a = env.project_dir().join("a").join("myproj");
    let checkout_b = env.project_dir().join("b").join("myproj");
    fs::create_dir_all(&checkout_a).unwrap();
    fs::create_dir_all(&checkout_b).unwrap();
    fs::write(checkout_a.join("pitchfork.toml"), WT_CONFIG_A).unwrap();
    fs::write(checkout_b.join("pitchfork.toml"), WT_CONFIG_B).unwrap();

    let ns_a = pitchfork_toml::namespace_from_path(&checkout_a.join("pitchfork.toml")).unwrap();
    let ns_b = pitchfork_toml::namespace_from_path(&checkout_b.join("pitchfork.toml")).unwrap();
    assert_ne!(
        ns_a, ns_b,
        "same-leaf checkouts must get distinct namespaces"
    );

    // Start the same-named daemon from each checkout
    let out_a = env.run_command_in_dir(&["start", "api"], &checkout_a);
    assert!(
        out_a.status.success(),
        "start in A failed: {}",
        String::from_utf8_lossy(&out_a.stderr)
    );
    let out_b = env.run_command_in_dir(&["start", "api"], &checkout_b);
    assert!(
        out_b.status.success(),
        "start in B failed: {}",
        String::from_utf8_lossy(&out_b.stderr)
    );

    env.sleep(Duration::from_secs(1));

    // Both run concurrently with separate logs
    let log_a = env.wait_for_logs(
        &format!("{ns_a}/api"),
        "hello from checkout A",
        Duration::from_secs(5),
    );
    assert!(log_a.contains("hello from checkout A"), "got: {log_a}");
    let log_b = env.wait_for_logs(
        &format!("{ns_b}/api"),
        "hello from checkout B",
        Duration::from_secs(5),
    );
    assert!(log_b.contains("hello from checkout B"), "got: {log_b}");

    // stop -l from checkout A only stops A's daemon
    let stop_a = env.run_command_in_dir(&["stop", "-l"], &checkout_a);
    assert!(
        stop_a.status.success(),
        "stop -l in A failed: {}",
        String::from_utf8_lossy(&stop_a.stderr)
    );
    env.sleep(Duration::from_secs(1));

    let status_a = env.run_command_in_dir(&["status", "api"], &checkout_a);
    let status_a_str = String::from_utf8_lossy(&status_a.stdout).to_string();
    assert!(
        status_a_str.contains("stopped") || status_a_str.contains("exited"),
        "A's api should be stopped after stop -l in A, got: {status_a_str}"
    );

    let status_b = env.run_command_in_dir(&["status", "api"], &checkout_b);
    let status_b_str = String::from_utf8_lossy(&status_b.stdout).to_string();
    assert!(
        status_b_str.contains("running"),
        "B's api must keep running after stop -l in A, got: {status_b_str}"
    );

    // Cleanup
    let _ = env.run_command_in_dir(&["stop", "-l"], &checkout_b);
}

/// A worktree nested inside the main checkout (e.g. .worktrees/feature) gets
/// its own namespace, and `stop -l` inside it never touches the parent
/// checkout's daemons even though the parent's pitchfork.toml is in the
/// directory hierarchy above it.
#[test]
fn test_namespace_per_worktree_nested_worktree() {
    let env = TestEnv::new();
    env.ensure_binary_exists().unwrap();

    let root = env.project_dir().join("myproj");
    let nested = root.join(".worktrees").join("feature");
    fs::create_dir_all(&nested).unwrap();
    fs::write(root.join("pitchfork.toml"), WT_CONFIG_A).unwrap();
    fs::write(nested.join("pitchfork.toml"), WT_CONFIG_B).unwrap();

    let ns_root = pitchfork_toml::namespace_from_path(&root.join("pitchfork.toml")).unwrap();
    let ns_nested = pitchfork_toml::namespace_from_path(&nested.join("pitchfork.toml")).unwrap();
    assert_ne!(ns_root, ns_nested);

    let out_root = env.run_command_in_dir(&["start", "api"], &root);
    assert!(
        out_root.status.success(),
        "start in root failed: {}",
        String::from_utf8_lossy(&out_root.stderr)
    );
    let out_nested = env.run_command_in_dir(&["start", "api"], &nested);
    assert!(
        out_nested.status.success(),
        "start in nested worktree failed: {}",
        String::from_utf8_lossy(&out_nested.stderr)
    );

    env.sleep(Duration::from_secs(1));

    // stop -l in the nested worktree must not stop the parent checkout's daemon
    let stop_nested = env.run_command_in_dir(&["stop", "-l"], &nested);
    assert!(
        stop_nested.status.success(),
        "stop -l in nested failed: {}",
        String::from_utf8_lossy(&stop_nested.stderr)
    );
    env.sleep(Duration::from_secs(1));

    let status_nested = env.run_command_in_dir(&["status", "api"], &nested);
    let status_nested_str = String::from_utf8_lossy(&status_nested.stdout).to_string();
    assert!(
        status_nested_str.contains("stopped") || status_nested_str.contains("exited"),
        "nested api should be stopped, got: {status_nested_str}"
    );

    let status_root = env.run_command_in_dir(&["status", "api"], &root);
    let status_root_str = String::from_utf8_lossy(&status_root.stdout).to_string();
    assert!(
        status_root_str.contains("running"),
        "root api must keep running after stop -l in nested worktree, got: {status_root_str}"
    );

    // Cleanup
    let _ = env.run_command_in_dir(&["stop", "-l"], &root);
}

const WT_TEMPLATE_CONFIG: &str = r#"
namespace = "myproj"
namespace_per_worktree = true

[daemons.api]
run = "sh -c 'echo id={{ id }} hash={{ worktree_hash }} && sleep 60'"
ready_output = "id="

[daemons.api.hooks]
on_ready = "sh -c 'echo {{ namespace }} > {{ dir }}/ready.txt'"
"#;

/// Configuration templates resolve to per-checkout values: `{{ id }}` and
/// `{{ worktree_hash }}` in `run` render each checkout's own identity, and a
/// hook (re-rendered supervisor-side at fire time) sees the hashed namespace.
#[test]
fn test_namespace_per_worktree_templates() {
    let env = TestEnv::new();
    env.ensure_binary_exists().unwrap();

    let checkout_a = env.project_dir().join("a").join("myproj");
    let checkout_b = env.project_dir().join("b").join("myproj");
    fs::create_dir_all(&checkout_a).unwrap();
    fs::create_dir_all(&checkout_b).unwrap();
    fs::write(checkout_a.join("pitchfork.toml"), WT_TEMPLATE_CONFIG).unwrap();
    fs::write(checkout_b.join("pitchfork.toml"), WT_TEMPLATE_CONFIG).unwrap();

    let ns_a = pitchfork_toml::namespace_from_path(&checkout_a.join("pitchfork.toml")).unwrap();
    let ns_b = pitchfork_toml::namespace_from_path(&checkout_b.join("pitchfork.toml")).unwrap();
    let hash_a = ns_a.rsplit('-').next().unwrap().to_string();
    let hash_b = ns_b.rsplit('-').next().unwrap().to_string();
    assert_ne!(hash_a, hash_b);

    for checkout in [&checkout_a, &checkout_b] {
        let out = env.run_command_in_dir(&["start", "api"], checkout);
        assert!(
            out.status.success(),
            "start in {} failed: {}",
            checkout.display(),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    env.sleep(Duration::from_secs(1));

    // run templates rendered each checkout's own id and hash
    for (ns, hash) in [(&ns_a, &hash_a), (&ns_b, &hash_b)] {
        let log = env.wait_for_logs(
            &format!("{ns}/api"),
            &format!("hash={hash}"),
            Duration::from_secs(5),
        );
        assert!(log.contains(&format!("id={ns}/api")), "got: {log}");
        assert!(log.contains(&format!("hash={hash}")), "got: {log}");
    }

    // on_ready hooks (re-rendered at fire time by the supervisor) wrote each
    // checkout's hashed namespace next to its config
    for (checkout, ns) in [(&checkout_a, &ns_a), (&checkout_b, &ns_b)] {
        let ready_file = checkout.join("ready.txt");
        let mut content = String::new();
        for _ in 0..20 {
            if ready_file.exists() {
                content = fs::read_to_string(&ready_file).unwrap_or_default();
                if !content.is_empty() {
                    break;
                }
            }
            env.sleep(Duration::from_millis(500));
        }
        assert_eq!(
            content.trim(),
            ns,
            "on_ready hook in {} should write the hashed namespace",
            checkout.display()
        );
    }

    // Cleanup
    let _ = env.run_command_in_dir(&["stop", "-l"], &checkout_a);
    let _ = env.run_command_in_dir(&["stop", "-l"], &checkout_b);
}
