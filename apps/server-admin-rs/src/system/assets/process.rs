use std::process::Command;

pub(super) fn run_process_success(command: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(command)
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        return Ok(());
    }
    let detail = summarize_process_output(&output.stdout, &output.stderr);
    Err(if detail.is_empty() {
        format!("{command} failed")
    } else {
        detail
    })
}

pub(super) fn summarize_process_output(stdout: &[u8], stderr: &[u8]) -> String {
    let detail = format!(
        "{}\n{}",
        String::from_utf8_lossy(stderr),
        String::from_utf8_lossy(stdout)
    );
    detail
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" | ")
        .chars()
        .take(500)
        .collect()
}

pub(super) fn command_succeeds(command: &str, args: &[&str]) -> bool {
    Command::new(command)
        .args(args)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
