use anyhow::Result;
use std::env;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{sleep, Duration};

pub async fn start_zinit(socket_path: &str, config_dir: &str) -> Result<()> {
    // Create a temporary config directory if it doesn't exist
    let config_path = Path::new(config_dir);
    if !config_path.exists() {
        tokio::fs::create_dir_all(config_path).await?;
    }

    // Get the path to the zinit binary (use the one we just built)
    let zinit_path = env::current_dir()?.join("target/debug/zinit");
    println!("Using zinit binary at: {}", zinit_path.display());

    // Start zinit in the background
    let mut cmd = Command::new(zinit_path);
    cmd.arg("--socket")
        .arg(socket_path)
        .arg("init")
        .arg("--config")
        .arg(config_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let child = cmd.spawn()?;

    // Give zinit some time to start up
    sleep(Duration::from_secs(1)).await;

    println!("Zinit started with PID: {:?}", child.id());

    Ok(())
}

pub async fn create_service_config(config_dir: &str, name: &str, command: &[&str]) -> Result<()> {
    let config_path = format!("{}/{}.toml", config_dir, name);
    let config_content = format!(
        r#"exec = {}
oneshot = false
shutdown_timeout = "10s"
after = []
log = "ring"
dir = "/"

[signal]
stop = "sigterm"
"#,
        serde_json::to_string(command)?
    );

    tokio::fs::write(config_path, config_content).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zinit::config;

    #[tokio::test]
    async fn generated_config_preserves_exec_arguments() {
        let dir = tempfile::tempdir().unwrap();
        create_service_config(
            dir.path().to_str().unwrap(),
            "worker",
            &["sh", "-c", "echo two words"],
        )
        .await
        .unwrap();

        let (_, service) = config::load(dir.path().join("worker.toml")).unwrap();
        assert_eq!(service.exec, ["sh", "-c", "echo two words"]);
    }
}
