# Installing Zinit

This guide provides detailed instructions for installing Zinit on various platforms.

## System Requirements

Zinit has minimal system requirements:

- Linux-based operating system
- Root access (for running as init system)

## Pre-built Binaries

Every pushed tag runs the release workflow and builds two Linux x86_64 variants:

| Rust target | Release binary | Container compatibility |
| --- | --- | --- |
| `x86_64-unknown-linux-musl` | `zinit-linux-x86_64-musl` | Statically linked; Alpine, Ubuntu, Debian, and other Linux bases |
| `x86_64-unknown-linux-gnu` | `zinit-linux-x86_64-gnu` | Built on Ubuntu 26.04; supported baseline glibc 2.43 |

Each binary has a matching `.tar.gz` bundle containing an executable named `zinit` and `LICENSE`. `SHA256SUMS` covers both standalone binaries and bundles. ARM and macOS releases are currently out of scope.

Use GitHub CLI to download a pinned version. Run `gh auth login` first, or set `GH_TOKEN` in CI; this also works if the repository is private.

```bash
# Use zinit-linux-x86_64-gnu.tar.gz for the glibc build.
gh release download vX.Y.Z --repo valarauca/zinit \
  --pattern zinit-linux-x86_64-musl.tar.gz --pattern SHA256SUMS
sha256sum --ignore-missing --check SHA256SUMS
tar -xzf zinit-linux-x86_64-musl.tar.gz
sudo install -m 0755 zinit /usr/local/bin/zinit
```

### Creating a release

Commit and push the workflow changes before creating a tag at the commit you want to release:

```bash
git tag v0.2.1
git push origin v0.2.1
```

All tags trigger a build. Semver tags with a prerelease suffix, such as `v0.2.1-rc.1`, create a GitHub prerelease. The workflow uses the repository's automatic `GITHUB_TOKEN`; no extra release secret is needed. Release automation builds and bundles the artifacts, generates checksums, and publishes once both builds finish. Tests remain in local development and the Rust workflow on branch pushes.

To build an existing tag manually once the workflow is on the default branch:

```bash
gh workflow run release.yaml --repo valarauca/zinit --ref v0.2.1
```

Manual runs must select a tag; branch runs skip release jobs. Failed builds do not publish a release. A failed upload leaves a new release as a draft, and rerunning the workflow retries its uploads.

## Building from Source

### Prerequisites

To build Zinit from source, you'll need:

- Rust toolchain (1.46.0 or later recommended)
- musl and musl-tools packages
- GNU Make

#### Install Rust

If you don't have Rust installed, use rustup:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

#### Install musl development tools

On Debian/Ubuntu:

```bash
sudo apt update
sudo apt install musl musl-tools
```

On Fedora:

```bash
sudo dnf install musl musl-devel
```

On Alpine Linux (musl is already the default libc):

```bash
apk add build-base
```

### Build Process

1. Clone the repository:

```bash
git clone git@github.com:valarauca/zinit.git
cd zinit
```

2. Build using make:

```bash
make
```

This will create a statically linked binary at `target/x86_64-unknown-linux-musl/release/zinit`.

3. Install the binary:

```bash
sudo cp target/x86_64-unknown-linux-musl/release/zinit /usr/local/bin/
```

### Development Build

For development or debugging:

```bash
make dev
```

## Docker Installation

### Using the Provided Dockerfile

Zinit includes a test Docker image:

```bash
# Build the Docker image
make docker

# Run the container
docker run -dt --device=/dev/kmsg:/dev/kmsg:rw zinit
```
> Don't forget to port-forward a port to get access to the Zinit proxy using the `-p XXXX:YYYY` flag when running the container.

### Adding a release binary to a worker image

Download and verify the binary before building the image. This keeps GitHub authentication in your local environment or CI job. From your worker image's build context:

```bash
version=vX.Y.Z
variant=musl # Use gnu for Ubuntu 26.04 / glibc 2.43 workers.
mkdir -p vendor/zinit
gh release download "$version" --repo valarauca/zinit --dir vendor/zinit \
  --pattern "zinit-linux-x86_64-$variant" --pattern SHA256SUMS
(cd vendor/zinit && sha256sum --ignore-missing --check SHA256SUMS)
install -m 0755 "vendor/zinit/zinit-linux-x86_64-$variant" vendor/zinit/zinit
```

Then add Zinit and your daemon configurations to the worker Dockerfile:

```dockerfile
FROM ubuntu:26.04
COPY --chmod=0755 vendor/zinit/zinit /usr/local/bin/zinit

RUN mkdir -p /etc/zinit
COPY services/*.toml /etc/zinit/
STOPSIGNAL SIGTERM
ENTRYPOINT ["/usr/local/bin/zinit", "init", "--container"]
```

Use your Runpod worker base image in place of `ubuntu:26.04`, and configure its daemons in `services/*.toml`. For example:

```toml
exec = ["python3", "-u", "/app/worker.py"]
log = "stdout"
shutdown_timeout = "30s"
```

Build the worker for x86_64:

```bash
docker buildx build --platform linux/amd64 --load -t my-worker:local .
```

Keep the container's stop grace period longer than the configured service shutdown timeouts so Zinit can finish stopping its daemons.

## Using Zinit as the Init System

To use Zinit as the init system (PID 1) on a Linux system:

### On a Standard Linux System

1. Install Zinit as described above
2. Create your service configurations in `/etc/zinit/`
3. Configure your bootloader to use zinit as init

For GRUB, add `init=/usr/local/bin/zinit` to the kernel command line:

```bash
# Edit GRUB configuration
sudo nano /etc/default/grub

# Add init parameter to GRUB_CMDLINE_LINUX
# Example:
# GRUB_CMDLINE_LINUX="init=/usr/local/bin/zinit"

# Update GRUB
sudo update-grub
```

### In a Container Environment

For containers, simply set Zinit as the entrypoint:

```bash
docker run -dt --device=/dev/kmsg:/dev/kmsg:rw \
  --entrypoint /usr/local/bin/zinit \
  your-image init --container
```

## First-time Setup

After installation, you'll need to create a basic configuration:

1. Create the configuration directory:

```bash
sudo mkdir -p /etc/zinit
```

2. Create a simple service configuration:

```bash
cat << EOF | sudo tee /etc/zinit/hello.toml
exec = ["echo", "Hello from Zinit!"]
oneshot = true
EOF
```

3. Test Zinit without running as init:

```bash
# For testing only - doesn't replace system init
sudo zinit init
```

If all is working correctly, you should see Zinit start and run your service.
