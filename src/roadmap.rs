pub const ROADMAP: &str = "SWE → SRE: work from symptoms to evidence, then choose the smallest useful action.

1. Linux investigation — du (guided now), df and inodes, files and permissions, processes, memory, I/O, logs.
2. Networking — DNS → routing → TCP → TLS → HTTP; timeouts, retries, connection pools.
3. Kubernetes — workload state, events, probes, requests/limits, Services, storage, rollouts.
4. Observability — metrics vs logs vs traces, RED/USE, tail latency, useful dashboards and cardinality.
5. Reliability — user journeys, SLIs/SLOs, error budgets, burn-rate alerts, toil.
6. Incidents and on-call — triage, mitigate, communicate, verify recovery, learn without blame.
7. Safe delivery — canaries, rollback triggers, migrations, feature flags, blast radius.
8. Distributed systems — overload, retries with jitter, backpressure, idempotency, queues, failure domains.
9. Capacity and data — load tests, saturation, scaling, backup restores, RPO/RTO.
10. Cloud and infrastructure — IAM, networking, Terraform state/drift, cost and resource ownership.

Ask, for example: /ask How does an SRE investigate rising p99 latency when CPU is normal?
Or: /ask Use DevDocs to explain Bash exit status, with an example.
Or: /ask Search current Kubernetes docs for how to troubleshoot Pending pods.

Mercury can search your DevDocs and SearXNG, and shows the sources it consulted. The default teaching interface has four learning levels, six executable beginner command courses, and a Kubernetes sample-output walkthrough. Use /levels to see progress; learned topics share one conversational reinforcement pool. From your shell, use docker compose run --rm trainer --deck sre (or networking, kubernetes, observability, delivery, distributed, data, cloud, iac, containers).";
