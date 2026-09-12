# From senior frontend engineer to senior SRE

Research checked 2026-09-12. This report is a curriculum recommendation, not a claim that the trainer already implements every exercise below. It uses primary documentation; the level structure, assessment rubric, and career milestones are synthesis for this learner.

The practical goal is to carry existing engineering judgment into production ownership while deliberately learning the systems knowledge and operational judgment that frontend work may not have required. Google's description of SRE explicitly combines software engineering with responsibilities for availability, latency, performance, efficiency, change management, monitoring, emergency response, and capacity. That is a broader destination than becoming comfortable with Kubernetes commands. [Google SRE introduction](https://sre.google/sre-book/introduction/)

## What transfers, and what needs new evidence

A senior frontend background is a useful starting point, but it does not establish a person's Linux, networking, distributed systems, or incident-management proficiency. Treat the strengths below as things to confirm from past work, rather than assuming every frontend engineer has them. Likewise, do not restart programming fundamentals merely because the learner is new to `du`.

The following matrix is a recommended assessment plan. The final column describes evidence to collect, not a universal hiring standard.

| Capability | Potential frontend transfer | New SRE depth to develop | Evidence of progress |
| --- | --- | --- | --- |
| User impact | User journeys, performance budgets, product priorities | Availability and latency SLIs; SLO windows; error-budget decisions | Define a meaningful checkout SLO and justify the measurement boundary |
| Debugging | Reproduction, DevTools, tracing state changes | Hypotheses across processes, network hops, dependencies, and infrastructure | Explain which observation rejects each alternative cause |
| Software engineering | Testing, review, refactoring, API design | Safe automation, idempotence, cancellation, retries, partial failure | Build and test a recovery tool that handles repeated invocation |
| Linux | CLI and development environments | Processes, signals, permissions, file descriptors, filesystems, memory, CPU and I/O | Diagnose a service failure without guessing or indiscriminate restarts |
| Networking | HTTP, browser caching, CDN behavior | Addressing, routes, DNS, TCP, TLS, listeners, proxies, connection pools | Identify the failing layer in an actual request path |
| Containers | Local Docker use, reproducible builds | Process lifecycle, image configuration, ports, networks, volumes, limits | Restore a failing container and prove the client request works |
| Kubernetes | Declarative configuration, CI familiarity | Reconciliation, scheduling, readiness, discovery, configuration, storage, RBAC | Fix a scoped workload and explain desired versus observed state |
| Observability | Client errors and performance instrumentation | Metrics, traces and logs across services; cardinality; sampling; actionable alerts | Follow a slow request and identify useful versus misleading telemetry |
| Delivery | CI, feature flags, incremental releases | Canary analysis, rollback boundaries, migrations, drift, artifact provenance | Stop or revert a harmful release using measured evidence |
| Distributed systems and data | Async code, caches, API contracts | Timeouts, retry amplification, idempotency, queues, consistency, restore semantics | Handle an unavailable dependency without cascading failure or duplicate effects |
| Infrastructure and cloud | Environment configuration | IAM, failure domains, quotas, capacity, storage, IaC, recovery and cost | Rebuild a disposable environment and explain its remaining failure modes |
| Leadership | Mentoring, design reviews, stakeholder alignment | Incident command, escalation, handoffs, reliability prioritization | Coordinate a drill, document uncertainty, and drive a prevention change |

Three sources anchor this breadth. OpenTelemetry treats traces, metrics, and logs as different signals. Google's large-system design material combines concrete capacity estimates with isolation and degradation. AWS reliability guidance calls for exercising failures and testing performance and recovery assumptions. [OpenTelemetry signals](https://opentelemetry.io/docs/concepts/signals/), [Google system design](https://sre.google/workbook/non-abstract-design/), [AWS reliability testing](https://docs.aws.amazon.com/wellarchitected/latest/reliability-pillar/test-reliability.html)

## Four cumulative learning levels

Use levels as prerequisite groups, not titles. Completing Level 4 in a trainer should never display “you are now a senior SRE.” Google's onboarding guidance favors orderly cumulative learning, early practical work, increasing ownership, and shadow on-call after system fundamentals. It also notes that exercises cannot fully substitute for real operational experience. [Google SRE onboarding](https://sre.google/sre-book/accelerating-sre-on-call/)

| Level | Topics to introduce one at a time | Exit evidence before unlocked scenarios |
| --- | --- | --- |
| 1 — Inspect a system | Shell arguments and paths; `pwd`, `ls`, `cat`, `du`, `df`; `grep`, `tail`, `jq`; exit codes; permissions; `ps`; `ip address`, `ip route`; basic `curl`; Docker inventory; Kubernetes context, namespace, pods and logs | Run a bounded command, identify what it inspected, read its output, and explain one useful next step |
| 2 — Operate a service | Processes and signals; disk versus inode usage; listeners and DNS; Docker lifecycle, inspect, logs, bridges and ports; Kubernetes labels, Services, EndpointSlices, probes, events, rollouts, environment and requests/limits | Diagnose and repair a single known class of fault, then verify the request from a client |
| 3 — Diagnose interacting failures | Timeout versus refusal versus DNS failure; retries and idempotency; CPU/memory/I/O saturation; queues and caches; deployment changes; metrics, traces and logs; SLOs and alert quality; RBAC; storage and restores | Resolve a variation without a prescribed command sequence, use evidence, and preserve data and isolation |
| 4 — Own reliability | Error budgets; release tradeoffs; load and capacity; failure domains; recovery objectives; IaC changes; incident roles and communication; postmortems; toil reduction; security and cost tradeoffs | Produce a verified recovery, a clear incident account, and a justified improvement plan; defend design tradeoffs |

Keep networking and Linux available throughout the Kubernetes curriculum. A service-discovery problem should connect back to DNS and listeners, rather than becoming a standalone recipe of `kubectl` flags. As an extension beyond these common foundations, let the learner choose a production domain: frontend delivery/CDN, backend services, platform engineering, data systems, or infrastructure.

## Teaching behavior for this trainer

These are product recommendations based on the user's stated preferences, not claims about an optimal universal learning schedule.

1. Resume the exact unfinished topic, step, transcript, draft, and live lab after restart. Choose randomly among the easiest eligible topics only when there is no active unfinished topic or the learner explicitly requests a new one.
2. Introduce what the command observes and why an SRE would use it. Show a small real example, then ask the learner to type it. Add one new argument or concept per step.
3. Inspect the actual exit code, stdout and stderr. Ask about something visible in that output. If a command fails, teach that failure before advancing.
4. Offer bounded experiments and answer questions. A typo should not erase progress; needing a hint should not become a punitive failure.
5. Graduate a topic after demonstrated practice and a meaningful explanation or transfer exercise. Add the learned skill to the common spaced-repetition pool.
6. Give due skills and their evidence to the teaching LLM. It should compose a focused recall or application prompt rather than ask “did you get it?” An unfamiliar skill receives instruction before assessment.
7. Start a new post-learning session only when at least 24 hours have elapsed and the learner rejoins or sends a message. That boundary must not reset the ordinary teaching conversation or an unfinished incident.
8. Unlock a scenario only when its actual prerequisite skills are learned. Track skill retention separately from scenario attempts and from production-readiness claims.

For the current `du` lesson, a reasonable sequence is:

```sh
du .
du -h .
du -sh .
du -h --max-depth=1 .
du -ah .
```

Present these separately. First establish that `.` means the current fixture directory. Then teach readable units, a summary, depth, and inclusion of individual files. GNU documents these options and distinguishes allocated usage from apparent file size; that distinction belongs in a later lesson, not in the first explanation. [GNU `du` manual](https://www.gnu.org/software/coreutils/manual/html_node/du-invocation.html)

For networking, teach the actual command `ip address` (commonly shortened to `ip addr`), followed by `ip -br address`, then `ip route`. Explain interface state, addresses and routes using what that lab actually shows. The manual documents `ip`'s object-and-command syntax and its address, link and route objects. [iproute2 manual](https://man7.org/linux/man-pages/man8/ip.8.html)

## Real scenario backlog and prerequisite gates

The following are proposed authored fixtures. All commands should execute against actual disposable files, processes, Docker containers or Kubernetes resources. An LLM must never fabricate command output. The exercise may stage an artificial incident, but the process state, network response and repair must be real.

Use a healthy baseline to teach each command before introducing the corresponding failure. The earliest incident should combine only two or three familiar skills; introduce multi-fault cases later.

| Scenario | Learned prerequisites | Real fixture and fault | Independent completion evidence |
| --- | --- | --- | --- |
| Disk investigation | `du`, paths, readable units; later `df` and inodes | A bounded directory tree containing a large log and many small files | Correct culprit and explanation of what the measurement establishes; no filling the host disk |
| Stopped Docker service | `docker ps`, logs, lifecycle, HTTP | Web container intentionally stopped on a private bridge | Container running plus a successful HTTP request from the client container |
| Wrong Docker network | Inspect, bridges, DNS, HTTP | Live client and web containers on different exercise networks | Correct membership restored and service-name request succeeds |
| Wrong listening port | Ports, listeners, inspect, HTTP errors | Server listens on a port different from the client's assumption | Correct endpoint tested and explanation of container versus published ports |
| Empty Kubernetes Service | Pods, labels, Service selectors, EndpointSlices | Healthy web deployment with a mismatched Service selector | Matching ready endpoint and successful request through the Service |
| Running but unready | HTTP status, probes, describe/events, rollout | Readiness probe points at a nonexistent path | Intended probe repaired, workload ready, and client request succeeds |
| Crash loop from configuration | Logs, previous logs, environment, deployments | Application exits because required configuration is absent | Correct configuration, stable restarted workload, and successful request |
| Pending workload | Scheduling, node selectors, requests and events | Impossible selector inside a tightly bounded lab | Intended workload schedules and serves; explanation cites scheduling evidence |
| Bad release | Rollout history, probes, logs, client checks | A small deployment receives a deliberately broken revision | Recovery through a justified rollback or fix, plus user-facing verification |
| Slow dependency | Traces, latency, timeouts, retries | Two services with bounded injected latency and a request generator | Reduced impact and a trace-backed explanation; no uncontrolled load generation |
| Restore after data loss | Volumes, backup, recovery objectives, checksums | Disposable data store and an independently held backup | Restored records verified, measured recovery time, and stated data-loss boundary |
| Incident coordination | SLOs, evidence, mitigation, status updates | A familiar technical fault with a changing impact brief | Verified recovery plus concise status updates, handoff, and follow-up ownership |

Kubernetes' Service debugging guide explicitly checks selectors, EndpointSlices and direct backend requests. Probe documentation explains why a running container can remain unready for traffic. Docker documents name resolution on user-defined bridges. These make good scenarios because the learner can observe concrete state changes and distinguish competing causes. [Kubernetes Service debugging](https://kubernetes.io/docs/tasks/debug/debug-application/debug-service/), [Kubernetes probes](https://kubernetes.io/docs/concepts/workloads/pods/probes/), [Docker bridge networks](https://docs.docker.com/engine/network/drivers/bridge/)

Recommended lab controls: scope credentials and mutations to the exercise; constrain resources and targets; keep provider secrets outside execution environments; preserve fixture identity across reconnects; make reset and cleanup explicit. A namespace alone should not be described as network isolation. Kubernetes states that NetworkPolicy objects have no enforcement effect without a supporting network implementation. Test actual reachability boundaries before advertising an isolated network exercise. [Kubernetes NetworkPolicy prerequisites](https://kubernetes.io/docs/concepts/services-networking/network-policies/#prerequisites)

## Assess reasoning, recovery and communication

Use the following recommended rubric independently for each dimension: **introduced → guided → independent → retained**. The learner can be independent in command use but still need guidance interpreting output. Avoid a single opaque percentage.

| Dimension | Evidence the trainer should collect |
| --- | --- |
| Execution | Actual command, target, timestamp, exit status and relevant output |
| Diagnosis | A supported hypothesis, a useful discriminating check, and interpretation of evidence |
| Recovery | A proportionate change and an independent check of the user-visible objective |
| Communication | Impact, known facts, uncertainty, mitigation, next action and verification |

Google's troubleshooting guidance describes an iterative process of hypotheses and confirming or disconfirming observations. It also separates immediate corrective action from completing causal analysis. Consequently, score the learner's investigation and safe recovery, not whether they reproduce a hidden ideal command sequence. [Google effective troubleshooting](https://sre.google/sre-book/effective-troubleshooting/)

Use deterministic checks for resource state and actual requests; use the LLM to evaluate the explanation against captured evidence and ask a targeted follow-up when ambiguous. A convincing paragraph must not override a failing live check. Conversely, restoring service by chance should trigger a short explanation step, not automatic mastery. Ask the learner to solve a changed version later, rather than repeating identical names and answers.

Senior-oriented exercises should include decisions such as “roll back now or investigate further?”, “is this page actionable?”, and “which reliability investment should we fund?” SLOs and error budgets support prioritizing reliability work, while SLO alerting considers significant budget consumption and tradeoffs between detection quality and speed. [Google implementing SLOs](https://sre.google/workbook/implementing-slos/), [Google alerting on SLOs](https://sre.google/workbook/alerting-on-slos/)

During incident drills, ask for short updates that distinguish observations from hypotheses and give the next action without inventing an ETA. Teach escalation and delegation before expecting incident leadership. Google's incident guidance separates command, operations and communications roles and favors mitigation first; its postmortem guidance emphasizes useful learning and acted-upon improvements. [Google incident response](https://sre.google/workbook/incident-response/), [Google postmortem culture](https://sre.google/workbook/postmortem-culture/)

## Transition milestones that can be demonstrated

These are suggested milestones, not calendar promises or evidence that a particular employer will assign a senior title. The time needed depends on existing systems experience, access to production ownership, mentorship, and the role's scope.

1. **Diagnose a learning environment independently.** Explain the path from client to service, operate the basic tools, and recover several unfamiliar variations of small faults.
2. **Own a small service with support.** Deploy it reproducibly; instrument it; define useful reliability targets; document recovery and escalation; practice restore and rollback.
3. **Join supervised operational work.** Shadow an experienced responder, handle bounded incidents with review, improve a runbook, and complete a prevention change. Production readiness is agreed with the responsible team.
4. **Deliver a reliability project with measured impact.** Examples: reduce noisy pages, make rollbacks safer, remove a repeated manual recovery, or verify restores. Include baseline, result, maintenance cost and limitations. Google's toil definition concerns recurring operational work without enduring value; automation should be judged by the operational burden it removes. [Google eliminating toil](https://sre.google/sre-book/eliminating-toil/)
5. **Demonstrate senior scope over time.** Lead a cross-team reliability decision, mentor responders, evaluate capacity and failure domains, and follow incident learning through to shipped improvements. Gather reviewable artifacts and feedback from people accountable for the service.

A strong bridge project for this background is an existing frontend plus a small API: trace one user journey through CDN/proxy, application, cache and data store; measure its success and latency; introduce bounded faults; restore it; and improve the design. It uses familiar product context while making the missing systems layers visible. Keep coding and design strengths active throughout the transition, and use tool practice to build the evidence required for that wider ownership.
