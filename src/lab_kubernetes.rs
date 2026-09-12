use crate::lab_runtime::{self as runtime, IMAGE, NAMESPACE};
use anyhow::Result;
use serde_json::{json, Value};

fn security() -> Value {
    json!({"allowPrivilegeEscalation":false,"readOnlyRootFilesystem":true,"runAsNonRoot":true,"runAsUser":10001,"capabilities":{"drop":["ALL"]},"seccompProfile":{"type":"RuntimeDefault"}})
}

fn container(name: &str, script: &str) -> Value {
    json!({"name":name,"image":IMAGE,"command":["sh","-c",script],"securityContext":security(),"resources":{"requests":{"cpu":"10m","memory":"16Mi"},"limits":{"cpu":"200m","memory":"64Mi"}},"volumeMounts":[{"name":"scratch","mountPath":"/tmp"}]})
}

fn metadata(name: &str) -> Value {
    json!({"name":name,"namespace":NAMESPACE,"labels":{"app.kubernetes.io/managed-by":"sre-trainer","app":name}})
}

pub fn manifest(lab: &str) -> Value {
    let mut web = container("web", "if [ \"$MODE\" != serving ]; then echo 'configuration error: MODE must be serving'; exit 1; fi; mkdir -p /tmp/www; echo 'SRE practice service healthy' > /tmp/www/index.html; echo 'listening on :8080'; exec httpd -f -v -p 8080 -h /tmp/www");
    web["env"] = json!([{"name":"MODE","value":if lab=="k8s-crash" {"broken"} else {"serving"}}]);
    web["readinessProbe"] = json!({"httpGet":{"path":if lab=="k8s-readiness" {"/missing"} else {"/"},"port":8080},"periodSeconds":2});
    let mut spec = json!({"automountServiceAccountToken":false,"terminationGracePeriodSeconds":1,"containers":[web],"volumes":[{"name":"scratch","emptyDir":{"sizeLimit":"16Mi"}}]});
    if lab == "k8s-pending" {
        spec["nodeSelector"] = json!({"sre-practice.invalid/node":"missing"});
    }
    if let Ok(node) = std::env::var("SRE_LAB_NODE") {
        if !node.is_empty() {
            spec["affinity"] = json!({"nodeAffinity":{"requiredDuringSchedulingIgnoredDuringExecution":{"nodeSelectorTerms":[{"matchExpressions":[{"key":"kubernetes.io/hostname","operator":"In","values":[node]}]}]}}});
        }
    }
    let deployment = json!({"apiVersion":"apps/v1","kind":"Deployment","metadata":metadata("web"),"spec":{"replicas":1,"selector":{"matchLabels":{"app":"web"}},"template":{"metadata":{"labels":{"app":"web","app.kubernetes.io/managed-by":"sre-trainer"}},"spec":spec}}});
    let mut client = json!({"apiVersion":"v1","kind":"Pod","metadata":metadata("client"),"spec":{"automountServiceAccountToken":false,"terminationGracePeriodSeconds":1,"containers":[container("client","exec sleep 604800")],"volumes":[{"name":"scratch","emptyDir":{"sizeLimit":"16Mi"}}]}});
    if let Some(affinity) = spec.get("affinity") {
        client["spec"]["affinity"] = affinity.clone();
    }
    let service = json!({"apiVersion":"v1","kind":"Service","metadata":metadata("web"),"spec":{"selector":{"app":if lab=="k8s-selector" {"wrong"} else {"web"}},"ports":[{"port":80,"targetPort":8080}]}});
    json!({"apiVersion":"v1","kind":"List","items":[deployment,client,service]})
}

pub fn start(lab: &str) -> Result<String> {
    let body = serde_json::to_string(&manifest(lab))?;
    runtime::kube(&["apply", "-f", "-"], Some(&body))
}

pub fn stop() -> Result<String> {
    runtime::kube(
        &[
            "delete",
            "deployment,replicaset,pod,service",
            "-l",
            runtime::LABEL,
            "--ignore-not-found",
            "--wait=false",
        ],
        None,
    )?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(65);
    loop {
        let remaining = runtime::kube(
            &[
                "get",
                "deployment,replicaset,pod,service",
                "-l",
                runtime::LABEL,
                "-o",
                "json",
            ],
            None,
        )?;
        let objects: Value = serde_json::from_str(&remaining)?;
        if objects["items"].as_array().is_some_and(Vec::is_empty) {
            return Ok("Practice Kubernetes resources removed.".into());
        }
        anyhow::ensure!(std::time::Instant::now()<deadline, "Practice resources are still terminating. Inspect their events and retry /lab stop; they have not been force-deleted.");
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

pub fn verify() -> Result<runtime::Response> {
    let command = vec![
        "kubectl",
        "--request-timeout=15s",
        "-n",
        NAMESPACE,
        "exec",
        "client",
        "--",
        "wget",
        "-T",
        "3",
        "-qO-",
        "http://web/",
    ];
    runtime::execute(
        &command.into_iter().map(String::from).collect::<Vec<_>>(),
        None,
    )
}
