//! Interactive REPL for Soyuz SDF scripting
//!
//! Provides an interactive environment for experimenting with SDF operations.

use anyhow::Result;
use rhai::Scope;
use rustyline::error::ReadlineError;
use rustyline::history::DefaultHistory;
use rustyline::{CompletionType, Config, EditMode, Editor};
use soyuz_script::ScriptEngine;
use std::path::PathBuf;

/// REPL state
pub struct Repl {
    engine: ScriptEngine,
    scope: Scope<'static>,
    editor: Editor<(), DefaultHistory>,
    history_path: Option<PathBuf>,
    current_sdf: Option<soyuz_render::SdfOp>,
}

impl Repl {
    /// Create a new REPL instance
    pub fn new() -> Result<Self> {
        let config = Config::builder()
            .history_ignore_space(true)
            .completion_type(CompletionType::List)
            .edit_mode(EditMode::Emacs)
            .build();

        let mut editor = Editor::with_config(config)?;

        // Try to load history from home directory
        let history_path = dirs_path();
        if let Some(ref path) = history_path {
            let _ = editor.load_history(path);
        }

        Ok(Self {
            engine: ScriptEngine::new(),
            scope: Scope::new(),
            editor,
            history_path,
            current_sdf: None,
        })
    }

    /// Run the REPL loop
    pub fn run(&mut self) -> Result<()> {
        println!("{}", WELCOME_MESSAGE);

        let mut multiline_buffer = String::new();
        let mut in_multiline = false;

        loop {
            let prompt = if in_multiline { "...> " } else { "sdf> " };

            match self.editor.readline(prompt) {
                Ok(line) => {
                    let trimmed = line.trim();

                    // Handle commands
                    if trimmed.starts_with(':') && !in_multiline {
                        match self.handle_command(trimmed) {
                            CommandResult::Continue => continue,
                            CommandResult::Exit => break,
                            CommandResult::Error(e) => {
                                eprintln!("Error: {}", e);
                                continue;
                            }
                        }
                    }

                    // Check for multiline input
                    if trimmed.ends_with('\\') {
                        multiline_buffer.push_str(&line[..line.len() - 1]);
                        multiline_buffer.push('\n');
                        in_multiline = true;
                        continue;
                    }

                    // Check for unbalanced braces
                    let full_input = if in_multiline {
                        multiline_buffer.push_str(&line);
                        let input = multiline_buffer.clone();
                        multiline_buffer.clear();
                        in_multiline = false;
                        input
                    } else {
                        line.clone()
                    };

                    // Check if braces are balanced
                    if !is_balanced(&full_input) {
                        multiline_buffer = full_input;
                        multiline_buffer.push('\n');
                        in_multiline = true;
                        continue;
                    }

                    // Skip empty lines
                    if full_input.trim().is_empty() {
                        continue;
                    }

                    // Add to history
                    let _ = self.editor.add_history_entry(&full_input);

                    // Evaluate
                    self.eval_and_print(&full_input);
                }
                Err(ReadlineError::Interrupted) => {
                    if in_multiline {
                        println!("^C - input cancelled");
                        multiline_buffer.clear();
                        in_multiline = false;
                    } else {
                        println!("Use :quit or Ctrl+D to exit");
                    }
                }
                Err(ReadlineError::Eof) => {
                    println!("\nGoodbye!");
                    break;
                }
                Err(err) => {
                    eprintln!("Error: {:?}", err);
                    break;
                }
            }
        }

        // Save history
        if let Some(ref path) = self.history_path {
            let _ = self.editor.save_history(path);
        }

        Ok(())
    }

    /// Evaluate input and print result
    fn eval_and_print(&mut self, input: &str) {
        // Try to evaluate as SDF expression
        match self
            .engine
            .inner()
            .eval_with_scope::<rhai::Dynamic>(&mut self.scope, input)
        {
            Ok(result) => {
                // Check if result is an SDF
                if let Some(sdf) = result.clone().try_cast::<soyuz_script::RhaiSdf>() {
                    self.current_sdf = Some(sdf.to_sdf_op());
                    println!("=> [SDF]");
                    println!("   Use :preview to see it, or :export <file> to save");
                } else if result.is_unit() {
                    // Statement executed, no output
                } else {
                    // Print other values
                    println!("=> {}", result);
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }

    /// Handle REPL commands
    fn handle_command(&mut self, cmd: &str) -> CommandResult {
        let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
        let command = parts[0];
        let args = parts.get(1).map(|s| s.trim());

        match command {
            ":help" | ":h" | ":?" => {
                println!("{}", HELP_MESSAGE);
                CommandResult::Continue
            }
            ":quit" | ":q" | ":exit" => CommandResult::Exit,
            ":clear" | ":c" => {
                self.scope.clear();
                self.current_sdf = None;
                println!("Scope cleared");
                CommandResult::Continue
            }
            ":vars" | ":v" => {
                if self.scope.is_empty() {
                    println!("No variables defined");
                } else {
                    println!("Variables:");
                    for (name, _constant, value) in self.scope.iter() {
                        println!("  {} = {}", name, value);
                    }
                }
                CommandResult::Continue
            }
            ":preview" | ":p" => self.open_preview(),
            ":export" | ":e" => match args {
                Some(path) => self.export_mesh(path),
                None => {
                    println!("Usage: :export <filename>");
                    CommandResult::Continue
                }
            },
            ":load" | ":l" => match args {
                Some(path) => self.load_script(path),
                None => {
                    println!("Usage: :load <filename>");
                    CommandResult::Continue
                }
            },
            ":sdf" => {
                match &self.current_sdf {
                    Some(sdf) => println!("Current SDF: {:?}", sdf),
                    None => println!("No SDF defined. Create one with e.g. sphere(1.0)"),
                }
                CommandResult::Continue
            }
            ":examples" => {
                println!("{}", EXAMPLES);
                CommandResult::Continue
            }
            _ => CommandResult::Error(format!(
                "Unknown command: {}. Type :help for available commands.",
                command
            )),
        }
    }

    /// Open preview window with current SDF
    fn open_preview(&self) -> CommandResult {
        let Some(sdf) = &self.current_sdf else {
            println!("No SDF to preview. Create one first, e.g.: sphere(1.0)");
            return CommandResult::Continue;
        };

        println!("Opening preview window...");
        println!("Press Escape or close the window to return to REPL");
        println!("{}", soyuz_render::controls_help());

        let config = soyuz_render::WindowConfig::default();
        if let Err(e) = soyuz_render::run_preview_with_sdf(config, Some(sdf.clone())) {
            return CommandResult::Error(format!("Preview failed: {}", e));
        }

        CommandResult::Continue
    }

    /// Export current SDF to mesh file
    fn export_mesh(&self, path: &str) -> CommandResult {
        if self.current_sdf.is_none() {
            println!("No SDF to export. Create one first.");
            return CommandResult::Continue;
        }

        println!("Exporting to {}...", path);

        // Convert SdfOp to Sdf for mesh generation
        // Note: This is a simplified version - full implementation would need
        // a way to convert SdfOp back to a CPU-evaluable SDF
        println!("Export not yet implemented for REPL SDFs.");
        println!("Use 'soyuz generate' with a script file instead.");

        CommandResult::Continue
    }

    /// Load and execute a script file
    fn load_script(&mut self, path: &str) -> CommandResult {
        let path = std::path::Path::new(path);

        match std::fs::read_to_string(path) {
            Ok(script) => {
                println!("Loading {}...", path.display());
                self.eval_and_print(&script);
                CommandResult::Continue
            }
            Err(e) => CommandResult::Error(format!("Failed to read {}: {}", path.display(), e)),
        }
    }
}

/// Result of handling a command
enum CommandResult {
    Continue,
    Exit,
    Error(String),
}

/// Check if braces/brackets/parens are balanced
fn is_balanced(input: &str) -> bool {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut prev_char = '\0';

    for c in input.chars() {
        if c == '"' && prev_char != '\\' {
            in_string = !in_string;
        }

        if !in_string {
            match c {
                '(' | '{' | '[' => depth += 1,
                ')' | '}' | ']' => depth -= 1,
                _ => {}
            }
        }

        prev_char = c;
    }

    depth == 0
}

/// Get the history file path
fn dirs_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("soyuz").join("repl_history"))
}

const WELCOME_MESSAGE: &str = r#"
╔═══════════════════════════════════════════════════════════╗
║             Soyuz Interactive SDF REPL                     ║
║                                                            ║
║  Type SDF expressions to create shapes.                    ║
║  Type :help for commands, :examples for usage examples.    ║
╚═══════════════════════════════════════════════════════════╝
"#;

const HELP_MESSAGE: &str = r#"
Commands:
  :help, :h, :?     - Show this help message
  :quit, :q, :exit  - Exit the REPL
  :clear, :c        - Clear all variables and current SDF
  :vars, :v         - Show defined variables
  :preview, :p      - Open preview window with current SDF
  :export <file>    - Export current SDF to mesh file
  :load <file>      - Load and execute a script file
  :sdf              - Show current SDF structure
  :examples         - Show usage examples

Tips:
  - End a line with \ for multiline input
  - Unfinished expressions (unbalanced braces) continue on next line
  - Variables persist between evaluations
  - Use Ctrl+C to cancel current input, Ctrl+D to exit
"#;

const EXAMPLES: &str = r#"
Examples:

  // Create basic shapes
  sphere(1.0)
  cube(2.0)
  cylinder(0.5, 2.0)
  torus(1.0, 0.3)

  // Store in variables
  let body = cylinder(0.5, 1.0);
  let ring = torus(0.5, 0.1);

  // Combine shapes
  body.union(ring)
  body.smooth_union(ring, 0.1)
  body.subtract(sphere(0.3))

  // Transform shapes
  sphere(1.0).translate(1.0, 0.0, 0.0)
  cube(1.0).rotate_y(0.785)  // 45 degrees in radians
  sphere(1.0).scale(2.0)

  // Modifiers
  sphere(1.0).hollow(0.1)
  cube(1.0).round(0.1)
  sphere(1.0).onion(0.1)

  // Repetition
  sphere(0.2).repeat(1.0, 1.0, 1.0)
  sphere(0.2).repeat_polar(6)

  // Complex example
  let barrel = cylinder(0.5, 1.2) \
      .smooth_union(torus(0.5, 0.08).translate_y(0.5), 0.05) \
      .smooth_union(torus(0.5, 0.08).translate_y(-0.5), 0.05) \
      .hollow(0.05);
  :preview
"#;

/// Entry point for the REPL command
pub fn run_repl() -> Result<()> {
    let mut repl = Repl::new()?;
    repl.run()
}
