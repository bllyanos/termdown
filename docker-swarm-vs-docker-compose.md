# Docker Swarm vs plain Docker Compose

## Research question

What changes in day-to-day deployment, operations, networking, scaling, updates, storage, and security when moving from plain `docker compose` to Docker Swarm mode? The comparison treats “plain Compose” as `docker compose up` against one Docker Engine, and “Docker Swarm” as Swarm mode plus `docker stack deploy`.

## Executive summary

- **Compose is a single-host application lifecycle tool.** It defines services, networks, and volumes in YAML and starts them on the Docker Engine selected by the client. Docker’s production guidance describes the simplest deployment as a single server; a remote `DOCKER_HOST` still points Compose at one Docker host. [Docker Compose](https://docs.docker.com/compose/), [Use Compose in production](https://docs.docker.com/compose/how-tos/production/)
- **Swarm is a cluster orchestrator built into Docker Engine.** Managers maintain desired state, schedule service tasks across managers/workers, replace failed tasks, support replicated/global services, and provide multi-host networking and routing. [Swarm mode](https://docs.docker.com/engine/swarm/), [How services work](https://docs.docker.com/engine/swarm/how-swarm-mode-works/services/)
- **The operational boundary changes.** Compose requires one host and its local storage/networking. Swarm requires cluster membership, manager quorum, node-to-node firewall rules, image distribution through a registry, and a plan for placement and storage. [Deploy a stack to a swarm](https://docs.docker.com/engine/swarm/stack-deploy/), [Administer and maintain a swarm](https://docs.docker.com/engine/swarm/admin_guide/)
- **Swarm is not simply “Compose with replicas.”** The same YAML can be used as input, but `docker compose` and `docker stack deploy` are different runtimes with different supported file features. Current Docker documentation says `docker stack deploy` uses the legacy Compose file version 3 format and is not compatible with the latest Compose Specification. [Deploy a stack to a swarm](https://docs.docker.com/engine/swarm/stack-deploy/)
- **Recommendation:** use Compose for local development, CI, demos, and production workloads that intentionally fit on one host. Choose Swarm only when the concrete requirement is multi-node scheduling, service rescheduling, cluster-wide service discovery, routing, or controlled rolling updates—and accept the added cluster and storage operations. Docker itself says to use Compose when not planning to deploy with Swarm. [Swarm mode](https://docs.docker.com/engine/swarm/)

## Findings

### 1. The command model is different

| Task | Plain Compose | Swarm |
|---|---|---|
| Start a stack | `docker compose up -d` | `docker stack deploy -c compose.yaml myapp` |
| Inspect workload | `docker compose ps`, `docker compose logs` | `docker stack services myapp`, `docker service ps myapp_web`, `docker service logs` |
| Scale a service | More containers on the current Engine | More service tasks scheduled across eligible swarm nodes |
| Stop/remove | `docker compose down` | `docker stack rm myapp` |
| Image build | Compose can build locally as part of `up` | Build first, push images to a registry, then deploy; `docker stack deploy` ignores unsupported options such as `build` |

Compose creates and manages containers for the project. Swarm accepts a service definition as desired state and creates tasks; each task invokes one container. If a task fails, the orchestrator creates a replacement task to restore the requested replica count. [Docker Compose](https://docs.docker.com/compose/), [How services work](https://docs.docker.com/engine/swarm/how-swarm-mode-works/services/), [Deploy a stack to a swarm](https://docs.docker.com/engine/swarm/stack-deploy/)

**Practical consequence:** with Compose, “the machine running this command” is the deployment target. With Swarm, a manager is the control point and the scheduler chooses a node. A service can remain `pending` when no node satisfies its resource or placement requirements instead of failing immediately. [How services work](https://docs.docker.com/engine/swarm/how-swarm-mode-works/services/)

### 2. Failure handling and availability

Compose can apply container restart policies, but it does not turn several independent Docker hosts into one scheduling domain. Docker’s production guidance recommends a restart policy such as `restart: always` for a single-server deployment and documents recreating a changed service with `docker compose up`. [Use Compose in production](https://docs.docker.com/compose/how-tos/production/)

Swarm continuously reconciles actual state with desired state. For example, if a service requests ten replicas and a node hosting two fails, the manager creates replacement replicas on available nodes. Swarm supports both:

- **Replicated services:** a specified number of tasks.
- **Global services:** one task on every eligible node, useful for agents such as monitoring or antivirus services.

[Swarm mode](https://docs.docker.com/engine/swarm/), [How services work](https://docs.docker.com/engine/swarm/how-swarm-mode-works/services/)

This is availability for stateless or replication-aware services, not automatic data replication. **[Inference]** A replicated database container does not become a safe database cluster merely because Swarm runs multiple replicas; database replication, failover semantics, backups, and storage placement still belong to the database and storage design.

### 3. Cluster management is a real new dependency

A Swarm has manager and worker nodes. Managers store cluster state using Raft and require a majority quorum for management operations. Docker’s guidance recommends an odd number of managers: three managers tolerate one manager failure, and five tolerate two. If quorum is lost, already-running tasks continue, but nodes and services cannot be added, updated, removed, started, stopped, moved, or updated. [Administer and maintain a swarm](https://docs.docker.com/engine/swarm/admin_guide/)

This creates work that plain Compose does not have:

- Provision and join nodes; choose manager placement and roles.
- Keep manager IPs stable and distribute managers across failure domains.
- Open and monitor Swarm ports: TCP/UDP 7946 for discovery and UDP 4789 for overlay data path. [Manage swarm service networks](https://docs.docker.com/engine/swarm/networking/)
- Back up `/var/lib/docker/swarm/` and protect the unlock/encryption keys. [Administer and maintain a swarm](https://docs.docker.com/engine/swarm/admin_guide/)
- Maintain an image registry reachable by every node. [Deploy a stack to a swarm](https://docs.docker.com/engine/swarm/stack-deploy/)

A single-node Swarm is useful for learning or for accessing Swarm APIs, but it provides no node-failure tolerance. **[Inference]** For a small deployment that will remain on one machine, enabling Swarm usually adds operational surface without delivering a practical availability benefit.

### 4. Networking changes from local bridge networks to cluster networking

Plain Compose creates a project default network using Docker’s `bridge` driver. Services discover one another by service name, but the network and container IPs are local to that Docker Engine. Compose can also attach projects to an externally created network on that host. [Networking in Compose](https://docs.docker.com/compose/how-tos/networking/)

Swarm uses overlay networks with swarm scope for service-to-service communication across nodes. It also creates:

- An **ingress overlay network** for published service ports and routing-mesh load balancing.
- A **`docker_gwbridge`** network connecting overlay networks to each daemon’s physical network.

When a published port receives traffic at any swarm node, Swarm’s IPVS routing sends it to a task participating in the service. That means a client can reach a published service through a node that is not hosting the selected task. [Manage swarm service networks](https://docs.docker.com/engine/swarm/networking/), [Deploy a stack to a swarm](https://docs.docker.com/engine/swarm/stack-deploy/)

Security nuance: Swarm control and management traffic is always encrypted, but application traffic on an overlay network is not encrypted by default. Overlay encryption can be enabled, with a documented performance cost. [Manage swarm service networks](https://docs.docker.com/engine/swarm/networking/)

### 5. Scaling is distributed scheduling, not just more containers

Compose can run multiple instances on one Engine, but the host remains the capacity and failure boundary. Published host ports and local CPU, memory, and storage remain constraints. **[Inference]** Scaling a service with a fixed host-port mapping commonly requires an appropriate port design because multiple containers cannot all claim the same host socket.

Swarm’s service model includes replica count, CPU/memory limits and reservations, placement constraints/preferences, endpoint mode, and global versus replicated operation. Managers schedule tasks on nodes that meet the declared requirements. [How services work](https://docs.docker.com/engine/swarm/how-swarm-mode-works/services/), [Compose Deploy Specification](https://compose-spec.github.io/compose-spec/deploy.html)

Example intent:

```yaml
deploy:
  mode: replicated
  replicas: 3
  resources:
    limits:
      cpus: "0.50"
      memory: 512M
  placement:
    constraints:
      - node.labels.zone == east
```

The key practical distinction is that Swarm can move those tasks to other eligible nodes when capacity or health changes; Compose cannot schedule across independent hosts.

### 6. Updates and rollbacks are first-class in Swarm

For a single-host Compose deployment, the normal documented update flow is to rebuild an image and recreate the service, for example `docker compose build web` followed by `docker compose up --no-deps -d web`. [Use Compose in production](https://docs.docker.com/compose/how-tos/production/)

Swarm services support rolling-update and rollback policies. The deployment model can specify update parallelism, delay, monitoring, failure action (`continue`, `rollback`, or `pause`), failure ratio, and whether to stop the old task before starting the new one. Rollback has corresponding controls. [Compose Deploy Specification](https://compose-spec.github.io/compose-spec/deploy.html), [Swarm mode](https://docs.docker.com/engine/swarm/)

**Practical consequence:** Swarm gives a built-in mechanism for gradually replacing replicas and pausing or rolling back a bad service update. Compose deployments generally need external sequencing, a reverse proxy/load balancer, or an application-specific deployment tool to achieve the same release behavior on one host.

### 7. Images and builds behave differently

Compose is convenient when source code and a Dockerfile are local: `docker compose up` can build the image and start the containers on that Engine. [Deploy a stack to a swarm](https://docs.docker.com/engine/swarm/stack-deploy/)

A multi-node Swarm cannot assume that a locally built image exists on every node. Docker’s stack deployment guide explicitly uses a registry, tags the application image with the registry address, pushes it, and then deploys the stack. The guide also shows `docker stack deploy` ignoring the unsupported `build` option. [Deploy a stack to a swarm](https://docs.docker.com/engine/swarm/stack-deploy/)

**Practical workflow:**

```console
# Compose on one host
docker compose build
docker compose up -d

# Swarm
docker compose build
docker compose push
docker stack deploy --compose-file compose.yaml myapp
```

For predictable rollouts, use immutable image tags or digests rather than a frequently changing `latest` tag. Docker’s service documentation explains that Swarm resolves service image tags to digests so workers use the resolved image version. [Deploy services to a swarm](https://docs.docker.com/engine/swarm/services/)

### 8. Secrets and configuration are more centralized in Swarm

Docker Swarm secrets are cluster-managed data for service tasks. Docker documents them as encrypted in transit and at rest, access-controlled to explicitly authorized services, and mounted only while the task runs. Docker secrets are available to Swarm services, not standalone containers. [Manage sensitive data with Docker secrets](https://docs.docker.com/engine/swarm/secrets/)

Compose files can describe secrets, and both `docker-compose` and `docker stack` support defining secrets in a Compose file, but the runtime semantics are not identical. Do not assume that a Compose file’s secret declaration provides the same cluster-level storage and distribution guarantees when used with plain `docker compose`. [Manage sensitive data with Docker secrets](https://docs.docker.com/engine/swarm/secrets/)

### 9. Storage is the main stateful-workload trap

Compose’s default mental model is local: a volume or bind mount exists on the host where the containers run. In Swarm, a task may be rescheduled to another node. Docker documents that:

- A local data volume is created on the particular host where a task is scheduled if it does not already exist there.
- A bind-mounted host path must exist on every node where the task might run.
- Bind mounts are non-portable and can cause problems when tasks move between nodes.

[Deploy services to a swarm](https://docs.docker.com/engine/swarm/services/)

**Practical consequence:** a local Swarm volume is not automatically shared across nodes. **[Inference]** For databases, uploads, queues, and other stateful services, use an explicit distributed storage design, placement constraints, or an external managed service; otherwise a rescheduled task may see an empty volume or unavailable path.

### 10. Compose-file compatibility is a migration risk

`docker stack deploy` consumes a Compose file, but it is not the same parser or feature set as the current Compose CLI. Docker’s current stack documentation states that the command uses the legacy Compose file version 3 format and that the latest Compose Specification is not compatible. The CLI reference also shows unsupported options being ignored. [Deploy a stack to a swarm](https://docs.docker.com/engine/swarm/stack-deploy/), [docker stack deploy](https://docs.docker.com/reference/cli/docker/stack/deploy/)

The Compose Deploy Specification includes fields such as replicas, placement, resources, restart policy, update policy, rollback policy, and endpoint mode. It is an optional part of the Compose Specification, and support is platform-specific. [Compose Deploy Specification](https://compose-spec.github.io/compose-spec/deploy.html)

Before migrating, validate the exact file with the target command and inspect the resulting services. In particular, check `build`, `depends_on`, `links`, host-mode networking, bind mounts, profiles, and any newer Compose-only keys instead of assuming that `docker stack deploy` will honor them.

## Tradeoffs and open questions

### When Swarm is worth the complexity

Swarm is a reasonable fit when all or most of these are true:

- The application must span multiple Docker hosts.
- Replica placement and automatic rescheduling matter.
- Built-in overlay networking and routing-mesh ingress are useful.
- Rolling updates and rollback policies should be part of the service definition.
- The team is prepared to operate manager quorum, backups, firewall rules, registries, and storage.
- Docker-native concepts and CLI ergonomics are preferred over adopting a larger platform.

### When Compose is the better practical choice

Compose is usually the better fit when:

- One server has enough capacity and is an acceptable failure boundary.
- The main goal is local development, testing, CI, demos, or a small single-host production deployment.
- The workload uses local volumes or host bind mounts intentionally.
- The team wants a simple build/run/log/recreate workflow rather than cluster operations.
- Multi-host failover, distributed service discovery, or rolling orchestration is not a requirement.

### Unresolved or version-sensitive points

- `docker stack deploy` compatibility is explicitly tied to legacy Compose file version 3 behavior, while the Compose Specification continues to evolve. Revalidate files against the Docker Engine/CLI version actually deployed.
- Compose’s support for optional `deploy` fields is platform and implementation dependent; the field’s presence in a YAML file does not guarantee identical behavior under `docker compose` and `docker stack deploy`.
- Swarm’s routing mesh is convenient but may not match every ingress, source-IP, TLS, or load-balancing requirement. A dedicated external load balancer may still be preferable.
- Swarm can restart and reschedule containers, but it does not provide application-level data replication, schema migration, backup, or disaster-recovery semantics.
- Docker’s current Swarm documentation recommends Compose when Swarm is not the intended runtime and points Kubernetes users toward Docker Desktop’s integrated Kubernetes feature. The choice should therefore be based on a concrete platform requirement, not on treating Swarm as a mandatory next step after Compose.

## Recommendation

Start with plain Compose. Keep the Compose file cleanly separated from environment-specific overrides, build images in CI, and use explicit image versions.

Move to Swarm only when a measured requirement crosses the single-host boundary: multi-node placement, automatic replacement after node failure, cluster-wide networking, or declarative rolling updates. Before doing so, run a compatibility review of the Compose file, establish a registry, design storage and database failover explicitly, configure manager quorum and backups, and test node loss plus a failed rollout.

The shortest practical rule is:

> **Compose runs an application on one Docker host; Swarm operates services across a Docker cluster.**

## Sources

- [Docker Compose](https://docs.docker.com/compose/) — Docker Documentation; accessed 2026-07-31
- [Use Compose in production](https://docs.docker.com/compose/how-tos/production/) — Docker Documentation; accessed 2026-07-31
- [Networking in Compose](https://docs.docker.com/compose/how-tos/networking/) — Docker Documentation; accessed 2026-07-31
- [Swarm mode](https://docs.docker.com/engine/swarm/) — Docker Documentation; accessed 2026-07-31
- [How services work](https://docs.docker.com/engine/swarm/how-swarm-mode-works/services/) — Docker Documentation; accessed 2026-07-31
- [Deploy a stack to a swarm](https://docs.docker.com/engine/swarm/stack-deploy/) — Docker Documentation; accessed 2026-07-31
- [docker stack deploy](https://docs.docker.com/reference/cli/docker/stack/deploy/) — Docker Documentation; accessed 2026-07-31
- [Deploy services to a swarm](https://docs.docker.com/engine/swarm/services/) — Docker Documentation; accessed 2026-07-31
- [Manage swarm service networks](https://docs.docker.com/engine/swarm/networking/) — Docker Documentation; accessed 2026-07-31
- [Manage sensitive data with Docker secrets](https://docs.docker.com/engine/swarm/secrets/) — Docker Documentation; accessed 2026-07-31
- [Administer and maintain a swarm of Docker Engines](https://docs.docker.com/engine/swarm/admin_guide/) — Docker Documentation; accessed 2026-07-31
- [Compose Deploy Specification](https://compose-spec.github.io/compose-spec/deploy.html) — Compose Specification; accessed 2026-07-31
