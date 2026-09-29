use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::time::Duration;
pub type Services = HashMap<String, Service>;

pub const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Signal {
    pub stop: String,
}

impl Default for Signal {
    fn default() -> Self {
        Signal {
            stop: String::from("sigterm"),
        }
    }
}

#[derive(Default, Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Log {
    None,
    #[default]
    Ring,
    Stdout,
}

fn default_shutdown_timeout_fn() -> Duration {
    DEFAULT_SHUTDOWN_TIMEOUT
}
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Service {
    /// executable followed by its arguments
    pub exec: Vec<String>,
    /// test command (optional)
    #[serde(default)]
    pub test: String,
    #[serde(rename = "oneshot")]
    pub one_shot: bool,
    #[serde(
        default = "default_shutdown_timeout_fn",
        deserialize_with = "duration_str::deserialize_duration"
    )]
    pub shutdown_timeout: Duration,
    pub on_crash: Vec<String>,
    pub after: Vec<String>,
    pub signal: Signal,
    pub log: Log,
    pub env: HashMap<String, String>,
    pub dir: String,
}

impl Default for Service {
    fn default() -> Self {
        Self {
            exec: Vec::new(),
            test: String::new(),
            one_shot: false,
            shutdown_timeout: DEFAULT_SHUTDOWN_TIMEOUT,
            on_crash: Vec::new(),
            after: Vec::new(),
            signal: Signal::default(),
            log: Log::default(),
            env: HashMap::new(),
            dir: String::new(),
        }
    }
}

impl Service {
    pub fn validate(&self) -> Result<()> {
        use nix::sys::signal::Signal;
        use std::str::FromStr;
        if self.exec.first().map_or(true, String::is_empty) {
            bail!("missing exec directive");
        }

        Signal::from_str(&self.signal.stop.to_uppercase())?;

        Ok(())
    }
}
/// load loads a single file
pub fn load<T: AsRef<Path>>(t: T) -> Result<(String, Service)> {
    let p = t.as_ref();
    //todo: can't find a way to shorten this down.
    let name = match p.file_stem() {
        Some(name) => match name.to_str() {
            Some(name) => name,
            None => bail!("invalid file name: {}", p.to_str().unwrap()),
        },
        None => bail!("invalid file name: {}", p.to_str().unwrap()),
    };

    let content = fs::read_to_string(p)?;
    let service: Service = toml::from_str(&content)?;
    service.validate()?;
    Ok((String::from(name), service))
}

/// walks over a directory and load all configuration files.
/// the callback is called with any error that is encountered on loading
/// a file, the callback can decide to either ignore the file, or stop
/// the directory walking
pub fn load_dir<T: AsRef<Path>>(p: T) -> Result<Services> {
    let mut services: Services = HashMap::new();

    for entry in fs::read_dir(p)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }

        let fp = entry.path();

        if !matches!(fp.extension(), Some(ext) if ext == OsStr::new("toml")) {
            continue;
        }

        let (name, service) = match load(&fp) {
            Ok(content) => content,
            Err(err) => {
                error!("failed to load config file {:?}: {}", fp, err);
                continue;
            }
        };

        services.insert(name, service);
    }

    Ok(services)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_toml_service_with_argument_array_and_duration() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("worker.toml");
        fs::write(
            &file,
            r#"
exec = ["/bin/echo", "two words"]
shutdown_timeout = "1m 250ms"
on_crash = ["/bin/false", "sh -c 'exit 2'"]

[signal]
stop = "SIGINT"
"#,
        )
        .unwrap();
        fs::write(dir.path().join("ignored.yaml"), "exec: /bin/false").unwrap();

        let services = load_dir(dir.path()).unwrap();
        assert_eq!(services.len(), 1);
        let worker = &services["worker"];
        assert_eq!(worker.exec, ["/bin/echo", "two words"]);
        assert_eq!(worker.shutdown_timeout, Duration::from_millis(60_250));
        assert_eq!(worker.on_crash.len(), 2);
        assert_eq!(worker.signal.stop, "SIGINT");
    }

    #[test]
    fn rejects_invalid_duration_and_empty_exec() {
        assert!(toml::from_str::<Service>("exec = []")
            .unwrap()
            .validate()
            .is_err());
        assert!(toml::from_str::<Service>("exec = ['echo']\nshutdown_timeout = 'soon'").is_err());
        assert_eq!(
            toml::from_str::<Service>("exec = ['echo']")
                .unwrap()
                .shutdown_timeout,
            DEFAULT_SHUTDOWN_TIMEOUT
        );
    }
}
