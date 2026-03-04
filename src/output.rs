use std::io::IsTerminal;

use owo_colors::{OwoColorize, Style};

/// Color policy: --no-color > NO_COLOR env > TERM=dumb > !isatty > default (color)
fn should_use_color_for(no_color_flag: bool, is_tty: bool) -> bool {
    if no_color_flag {
        return false;
    }
    if std::env::var("NO_COLOR").is_ok() {
        return false;
    }
    if std::env::var("TERM").ok().as_deref() == Some("dumb") {
        return false;
    }
    is_tty
}

fn should_use_color(no_color_flag: bool) -> bool {
    should_use_color_for(no_color_flag, std::io::stdout().is_terminal())
}

fn should_use_color_stderr(no_color_flag: bool) -> bool {
    should_use_color_for(no_color_flag, std::io::stderr().is_terminal())
}

struct Colors {
    id: Style,
    success: Style,
    warning: Style,
    error_style: Style,
    dimmed: Style,
}

impl Colors {
    fn new(use_color: bool) -> Self {
        if use_color {
            Self {
                id: Style::new().cyan().dimmed(),
                success: Style::new().green(),
                warning: Style::new().yellow(),
                error_style: Style::new().red().bold(),
                dimmed: Style::new().dimmed(),
            }
        } else {
            Self {
                id: Style::new(),
                success: Style::new(),
                warning: Style::new(),
                error_style: Style::new(),
                dimmed: Style::new(),
            }
        }
    }
}

pub struct Printer {
    colors: Colors,
}

impl Printer {
    pub fn new(no_color_flag: bool) -> Self {
        let use_color = should_use_color(no_color_flag);
        Self {
            colors: Colors::new(use_color),
        }
    }

    pub fn new_for_stderr(no_color_flag: bool) -> Self {
        let use_color = should_use_color_stderr(no_color_flag);
        Self {
            colors: Colors::new(use_color),
        }
    }

    pub fn print_error(&self, message: &str) {
        eprintln!("{}", message.style(self.colors.error_style));
    }

    pub fn print_success(&self, message: &str) {
        println!("{}", message.style(self.colors.success));
    }

    pub fn print_json(&self, output: &str) {
        println!("{}", output);
    }

    pub fn fmt_id(&self, id: &impl std::fmt::Display) -> String {
        format!("{}", id.to_string().style(self.colors.id))
    }

    #[allow(dead_code)]
    pub fn fmt_dimmed(&self, text: &str) -> String {
        format!("{}", text.style(self.colors.dimmed))
    }

    #[allow(dead_code)]
    pub fn fmt_warning(&self, text: &str) -> String {
        format!("{}", text.style(self.colors.warning))
    }
}

impl Default for Printer {
    fn default() -> Self {
        Self::new(false)
    }
}
