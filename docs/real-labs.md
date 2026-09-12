# Real practice environments

The trainer sends bounded requests to a separate runner on the Compose network. Every accepted command invokes the actual Docker or kubectl executable with explicit arguments. The runner never interprets learner input as a shell script. Applications in the fixtures use trusted, authored startup scripts.

The healthy environments teach commands before incidents. Kubernetes creates a Deployment, a client Pod, and a Service in `sre-practice`. Docker creates a web container and a client container on `sre-practice-net`, an internal bridge with no published ports. Only one environment is active at a time. Resource creation is explicit; reconnecting to an existing lab does not reapply its original fault. Use `/lab stop`, then explicitly start again, to reset it.

## Kubernetes setup

An administrator applies the dedicated namespace, quota, restricted Pod Security policy, service account, Role and binding:

```bash
kubectl apply -f deploy/lab-namespace.yaml
```

Supply a kubeconfig for that service account as `.credentials/config`, mode 0600 in a mode-0700 directory. The context should be named `sre-practice` with that namespace. Preserve your cluster's CA verification; the API endpoint must be reachable from the runner. The trainer itself never receives this file. The root runner retains only DAC_OVERRIDE beyond its dropped capabilities so it can read the privately owned mount; its Docker socket is already a privileged host capability. `.credentials/` is excluded from Git and Docker build contexts.

For a runner outside Kubernetes, this repository supplies `deploy/lab-token.yaml` as an explicit, manually managed service-account token Secret. Apply it only when choosing that credential lifecycle. The token is long-lived and confined by the namespaced Role. Rotate it by replacing the Secret and regenerating the private kubeconfig; never paste it into chat, terminal logs, source files, or image layers. Kubernetes recommends short-lived TokenRequest credentials where their renewal can be managed; an in-cluster runner can instead use a projected service-account token. The current Docker-hosted setup uses the manually managed credential so a prolonged shutdown does not strand the learner with an expired token. [Kubernetes service-account credentials](https://kubernetes.io/docs/tasks/configure-pod-container/configure-service-account/)

After privately generating the kubeconfig, verify both required access and denials:

```bash
kubectl --kubeconfig .credentials/config auth can-i get pods -n sre-practice
kubectl --kubeconfig .credentials/config auth can-i get secrets -n default
kubectl --kubeconfig .credentials/config auth can-i get nodes
```

Expected results are `yes`, `no`, `no`. The runner's normal command interface further restricts even namespaced access to authored commands. Kubernetes binaries are pinned to v1.31.14 to match the deployment cluster's minor version and checked against the upstream SHA-256 during build. Change the Docker build argument deliberately for another cluster version; the provided image build currently targets Linux amd64.

Set `SRE_LAB_NODE` in the private `.env` to constrain Kubernetes fixtures to a tested worker. This applies required node affinity to both web and client while leaving the Pending scenario's separate bad selector observable and repairable. The Nucbox deployment selects Undyinglands; Erebor exposed an existing runtime signal-permission failure during cleanup, so it is excluded from these labs.

Docker-only practice works without a Kubernetes kubeconfig. Reference lookup and local Linux teaching do not require the Kubernetes API.

## Boundaries and resource use

The runner has the Docker socket, which is privileged access to its host. It must remain a trusted local component: do not publish its HTTP port or attach untrusted containers to its Compose network. This is a personal learning installation, not a multi-user execution service. The default Compose file publishes no runner ports. The trainer contains only the runner's internal URL.

Learners cannot supply arbitrary manifests, images, shell commands, mount paths, namespaces, contexts or container names. Commands are checked as tokenized argument sequences against the active lab's allowlist. Docker cleanup selects only the trainer ownership label. Kubernetes cleanup is confined to the dedicated namespace and ownership label. A command has a 25-second process timeout and bounded returned output. Failures are visible and do not fabricate success.

Each authored web/client process is non-root with dropped capabilities and a read-only root. Docker fixtures use memory, CPU and PID limits and bounded temporary storage. Kubernetes uses restricted Pod Security, disabled service-account automount, resource requests/limits, a namespace quota, and bounded temporary volumes. Practice images are pinned to BusyBox 1.37.0; fixed version tags are used, not digest pinning.

The deployed cluster uses Flannel without a NetworkPolicy enforcement provider. Accordingly, **the Kubernetes practice namespace is an authorization and resource boundary, not an enforced network sandbox**. The runner accepts only requests to the fixed practice Service; arbitrary network targets and interactive shells are not exposed. Do not add arbitrary learner-authored code on the assumption that namespace isolation blocks egress. NetworkPolicy enforcement depends on a supporting network implementation. [Kubernetes NetworkPolicy prerequisites](https://kubernetes.io/docs/concepts/services-networking/network-policies/#prerequisites)

## Persistence and verification

The `lab-state` volume stores the runner's active fixture identity. The trainer's existing `sre-progress` volume stores the chat, active attempt, captured command evidence and archived scenario attempts. A 24-hour post-learning session boundary does not recreate resources or reset lesson progress. A long-running client pod/container eventually exits after seven days; inspect and explicitly recreate an expired lab using stop/start. Exiting the trainer leaves active resources available for resumption; stop a lab when finished to release them.

Run the real integration check only with no active learner incident:

```bash
docker compose run --rm -T trainer --check-labs
```

It creates each incident, checks a real failing client request, applies the scoped repair, checks the actual response, verifies that resuming does not reinject the fault, and cleans up. It also checks the two healthy teaching baselines and rejects a command targeting an unrelated container. It does not call an LLM or alter the learner's teaching database. A fixture mismatch fails instead of resetting an already active different lab. Tests cannot substitute for checking the actual deployment and its RBAC boundaries.
