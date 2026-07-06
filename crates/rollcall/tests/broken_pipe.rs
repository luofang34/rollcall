//! The status run must survive its reader going away (`status | head`):
//! the snapshot is the artifact, stdout is best-effort.

#![allow(clippy::expect_used, clippy::panic)]

use std::net::TcpListener;
use std::process::{Command, Stdio};

#[test]
fn closed_stdout_does_not_lose_the_snapshot() {
    // A bound-then-dropped ephemeral port gives a fast, offline "down" probe.
    let addr = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        listener.local_addr().expect("local addr")
    };
    let repo = tempfile::tempdir().expect("temp repo");
    std::fs::create_dir(repo.path().join("inventory")).expect("mkdir inventory");
    std::fs::write(
        repo.path().join("inventory").join("probes.toml"),
        format!(
            "[[probe]]\nid = \"svc\"\ndesc = \"svc\"\nkind = \"http\"\nurl = \"http://{addr}/\"\n"
        ),
    )
    .expect("write probes.toml");

    let mut child = Command::new(env!("CARGO_BIN_EXE_rollcall"))
        .arg("status")
        .arg("--repo")
        .arg(repo.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn rollcall");
    // Close the read side so the child's stdout writes hit a broken pipe.
    drop(child.stdout.take());
    let status = child.wait().expect("child exits");

    assert!(
        status.success(),
        "status run must not die on a broken pipe (got {status:?})"
    );
    let snapshots: Vec<_> = std::fs::read_dir(repo.path().join("status"))
        .expect("status dir written despite broken pipe")
        .collect();
    assert_eq!(snapshots.len(), 1, "exactly one dated snapshot written");
}
