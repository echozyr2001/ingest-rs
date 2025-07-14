use colored::*;

pub fn success(message: &str) {
    println!("{} {}", "✓".bright_green(), message);
}

pub fn info(message: &str) {
    println!("{} {}", "ℹ".bright_blue(), message);
}

#[allow(dead_code)]
pub fn warning(message: &str) {
    println!("{} {}", "⚠".bright_yellow(), message);
}

pub fn error(message: &str) {
    eprintln!("{} {}", "✗".bright_red(), message);
}

#[allow(dead_code)]
pub fn step(step: usize, total: usize, message: &str) {
    println!(
        "{} {} {}",
        format!("[{step}/{total}]").bright_cyan(),
        "→".bright_green(),
        message
    );
}

#[allow(dead_code)]
pub fn progress(message: &str) {
    println!("{} {}", "⏳".bright_yellow(), message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_functions() {
        // These functions just print to stdout/stderr
        // We test that they don't panic
        success("Test success message");
        info("Test info message");
        warning("Test warning message");
        error("Test error message");
        step(1, 3, "Test step message");
        progress("Test progress message");
    }
}
