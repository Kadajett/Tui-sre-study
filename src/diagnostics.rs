use anyhow::{Context, Result};

pub fn run(args: &[String]) -> Result<bool> {
    if let Some(index) = args.iter().position(|arg| arg == "--check-teacher") {
        return check_teacher(args.get(index + 1).map(String::as_str));
    }
    if let Some(index) = args.iter().position(|arg| arg == "--check-coach") {
        println!(
            "{}",
            crate::coach::completion(
                args.get(index + 1)
                    .map(String::as_str)
                    .unwrap_or("Explain what du -h adds in one short sentence. No lookup needed."),
                Vec::new()
            )?
        );
        return Ok(true);
    }
    if let Some(index) = args.iter().position(|arg| arg == "--lookup") {
        let name = args
            .get(index + 1)
            .context("Expected a reference tool name")?;
        let arguments = args
            .get(index + 2)
            .context("Expected JSON lookup arguments")?;
        let reference = crate::references::lookup(name, arguments)?;
        println!("{}\n{}", reference.text, reference.sources.join("\n"));
        return Ok(true);
    }
    Ok(false)
}

fn check_teacher(topic: Option<&str>) -> Result<bool> {
    let path = std::env::var("SRE_LESSONS").unwrap_or_else(|_| "data/lessons.json".into());
    let lessons = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    let directory = tempfile::TempDir::new()?;
    let store = crate::Store::open(&directory.path().join("check.db"))?;
    let mut app = crate::teaching::Teacher::new(store, lessons, None)?;
    if let Some(topic) = topic {
        app.submit(&format!("/topic {topic}"))?;
    }
    let turn = app.queue.pop_front().context("Missing opening turn")?;
    println!(
        "{}",
        crate::coach::teaching_completion(&app.context(&turn)?, Vec::new())?
    );
    Ok(true)
}
