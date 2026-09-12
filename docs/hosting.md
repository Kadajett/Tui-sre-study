# Hosting options

SRE Trainer runs in a terminal. Choose where that terminal process and its persistent data should live; a web server, public domain, Tailscale account, and Kubernetes cluster are not required for basic teaching.

The supplied full-lab runner targets Linux amd64 and needs a Docker daemon with the Unix socket at `/var/run/docker.sock`. For another client platform, an SSH connection to a compatible Linux host provides the same TUI. The trainer-only image does not contain the runner's architecture-specific kubectl download. No managed hosting service or Kubernetes deployment of the trainer is included.

## Local Docker Compose: full labs

Install Docker Engine and the Compose plugin, then:

```bash
git clone https://github.com/Kadajett/Tui-sre-study.git
cd Tui-sre-study
cp .env.example .env
mkdir -p .credentials
# Edit .env and add your own OpenRouter key.
docker compose build
docker compose run --rm trainer
```

Compose starts the separate lab runner. `/lab docker` creates real practice containers. Kubernetes exercises become available after you supply the namespace-scoped credentials described in [real-lab setup](real-labs.md). The empty `.credentials` directory is sufficient for Docker-only practice.

The runner is a trusted component with Docker socket access. Keep its HTTP port private; the supplied Compose file publishes no ports. This is a single-learner deployment. Use one active trainer per progress volume and one active lab per runner. Provision separate installations for independent learners rather than sharing this runner as a public execution service.

## Remote Linux server or VM

Use a Linux machine with Docker Engine, Compose, and an account permitted to use Docker. Connect from your computer:

```bash
ssh your-user@your-server
```

Then follow the local Compose setup above **on that server**. On subsequent visits:

```bash
ssh your-user@your-server
cd ~/Tui-sre-study
docker compose run --rm trainer
```

Replace the account, host, and checkout path with your own. If you already use Tailscale SSH, `tailscale ssh your-user@your-machine` can replace the SSH command; configure and authorize that access separately. The app has no dependency on a particular tailnet or machine name.

The TUI runs on the server, so its progress volumes and Docker fixtures also live there. Kubernetes fixtures live in whichever cluster its scoped kubeconfig selects. Closing the trainer saves the conversation; reconnecting and launching it again resumes that stored state.

## Teaching only: no lab runner

To use conversation, reinforcement, and the local Linux command courses without starting the privileged runner:

```bash
cp .env.example .env
# Add your OpenRouter key if you want the conversational teacher.
docker compose build trainer
docker compose run --rm --no-deps -e SRE_LAB_URL= trainer
```

`--no-deps` skips starting the dependent runner, and the empty lab URL disables access for this trainer invocation. Docker/Kubernetes incident execution is unavailable in this mode; the six local Linux command courses still execute inside the trainer container. Named progress storage is the same as the full Compose mode, so you can enable the runner later without losing your teaching position. [Compose run options](https://docs.docker.com/reference/cli/docker/compose/run/)

## Run from source on Linux

For development, install Rust and the GNU/Linux commands listed in the Dockerfile's runtime package installation, including `jq`, `ip`, and `ps`. From the repository root:

```bash
cargo build --release --locked
./target/release/sre-trainer
```

Export `OPENROUTER_API_KEY` and any optional reference URLs through your shell or secret manager before launch. The native binary does **not** load `.env` automatically. Without a key, built-in teaching material and local command execution remain available, while conversational assessment waits for the model connection.

Native progress defaults to `.sre-trainer/progress.db`; `SRE_DATA_DIR` changes its directory. `SRE_LESSONS` can point to a different lesson catalog. A native process has your host account's permissions, so use the container setup for the standard practice boundary. Real Docker/Kubernetes incidents require the separate runner and a reachable `SRE_LAB_URL`; native execution alone does not provision it.

## Add a Kubernetes cluster as a lab target

The trainer and Docker runner can stay on a laptop or remote server while Kubernetes fixtures run in a separate local, homelab, or managed cluster:

1. Apply the namespace and RBAC manifests using an administrator context, following [real-lab setup](real-labs.md).
2. Put the runner's scoped kubeconfig in `.credentials/config`. Its API endpoint must be reachable from the **runner container**, with the cluster CA correctly configured. A kubeconfig pointing at `127.0.0.1` refers to the runner itself, not your laptop's API server.
3. Build the runner with a kubectl version appropriate for your cluster. The example defaults to v1.31.14; pass a different `KUBECTL_VERSION` build argument when needed.
4. Optionally set `SRE_LAB_NODE` to a worker hostname. Leave it blank for normal scheduling.
5. Start the Compose runner and use `/lab kubernetes` from the TUI.

A containerd-only Kubernetes node does not supply the Docker daemon this runner needs. The supplied manifests provision **practice resources and permissions**, not a deployment of the trainer or runner inside Kubernetes.

## Optional DevDocs and SearXNG

These integrations are disabled by default. Configure your own compatible services in `.env`:

```dotenv
DEVDOCS_URL=https://docs.example.com
SEARXNG_URL=https://search.example.com
```

The example domains are placeholders. Use URLs reachable from the trainer container; a private network or tailnet is fine. HTTPS is suitable across untrusted networks, and HTTP is accepted for an explicitly configured trusted local service such as `http://searxng:8080`. URL credentials, query strings, and fragments are rejected. The client keeps certificate verification enabled and does not follow redirects.

The DevDocs integration expects `/assets/docs.js`, `/docs/<collection>/index.json`, and documentation HTML under `/docs/<collection>/`. SearXNG must allow JSON responses at `/search?format=json`. These services are not included in Compose. Blank URLs remove their tools from the model request; no lookup is sent to a maintainer's infrastructure.

## Updates and moving hosts

Exit the trainer before upgrading. For a full Compose installation, run from the same checkout and Compose project:

```bash
git pull --ff-only
docker compose build
docker compose up -d --no-build lab-runner
docker compose run --rm trainer
```

For teaching-only installations, rebuild `trainer` and repeat the teaching-only launch instead. Rebuilding or replacing a container does not remove its named volumes. Keep the same Compose project name/directory to select the same volumes; otherwise, a new project can appear to have empty progress. The `sre-progress` volume holds teaching/chat data and `lab-state` holds runner state. Avoid `docker compose down -v` unless deletion is intentional. [Docker volume persistence](https://docs.docker.com/engine/storage/volumes/)

Before moving hosts, stop any active lab explicitly with `/lab stop`, exit the trainer, and back up the progress volume. Restore it on the destination under the destination Compose project's progress volume name. Copy `.env` and scoped credentials separately through a private channel, then rebuild or transfer platform-compatible images. Git clones contain neither learner progress nor private configuration. A saved lab identity cannot move the Docker containers that exist on the old host.

If the server has little build space, build both Dockerfile targets on another machine for the server's platform and transfer them with `docker save` / `docker load`. Use the image names shown by `docker compose config --images` on the destination; loading a differently named image does not satisfy Compose's expected image name. Then use `docker compose run --rm trainer` after starting the runner with `docker compose up -d --no-build lab-runner`.
