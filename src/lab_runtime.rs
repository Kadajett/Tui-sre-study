use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
};

pub const NAMESPACE: &str = "sre-practice";
pub const IMAGE: &str = "busybox:1.37.0";
pub const LABEL: &str = "app.kubernetes.io/managed-by=sre-trainer";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub action: String,
    pub lab: String,
    #[serde(default)]
    pub command: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    pub output: String,
}

pub fn remote(request: &Request) -> Result<Response> {
    let base = std::env::var("SRE_LAB_URL").context("Real labs require SRE_LAB_URL. Start the lab-runner with Docker Compose; no sample output is substituted.")?;
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(100))
        .build()?;
    let response: Response = client
        .post(format!("{}/lab", base.trim_end_matches('/')))
        .json(request)
        .send()?
        .error_for_status()?
        .json()?;
    Ok(response)
}

pub fn execute(args: &[String], input: Option<&str>) -> Result<Response> {
    ensure!(!args.is_empty(), "Missing executable");
    let stdout = tempfile::tempfile()?;
    let stderr = tempfile::tempfile()?;
    let mut command = process(args, &stdout, &stderr)?;
    let mut child = command.spawn().context("Starting real lab command")?;
    write_input(&mut child, input)?;
    let status = child.wait()?;
    let output = format!("{}{}", read_output(stdout)?, read_output(stderr)?);
    Ok(Response {
        ok: status.success(),
        output,
    })
}

fn write_input(child: &mut std::process::Child, input: Option<&str>) -> Result<()> {
    if let Some(body) = input {
        child
            .stdin
            .take()
            .context("Missing process input")?
            .write_all(body.as_bytes())?;
    }
    drop(child.stdin.take());
    Ok(())
}

fn process(args: &[String], stdout: &std::fs::File, stderr: &std::fs::File) -> Result<Command> {
    let mut command = Command::new("timeout");
    command
        .args(["--signal=KILL", "25"])
        .args(args)
        .env_clear()
        .env("PATH", "/usr/local/bin:/usr/bin:/bin")
        .env("HOME", "/tmp")
        .env("LANG", "C.UTF-8")
        .env(
            "KUBECONFIG",
            std::env::var("KUBECONFIG").unwrap_or_else(|_| "/credentials/config".into()),
        )
        .stdin(Stdio::piped())
        .stdout(stdout.try_clone()?)
        .stderr(stderr.try_clone()?);
    Ok(command)
}

fn read_output(mut file: std::fs::File) -> Result<String> {
    use std::io::{Seek, SeekFrom};
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    file.take(32_768).read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

pub fn checked(args: &[&str], input: Option<&str>) -> Result<String> {
    let response = execute(&args.iter().map(|s| (*s).into()).collect::<Vec<_>>(), input)?;
    ensure!(response.ok, "{}", response.output);
    Ok(response.output)
}

pub fn kube(args: &[&str], input: Option<&str>) -> Result<String> {
    let mut command = vec!["kubectl", "--request-timeout=15s", "-n", NAMESPACE];
    command.extend_from_slice(args);
    checked(&command, input)
}
