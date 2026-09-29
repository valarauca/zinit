use std::path::Path;
use std::time::Duration;

use nix::sys::signal::{kill, Signal};
use tokio::time::{sleep, timeout};
use zinit::zinit::{config, State, ZInit};

fn hook(path: &Path) -> String {
    format!("sh -c 'printf crash > {}'", path.display())
}

async fn wait_for_file(path: &Path) {
    timeout(Duration::from_secs(5), async {
        while !path.exists() {
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("crash hook did not run");
}

async fn wait_for_pid(zinit: &ZInit, name: &str) -> nix::unistd::Pid {
    timeout(Duration::from_secs(5), async {
        loop {
            let status = zinit.status(name).await.unwrap();
            if status.pid.as_raw() != 0 {
                return status.pid;
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("service did not start")
}

#[tokio::test]
async fn runs_crash_hooks_only_for_unexpected_failed_exits() {
    let dir = tempfile::tempdir().unwrap();
    let zinit = ZInit::new(100, false);
    zinit.serve();

    let first = dir.path().join("first");
    let second = dir.path().join("second");
    let mut failed = config::Service::default();
    failed.exec = vec!["sh".into(), "-c".into(), "exit 7".into()];
    failed.log = config::Log::None;
    failed.on_crash = vec![hook(&first), hook(&second)];
    zinit.monitor("failed", failed).await.unwrap();
    wait_for_file(&first).await;
    wait_for_file(&second).await;
    zinit.stop("failed").await.unwrap();

    let success_marker = dir.path().join("success");
    let mut success = config::Service::default();
    success.exec = vec![
        "sh".into(),
        "-c".into(),
        "test \"$1\" = 'two words'".into(),
        "sh".into(),
        "two words".into(),
    ];
    success.one_shot = true;
    success.log = config::Log::None;
    success.on_crash = vec![hook(&success_marker)];
    zinit.monitor("success", success).await.unwrap();
    timeout(Duration::from_secs(5), async {
        loop {
            if zinit.status("success").await.unwrap().state == State::Success {
                break;
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(!success_marker.exists());

    let stop_marker = dir.path().join("stopped");
    let mut stopped = config::Service::default();
    stopped.exec = vec!["sleep".into(), "10".into()];
    stopped.log = config::Log::None;
    stopped.on_crash = vec![hook(&stop_marker)];
    zinit.monitor("stopped", stopped).await.unwrap();
    wait_for_pid(&zinit, "stopped").await;
    zinit.stop("stopped").await.unwrap();
    timeout(Duration::from_secs(5), async {
        while zinit.status("stopped").await.unwrap().pid.as_raw() != 0 {
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(!stop_marker.exists());

    let controlled_marker = dir.path().join("controlled");
    let mut controlled = config::Service::default();
    controlled.exec = vec!["sleep".into(), "10".into()];
    controlled.one_shot = true;
    controlled.log = config::Log::None;
    controlled.on_crash = vec![hook(&controlled_marker)];
    zinit.monitor("controlled", controlled).await.unwrap();
    wait_for_pid(&zinit, "controlled").await;
    zinit.kill("controlled", Signal::SIGKILL).await.unwrap();
    timeout(Duration::from_secs(5), async {
        while zinit.status("controlled").await.unwrap().pid.as_raw() != 0 {
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(!controlled_marker.exists());

    let signal_marker = dir.path().join("signaled");
    let mut signaled = config::Service::default();
    signaled.exec = vec!["sleep".into(), "10".into()];
    signaled.one_shot = true;
    signaled.log = config::Log::None;
    signaled.on_crash = vec![hook(&signal_marker)];
    zinit.monitor("signaled", signaled).await.unwrap();
    let pid = wait_for_pid(&zinit, "signaled").await;
    zinit.kill("signaled", Signal::SIGCONT).await.unwrap();
    kill(pid, Signal::SIGKILL).unwrap();
    wait_for_file(&signal_marker).await;
}
