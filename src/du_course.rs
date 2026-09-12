use crate::du_command::DuCommand;

pub const LAST_STEP: usize = 5;
pub const LESSON_ID: &str = "linux-du-1";

pub struct Step {
    pub title: &'static str,
    pub introduction: &'static str,
    pub hint: &'static str,
    pub observation: &'static str,
    pub experiment: &'static str,
}

pub fn step(index: usize) -> Step {
    match index {
        0 => Step {
            title: "Meet du",
            introduction: "du means disk usage. It walks a directory tree and adds up allocated disk space. Our practice tree contains logs/archive, cache and an empty directory. Let's look before adding any options.\n\nType: du",
            hint: "Type du and press Enter. With no path, it starts in the current directory (.).",
            observation: "Each line is a directory's total, including its descendants. The final . is the whole tree. In this lab the numbers are KiB (1024-byte units). Even an empty directory can occupy disk blocks.",
            experiment: "Try du . to make the current-directory target explicit. /next adds readable units.",
        },
        1 => Step {
            title: "Add -h: readable sizes",
            introduction: "Those bare numbers are hard to scan. Add one option: -h, for human-readable sizes. It chooses suffixes such as K, M and G using powers of 1024.\n\nType: du -h",
            hint: "Type du -h. You can also spell it du --human-readable.",
            observation: "The same directories appear, now with size suffixes. Plain du reports KiB: 4 becomes 4.0K, and 4096 becomes 4.0M. -h changes how sizes are displayed; it doesn't change what du measures.",
            experiment: "Compare du and du -h. /next narrows the scan to one directory.",
        },
        2 => Step {
            title: "Choose a target: logs",
            introduction: "Keep -h, then add a directory name. A path tells du where to begin its walk.\n\nType: du -h logs",
            hint: "Type du -h logs. Options change behavior; logs is the thing being measured.",
            observation: "Only logs and its archive subtree appear. The logs total already includes archive, so adding those two rows would double-count the archive.",
            experiment: "Try du -h cache, then du -h empty. /next compresses the output to a summary.",
        },
        3 => Step {
            title: "Add -s: one summary",
            introduction: "Keep readable units and the logs target. Add -s to show just the total for that target. Short options can be combined.\n\nType: du -sh logs",
            hint: "Type du -sh logs, or du -h -s logs. -s means summarize.",
            observation: "Now logs gets one line. Its archive is still included in the total, but its separate line is hidden. -s reduces output, not the work of scanning the files.",
            experiment: "Try du -sh logs cache empty to compare three totals. /next shows a one-level breakdown.",
        },
        4 => Step {
            title: "Swap summary for a depth limit",
            introduction: "One total can hide which child is large. Replace -s with --max-depth=1 and target . again. Depth 0 is the starting directory; depth 1 includes its immediate children.\n\nType: du -h --max-depth=1 .",
            hint: "Type du -h --max-depth=1 . or du -hd1 .",
            observation: "You see logs, cache, empty and the overall total, but no separate logs/archive row. Its bytes still count in logs. Depth limits displayed rows, not traversal.",
            experiment: "Try du -h --max-depth=0 . and then depth 2. Optional: du -ah logs includes files. /next starts a challenge without a suggested command.",
        },
        _ => Step {
            title: "Your turn: investigate disk usage",
            introduction: "Start at the current directory. Show human-readable totals for each immediate child directory and the whole tree, without listing nested directories or individual files. Build the command from memory.\n\nYour command and its actual output decide whether this is complete.",
            hint: "Combine readable units (-h), depth 1 (--max-depth=1 or -d 1), and the current directory (.).",
            observation: "You built a usable disk-usage investigation command and demonstrated it against real files.",
            experiment: "Keep experimenting here. Your scheduled review will hide the example again.",
        },
    }
}

pub fn meets_objective(index: usize, command: &DuCommand, output: &str) -> bool {
    if output.is_empty() || command.all {
        return false;
    }
    let (human, summary, depth, target) = match index {
        0 => (false, false, None, "."),
        1 => (true, false, None, "."),
        2 => (true, false, None, "logs"),
        3 => (true, true, None, "logs"),
        _ => (true, false, Some(1), "."),
    };
    let options_match = (command.human, command.summary, command.depth) == (human, summary, depth);
    if !options_match || !command.targets(target) {
        return false;
    }
    if index < 4 {
        return true;
    }
    ["./logs", "./cache", "./empty"]
        .iter()
        .all(|path| output.contains(path))
        && !output.contains("./logs/archive")
}
