use crate::lab_runtime::{remote, Request, Response};
use anyhow::{ensure, Result};

fn call(lab: &str, action: &str, command: &str) -> Result<Response> {
    remote(&Request {
        lab: lab.into(),
        action: action.into(),
        command: command.into(),
    })
}

fn successful(lab: &str, action: &str, command: &str) -> Result<String> {
    let result = call(lab, action, command)?;
    ensure!(result.ok, "{lab} {action}: {}", result.output);
    Ok(result.output)
}

fn await_command(lab: &str, command: &str) -> Result<()> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(100);
    loop {
        let response = call(lab, "command", command)?;
        if response.ok {
            return Ok(());
        }
        ensure!(
            std::time::Instant::now() < deadline,
            "{lab} did not become ready: {}",
            response.output
        );
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

fn client_ready(lab: &str) -> Result<()> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(100);
    loop {
        let response = call(lab, "command", "kubectl get pods -n sre-practice")?;
        if response.ok
            && response
                .output
                .lines()
                .any(|line| line.starts_with("client ") && line.contains("1/1"))
        {
            return Ok(());
        }
        ensure!(
            std::time::Instant::now() < deadline,
            "Client pod not ready: {}",
            response.output
        );
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

fn request(lab: &str, actual: bool) -> &'static str {
    if lab.starts_with("k8s-") {
        return "kubectl exec client -n sre-practice -- wget -T 3 -qO- http://web/";
    }
    if lab == "docker-port" && actual {
        return "docker exec sre-practice-client wget -T 3 -qO- http://web:8081/";
    }
    "docker exec sre-practice-client wget -T 3 -qO- http://web:8080/"
}

fn exercise(lab: &str) -> Result<()> {
    if lab.starts_with("k8s-") {
        client_ready(lab)?;
    }
    let denied = call(
        lab,
        "command",
        "docker inspect unrelated-production-container",
    )?;
    ensure!(!denied.ok, "An unrelated command was accepted");
    let failed = call(lab, "command", request(lab, false))?;
    ensure!(
        !failed.ok,
        "{lab} did not exhibit its intended client failure"
    );
    println!("{lab}: real client failure observed");
    investigate_and_repair(lab)?;
    await_command(lab, request(lab, true))?;
    let verified = successful(lab, "verify", "")?;
    ensure!(
        verified.contains("SRE practice service healthy"),
        "Wrong recovery response"
    );
    successful(lab, "start", "")?;
    successful(lab, "verify", "")?;
    println!("{lab}: repair, client verification and resume passed");
    Ok(())
}

fn investigate_and_repair(lab: &str) -> Result<()> {
    let commands = crate::lab_commands::available(lab);
    let observation = if lab.starts_with("k8s-") {
        "kubectl describe deployment web -n sre-practice"
    } else {
        "docker inspect sre-practice-web"
    };
    successful(lab, "command", observation)?;
    if lab != "docker-port" {
        let repair = commands
            .last()
            .ok_or_else(|| anyhow::anyhow!("Missing repair command"))?;
        successful(lab, "command", repair)?;
    }
    Ok(())
}

pub fn run() -> Result<()> {
    for scenario in crate::scenario_catalog::catalog() {
        successful(&scenario.id, "start", "")?;
        let result = exercise(&scenario.id);
        let cleanup = successful(&scenario.id, "stop", "");
        result?;
        cleanup?;
        println!("{}: cleaned up", scenario.id);
    }
    for lab in ["k8s-basics", "docker-basics"] {
        successful(lab, "start", "")?;
        let result = await_command(lab, request(lab, true));
        let cleanup = successful(lab, "stop", "");
        result?;
        cleanup?;
        println!("{lab}: healthy baseline passed and cleaned up");
    }
    Ok(())
}
