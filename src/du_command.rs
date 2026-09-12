use std::{
    path::Path,
    process::{Command, Stdio},
};

use anyhow::{bail, ensure, Context, Result};

#[derive(Debug, Default, PartialEq)]
pub struct DuCommand {
    pub human: bool,
    pub summary: bool,
    pub all: bool,
    pub depth: Option<u8>,
    pub paths: Vec<String>,
    args: Vec<String>,
}

impl DuCommand {
    pub fn parse(input: &str) -> Result<Self> {
        let words = shell_words::split(input).context("Check your quotes")?;
        ensure!(
            words.first().map(String::as_str) == Some("du"),
            "This lab runs du. Type /ask followed by a question to talk to Mercury."
        );
        let mut parsed = Self::default();
        let mut args = words.iter().skip(1);
        while let Some(arg) = args.next() {
            parsed.argument(arg, &mut args)?;
        }
        ensure!(
            !(parsed.summary && (parsed.all || parsed.depth.is_some())),
            "Try -s by itself: it conflicts with -a and explicit depth in this lesson."
        );
        if parsed.paths.is_empty() {
            parsed.paths.push(".".into());
        }
        parsed.args = words.into_iter().skip(1).collect();
        Ok(parsed)
    }

    fn argument<'a>(
        &mut self,
        arg: &str,
        rest: &mut impl Iterator<Item = &'a String>,
    ) -> Result<()> {
        match arg {
            "--human-readable" => self.human = true,
            "--summarize" => self.summary = true,
            "--all" => self.all = true,
            "--total" => {},
            "--max-depth" => self.depth = Some(depth(rest.next().context("Give --max-depth a number")?)?),
            value if value.starts_with("--max-depth=") => self.depth = Some(depth(&value[12..])?),
            value if value.starts_with("--") => bail!("This lesson supports --human-readable, --summarize, --all, --total and --max-depth."),
            value if value.starts_with('-') => self.short_options(&value[1..], rest)?,
            value => {
                ensure!([".", "logs", "logs/", "./logs", "logs/archive", "./logs/archive", "cache", "./cache", "empty", "./empty"].contains(&value), "Use a lab path: ., logs, logs/archive, cache or empty. Shell operators and host paths are not supported.");
                self.paths.push(value.trim_start_matches("./").trim_end_matches('/').to_owned());
            }
        }
        Ok(())
    }

    fn short_options<'a>(
        &mut self,
        flags: &str,
        rest: &mut impl Iterator<Item = &'a String>,
    ) -> Result<()> {
        ensure!(!flags.is_empty(), "Use a flag after -, such as -h.");
        let mut flags = flags.chars();
        while let Some(flag) = flags.next() {
            match flag {
                'h' => self.human = true,
                's' => self.summary = true,
                'a' => self.all = true,
                'c' => {}
                'd' => {
                    let value = flags.as_str();
                    self.depth = Some(depth(if value.is_empty() {
                        rest.next().context("Give -d a number")?
                    } else {
                        value
                    })?);
                    return Ok(());
                }
                _ => bail!("We're exploring -h, -s, -a, -c and -d NUMBER. Try /hint."),
            }
        }
        Ok(())
    }

    pub fn run(&self, root: &Path) -> Result<String> {
        let output = Command::new("/usr/bin/timeout")
            .args(["--signal=KILL", "4", "/usr/bin/du"])
            .args(&self.args)
            .current_dir(root)
            .env_clear()
            .env("LANG", "C.UTF-8")
            .stdin(Stdio::null())
            .output()
            .context("Could not run du in the practice directory")?;
        ensure!(
            output.status.success(),
            "du exited {}: {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_owned())
    }

    pub fn targets(&self, path: &str) -> bool {
        self.paths == [path]
    }
}

fn depth(value: &str) -> Result<u8> {
    let depth: u8 = value
        .parse()
        .context("Depth must be a whole number, for example -d 1")?;
    ensure!(
        depth <= 8,
        "Use a depth from 0 through 8 for this small lab."
    );
    Ok(depth)
}

pub fn review(input: &str) -> (bool, String) {
    let result = (|| -> Result<(bool, String)> {
        let command = DuCommand::parse(input)?;
        let root = tempfile::TempDir::new()?;
        crate::make_fixture("disk_usage", root.path())?;
        let output = command.run(root.path())?;
        Ok((
            crate::du_course::meets_objective(5, &command, &output),
            format!("$ {input}\n{output}\n[exit 0]"),
        ))
    })();
    result.unwrap_or_else(|error| (false, error.to_string()))
}
