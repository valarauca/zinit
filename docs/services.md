# Service Configuration Format

Zinit reads one TOML file per service from its configuration directory (default: `/etc/zinit`). The filename is the service name followed by `.toml`, such as `/etc/zinit/worker.toml`. Use `-c` to choose another directory.

```toml
exec = ["/usr/local/bin/worker"]
test = "/usr/local/bin/worker-health"
oneshot = false
shutdown_timeout = "30s"
after = ["networking", "database"]
on_crash = ["/usr/local/bin/notify-queue", "sh -c 'echo worker failed >&2'"]
log = "stdout"
dir = "/opt/worker"

[signal]
stop = "SIGTERM"

[env]
QUEUE_URL = "http://localhost:8000"
```

Only `exec` is required. Each element of `exec` is one argument: the executable first, then its arguments. Zinit passes these arguments directly to the process. Use an explicit shell such as `sh -c` for pipes, redirects, variable expansion, or other shell features. `test` and `on_crash` entries remain command strings split using shell-style quoting.

| Field | Default | Description |
|-------|---------|-------------|
| `exec` | Required | Array containing the executable and its arguments. |
| `test` | `""` | Command to check readiness. Exit code 0 marks the service running. |
| `oneshot` | `false` | Run once without automatic restart. Dependents wait for successful completion. |
| `shutdown_timeout` | `"10s"` | Time allowed during ordered shutdown. Use a duration string such as `"500ms"`, `"30s"`, or `"2m"`. |
| `after` | `[]` | Service names that must be running or have completed successfully first. |
| `on_crash` | `[]` | Commands to start when this service exits unexpectedly with a nonzero code or terminates from a signal. |
| `signal.stop` | `"sigterm"` | Signal zinit sends for a requested stop. |
| `log` | `"ring"` | Output handling: `"ring"`, `"stdout"`, or `"none"`. |
| `env` | `{}` | Environment variables added to the process environment. |
| `dir` | `""` | Working directory. An empty value uses `/`. |

The `shutdown_timeout` value is parsed by [`duration-str`](https://docs.rs/duration-str/0.21.0/duration_str/). Units are explicit in the examples; numeric TOML values are not accepted for this field.

For containers, `log = "stdout"` writes service output to zinit's stdout. The `"ring"` setting uses `/dev/kmsg`; Docker needs `--device=/dev/kmsg:/dev/kmsg:rw` for that device.

## Crash hooks

Each `on_crash` item is a command line split using shell-style quoting. Hooks use the same environment, working directory, and log setting as `exec`. Zinit starts every configured hook when the managed process exits with a nonzero code or is killed by a signal. Hook failures are logged. Hooks are not themselves supervised or restarted.

Hooks never run after a zero exit. A regular service restarts without running its hooks; a `oneshot` service stays stopped.

Hooks do not run after `zinit stop` or an ordered shutdown. If `zinit kill` terminates the process with the requested signal, hooks do not run. A `zinit kill` using SIGTERM, SIGINT, SIGQUIT, or the configured stop signal also counts as a requested stop if the process handles the signal and later exits with a nonzero code.

A failing `oneshot` service runs its hooks after it exits; a regular service runs its hooks before its normal restart delay. Hooks run independently, so a long-running hook does not delay other hooks or the regular service restart.

For a queue worker, a hook can signal a separate queue process when the worker fails:

```toml
exec = ["/usr/local/bin/model-worker"]
on_crash = ["/usr/local/bin/cancel-queue-job.sh"]
log = "stdout"
```

## More examples

### Web server

```toml
exec = ["/usr/bin/nginx", "-g", "daemon off;"]
test = "curl -fsS http://localhost/health"
after = ["networking"]
log = "stdout"
```

### Database initialization

```toml
exec = ["sh", "-c", "echo Creating schema && /usr/bin/db-migrate"]
oneshot = true
dir = "/opt/myapp"

[env]
DB_HOST = "localhost"
DB_USER = "admin"
```
