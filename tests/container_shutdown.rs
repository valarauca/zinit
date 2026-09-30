use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use std::fs;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

#[test]
fn container_signals_preserve_ordered_cleanup_and_failure_status() {
    for (signal, exit_code) in [
        (Signal::SIGINT, 0),
        (Signal::SIGTERM, 0),
        (Signal::SIGUSR1, 1),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let started = dir.path().join("started");
        let stopped = dir.path().join("stopped");
        let crashed = dir.path().join("crashed");
        let command = format!(
            "trap 'sleep 0.8; touch {} ; exit 0' TERM INT; touch {}; while :; do sleep 0.1; done",
            stopped.display(),
            started.display()
        );
        let hook = format!("touch {}", crashed.display());
        fs::write(
            dir.path().join("worker.toml"),
            format!(
                "exec = [\"sh\", \"-c\", {:?}]\nshutdown_timeout = \"3s\"\nlog = \"stdout\"\non_crash = [{:?}]\n",
                command, hook
            ),
        ).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_zinit"))
            .args([
                "--socket",
                dir.path().join("zinit.sock").to_str().unwrap(),
                "init",
                "--container",
                "--config",
                dir.path().to_str().unwrap(),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !started.exists() {
            if child.try_wait().unwrap().is_some() || Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("zinit did not start the service from the supplied config directory");
            }
            sleep(Duration::from_millis(20));
        }
        kill(Pid::from_raw(child.id() as i32), signal).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("zinit did not finish container shutdown");
            }
            sleep(Duration::from_millis(20));
        };
        assert_eq!(status.code(), Some(exit_code));
        assert!(
            stopped.exists(),
            "cleanup requiring >500ms was killed early"
        );
        assert!(
            !crashed.exists(),
            "controlled shutdown incorrectly ran crash hooks"
        );
    }
}
