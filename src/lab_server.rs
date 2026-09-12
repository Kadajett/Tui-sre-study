use crate::lab_runtime::{Request, Response};
use anyhow::{ensure, Result};
use std::{io::Read, path::PathBuf};

fn state_path() -> PathBuf {
    PathBuf::from(std::env::var("SRE_LAB_DATA").unwrap_or_else(|_| "/data".into()))
        .join("active-lab.json")
}

fn active() -> Result<Option<String>> {
    match std::fs::read_to_string(state_path()) {
        Ok(body) => Ok(Some(serde_json::from_str(&body)?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn start(lab: &str) -> Result<Response> {
    if let Some(current) = active()? {
        ensure!(current==lab, "Lab {current} is already active. Use /lab stop before creating another; restarting the trainer never resets a live lab.");
        return Ok(Response {ok:true,output:format!("Resumed existing {lab} resources. Use /lab commands to investigate; /lab stop followed by an explicit start recreates a damaged or incomplete fixture.")});
    }
    save_active(lab)?;
    let output = if lab.starts_with("k8s-") {
        crate::lab_kubernetes::start(lab)?
    } else {
        crate::lab_docker::start(lab)?
    };
    Ok(Response { ok: true, output })
}

fn save_active(lab: &str) -> Result<()> {
    let path = state_path();
    std::fs::create_dir_all(path.parent().unwrap_or_else(|| std::path::Path::new(".")))?;
    std::fs::write(path.with_extension("tmp"), serde_json::to_vec(lab)?)?;
    std::fs::rename(path.with_extension("tmp"), path)?;
    Ok(())
}

pub fn handle(request: Request) -> Result<Response> {
    ensure!(
        crate::scenario_catalog::known(&request.lab),
        "Unknown practice lab"
    );
    if request.action == "start" {
        return start(&request.lab);
    }
    ensure!(
        active()?.as_deref() == Some(request.lab.as_str()),
        "This lab is not active. Start or resume it before executing commands."
    );
    match request.action.as_str() {
        "command" => crate::lab_runtime::execute(
            &crate::lab_commands::validate(&request.lab, &request.command)?,
            None,
        ),
        "verify" if request.lab.starts_with("k8s-") => crate::lab_kubernetes::verify(),
        "verify" => crate::lab_docker::verify(&request.lab),
        "stop" => stop(&request.lab),
        _ => anyhow::bail!("Unsupported lab action"),
    }
}

fn stop(lab: &str) -> Result<Response> {
    let output = if lab.starts_with("k8s-") {
        crate::lab_kubernetes::stop()?
    } else {
        crate::lab_docker::stop()?
    };
    std::fs::remove_file(state_path())?;
    Ok(Response { ok: true, output })
}

fn decode(request: &mut tiny_http::Request) -> Result<Request> {
    ensure!(
        request.method() == &tiny_http::Method::Post && request.url() == "/lab",
        "POST /lab required"
    );
    ensure!(
        request.body_length().unwrap_or(0) <= 16_384,
        "Request too large"
    );
    let mut bytes = Vec::new();
    request.as_reader().take(16_385).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 16_384, "Request too large");
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn serve() -> Result<()> {
    let address = std::env::var("SRE_LAB_BIND").unwrap_or_else(|_| "0.0.0.0:8766".into());
    let server = tiny_http::Server::http(address).map_err(|error| anyhow::anyhow!("{error}"))?;
    for mut request in server.incoming_requests() {
        let response = decode(&mut request)
            .and_then(handle)
            .unwrap_or_else(|error| Response {
                ok: false,
                output: error.to_string(),
            });
        let body = serde_json::to_string(&response)?;
        if let Err(error) = request.respond(tiny_http::Response::from_string(body)) {
            eprintln!("Lab client disconnected: {error}");
        }
    }
    Ok(())
}
