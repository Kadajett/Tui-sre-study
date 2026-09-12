use crate::lab_runtime::{self as runtime, IMAGE, LABEL};
use anyhow::Result;

pub const WEB: &str = "sre-practice-web";
pub const CLIENT: &str = "sre-practice-client";
pub const NETWORK: &str = "sre-practice-net";

fn create(name: &str, script: &str) -> Result<String> {
    runtime::checked(
        &[
            "docker",
            "run",
            "-d",
            "--name",
            name,
            "--label",
            LABEL,
            "--network",
            NETWORK,
            "--network-alias",
            if name == WEB { "web" } else { "client" },
            "--memory",
            "64m",
            "--cpus",
            "0.2",
            "--pids-limit",
            "32",
            "--cap-drop",
            "ALL",
            "--security-opt",
            "no-new-privileges",
            "--read-only",
            "--user",
            "10001:10001",
            "--tmpfs",
            "/tmp:rw,noexec,nosuid,size=16m,mode=1777",
            IMAGE,
            "sh",
            "-c",
            script,
        ],
        None,
    )
}

pub fn start(lab: &str) -> Result<String> {
    runtime::checked(
        &[
            "docker",
            "network",
            "create",
            "--internal",
            "--label",
            LABEL,
            NETWORK,
        ],
        None,
    )?;
    create(CLIENT, "exec sleep 604800")?;
    let port = if lab == "docker-port" { "8081" } else { "8080" };
    create(WEB, &format!("mkdir -p /tmp/www; echo 'SRE practice service healthy' > /tmp/www/index.html; echo 'listening on :{port}'; exec httpd -f -v -p {port} -h /tmp/www"))?;
    match lab {
        "docker-stopped" => runtime::checked(&["docker", "stop", "--time", "1", WEB], None),
        "docker-network" => {
            runtime::checked(&["docker", "network", "disconnect", NETWORK, WEB], None)
        }
        _ => Ok("Practice web and client containers created.".into()),
    }
}

pub fn stop() -> Result<String> {
    let containers = runtime::checked(
        &["docker", "ps", "-aq", "--filter", &format!("label={LABEL}")],
        None,
    )?;
    for id in containers.lines().filter(|id| !id.is_empty()) {
        runtime::checked(&["docker", "rm", "-f", id], None)?;
    }
    let networks = runtime::checked(
        &[
            "docker",
            "network",
            "ls",
            "-q",
            "--filter",
            &format!("label={LABEL}"),
        ],
        None,
    )?;
    for id in networks.lines().filter(|id| !id.is_empty()) {
        runtime::checked(&["docker", "network", "rm", id], None)?;
    }
    Ok("Removed trainer-owned Docker lab resources.".into())
}

pub fn verify(lab: &str) -> Result<runtime::Response> {
    let url = if lab == "docker-port" {
        "http://web:8081/"
    } else {
        "http://web:8080/"
    };
    runtime::execute(
        &["docker", "exec", CLIENT, "wget", "-T", "3", "-qO-", url]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>(),
        None,
    )
}
