use anyhow::{ensure, Result};

const KUBE_READ: &[&str] = &[
    "kubectl config current-context",
    "kubectl get pods -n sre-practice",
    "kubectl get pods -n sre-practice -o wide",
    "kubectl get pods -n sre-practice -o yaml",
    "kubectl get deployments -n sre-practice",
    "kubectl describe deployment web -n sre-practice",
    "kubectl describe pods -n sre-practice",
    "kubectl get events -n sre-practice --sort-by=.metadata.creationTimestamp",
    "kubectl logs deployment/web -n sre-practice --tail=20",
    "kubectl get service web -n sre-practice -o yaml",
    "kubectl get endpointslices -n sre-practice",
    "kubectl get deployment web -n sre-practice -o yaml",
    "kubectl rollout status deployment/web -n sre-practice --timeout=10s",
    "kubectl exec client -n sre-practice -- wget -T 3 -qO- http://web/",
];
const DOCKER_READ: &[&str] = &[
    "docker ps --filter label=app.kubernetes.io/managed-by=sre-trainer",
    "docker ps -a --filter label=app.kubernetes.io/managed-by=sre-trainer",
    "docker inspect sre-practice-web",
    "docker logs --tail 20 sre-practice-web",
    "docker network inspect sre-practice-net",
    "docker exec sre-practice-client wget -T 3 -qO- http://web:8080/",
    "docker exec sre-practice-client wget -T 3 -qO- http://web:8081/",
];

pub fn available(lab: &str) -> Vec<String> {
    let base = if lab.starts_with("k8s-") {
        KUBE_READ
    } else {
        DOCKER_READ
    };
    let mut commands = base.iter().map(|s| (*s).into()).collect::<Vec<String>>();
    let repair = match lab {
        "k8s-selector" => "kubectl patch service web -n sre-practice --type=merge -p '{\"spec\":{\"selector\":{\"app\":\"web\"}}}'",
        "k8s-readiness" => "kubectl patch deployment web -n sre-practice --type=json -p '[{\"op\":\"replace\",\"path\":\"/spec/template/spec/containers/0/readinessProbe/httpGet/path\",\"value\":\"/\"}]'",
        "k8s-crash" => "kubectl set env deployment/web MODE=serving -n sre-practice",
        "k8s-pending" => "kubectl patch deployment web -n sre-practice --type=json -p '[{\"op\":\"remove\",\"path\":\"/spec/template/spec/nodeSelector\"}]'",
        "docker-stopped" => "docker start sre-practice-web",
        "docker-network" => "docker network connect --alias web sre-practice-net sre-practice-web",
        _ => "",
    };
    if !repair.is_empty() {
        commands.push(repair.into());
    }
    commands
}

pub fn validate(lab: &str, command: &str) -> Result<Vec<String>> {
    let parts = shell_words::split(command)?;
    ensure!(available(lab).iter().any(|allowed| shell_words::split(allowed).ok().as_ref()==Some(&parts)),
        "This command is outside the active practice lab. Use /lab commands for supported real commands; arbitrary shell and production resources are unavailable.");
    Ok(parts)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn commands_cannot_escape_lab() {
        for command in [
            "kubectl get secrets -A",
            "kubectl exec client -n sre-practice -- sh",
            "docker inspect mixpost-mysql-1",
            "docker run -v /:/host busybox",
            "docker start sre-practice-web; id",
            "kubectl --context production get pods -n sre-practice",
        ] {
            assert!(validate("k8s-basics", command).is_err());
            assert!(validate("docker-stopped", command).is_err());
        }
        assert!(validate("docker-stopped", "docker start sre-practice-web").is_ok());
        assert!(validate("docker-basics", "docker start sre-practice-web").is_err());
    }
}
