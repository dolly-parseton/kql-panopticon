//! Main TUI application state and loop

use crate::commands::{self, Command, CommandResult, ReplCommand};
use crate::context::SharedContext;
use crate::tui::ui;
use crate::validator::join_continuation_lines;
use anyhow::Result;
use clap::Parser;
use crossterm::event::{self, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io::Stdout;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Unique block ID generator
static NEXT_BLOCK_ID: AtomicU64 = AtomicU64::new(1);

/// Generate a unique block ID
fn next_block_id() -> BlockId {
    BlockId(NEXT_BLOCK_ID.fetch_add(1, Ordering::SeqCst))
}

/// Unique identifier for output blocks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u64);

/// Content types for output blocks
#[derive(Debug, Clone)]
pub enum BlockContent {
    /// Echoed command
    Command(String),
    /// Text output
    Text(String),
    /// Error output
    Error(String),
    /// System message (welcome, notifications)
    System(String),
}

/// A single output block in the scrollable area
#[derive(Debug, Clone)]
pub struct OutputBlock {
    pub id: BlockId,
    pub content: BlockContent,
    pub minimized: bool,
}

impl OutputBlock {
    pub fn command(cmd: impl Into<String>) -> Self {
        Self {
            id: next_block_id(),
            content: BlockContent::Command(cmd.into()),
            minimized: false,
        }
    }

    pub fn text(text: impl Into<String>) -> Self {
        Self {
            id: next_block_id(),
            content: BlockContent::Text(text.into()),
            minimized: false,
        }
    }

    pub fn error(err: impl Into<String>) -> Self {
        Self {
            id: next_block_id(),
            content: BlockContent::Error(err.into()),
            minimized: false,
        }
    }

    pub fn system(msg: impl Into<String>) -> Self {
        Self {
            id: next_block_id(),
            content: BlockContent::System(msg.into()),
            minimized: false,
        }
    }
}

/// Focus state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Focus {
    /// Input line has focus
    Input,
    /// A block has focus (for minimize/remove)
    Block(BlockId),
}

/// Input line state
#[derive(Debug, Clone, Default)]
pub struct InputState {
    /// Current input buffer
    pub buffer: String,
    /// Cursor position (character index)
    pub cursor: usize,
    /// Command history
    pub history: Vec<String>,
    /// Current history navigation index (None = current input)
    pub history_index: Option<usize>,
    /// Saved current input when navigating history
    pub saved_input: String,
}

/// Application state
pub struct AppState {
    /// Output blocks (newest at end)
    pub output: Vec<OutputBlock>,
    /// Scroll offset from bottom (0 = at bottom)
    pub scroll_offset: usize,
    /// Input line state
    pub input: InputState,
    /// Current focus
    pub focus: Focus,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            output: Vec::new(),
            scroll_offset: 0,
            input: InputState::default(),
            focus: Focus::Input,
        }
    }
}

/// Main TUI application
pub struct App {
    /// Application state
    pub state: AppState,
    /// Shared REPL context
    pub ctx: SharedContext,
    /// Should quit flag
    pub should_quit: bool,
}

impl App {
    /// Create a new application
    pub fn new(ctx: SharedContext) -> Self {
        let mut state = AppState::default();

        // Add welcome message
        state.output.push(OutputBlock::system(
            "KQL Panopticon TUI Shell\nType 'help' for commands, 'exit' to quit."
        ));

        Self {
            state,
            ctx,
            should_quit: false,
        }
    }

    /// Run the main application loop
    pub async fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        while !self.should_quit {
            // Render UI
            terminal.draw(|frame| {
                ui::render(frame, &self.state, &self.ctx);
            })?;

            // Poll for events with timeout (for async updates)
            if event::poll(Duration::from_millis(100))? {
                if let event::Event::Key(key) = event::read()? {
                    self.handle_key_event(key).await;
                }
            }
        }

        Ok(())
    }

    /// Handle a key event
    async fn handle_key_event(&mut self, key: KeyEvent) {
        match self.state.focus {
            Focus::Input => self.handle_input_key(key).await,
            Focus::Block(id) => self.handle_block_key(key, id),
        }
    }

    /// Handle key event when input line has focus
    async fn handle_input_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            // Quit: Ctrl+D on empty line or Ctrl+C
            (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
                if self.state.input.buffer.is_empty() {
                    self.should_quit = true;
                }
            }
            (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                // Clear current input
                self.state.input.buffer.clear();
                self.state.input.cursor = 0;
            }

            // Clear screen: Ctrl+L
            (KeyModifiers::CONTROL, KeyCode::Char('l')) => {
                self.state.output.clear();
                self.state.scroll_offset = 0;
            }

            // Line editing: Ctrl+A (start), Ctrl+E (end), Ctrl+U (clear)
            (KeyModifiers::CONTROL, KeyCode::Char('a')) => {
                self.state.input.cursor = 0;
            }
            (KeyModifiers::CONTROL, KeyCode::Char('e')) => {
                self.state.input.cursor = self.state.input.buffer.len();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('u')) => {
                self.state.input.buffer.clear();
                self.state.input.cursor = 0;
            }

            // Enter: Execute command
            (_, KeyCode::Enter) => {
                self.execute_input().await;
            }

            // Character input
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                self.state.input.buffer.insert(self.state.input.cursor, c);
                self.state.input.cursor += 1;
            }

            // Backspace
            (_, KeyCode::Backspace) => {
                if self.state.input.cursor > 0 {
                    self.state.input.cursor -= 1;
                    self.state.input.buffer.remove(self.state.input.cursor);
                }
            }

            // Delete
            (_, KeyCode::Delete) => {
                if self.state.input.cursor < self.state.input.buffer.len() {
                    self.state.input.buffer.remove(self.state.input.cursor);
                }
            }

            // Cursor movement
            (_, KeyCode::Left) => {
                if self.state.input.cursor > 0 {
                    self.state.input.cursor -= 1;
                }
            }
            (_, KeyCode::Right) => {
                if self.state.input.cursor < self.state.input.buffer.len() {
                    self.state.input.cursor += 1;
                }
            }
            (_, KeyCode::Home) => {
                self.state.input.cursor = 0;
            }
            (_, KeyCode::End) => {
                self.state.input.cursor = self.state.input.buffer.len();
            }

            // History navigation
            (_, KeyCode::Up) => {
                self.history_prev();
            }
            (_, KeyCode::Down) => {
                self.history_next();
            }

            // Tab: Focus first block (for now, completion comes later)
            (_, KeyCode::Tab) => {
                if let Some(block) = self.state.output.last() {
                    self.state.focus = Focus::Block(block.id);
                }
            }

            // Page Up/Down for scrolling
            (_, KeyCode::PageUp) => {
                self.scroll_up(10);
            }
            (_, KeyCode::PageDown) => {
                self.scroll_down(10);
            }

            _ => {}
        }
    }

    /// Handle key event when a block has focus
    fn handle_block_key(&mut self, key: KeyEvent, block_id: BlockId) {
        match key.code {
            // Escape: Return to input
            KeyCode::Esc => {
                self.state.focus = Focus::Input;
            }

            // Minimize toggle
            KeyCode::Char('-') | KeyCode::Char('m') => {
                if let Some(block) = self.state.output.iter_mut().find(|b| b.id == block_id) {
                    block.minimized = !block.minimized;
                }
            }

            // Remove block
            KeyCode::Char('x') | KeyCode::Char('d') | KeyCode::Delete => {
                self.state.output.retain(|b| b.id != block_id);
                self.state.focus = Focus::Input;
            }

            // Navigate to previous block
            KeyCode::Up | KeyCode::Char('k') => {
                self.focus_prev_block(block_id);
            }

            // Navigate to next block
            KeyCode::Down | KeyCode::Char('j') => {
                self.focus_next_block(block_id);
            }

            // Tab: Return to input
            KeyCode::Tab => {
                self.state.focus = Focus::Input;
            }

            _ => {}
        }
    }

    /// Execute the current input as a command
    async fn execute_input(&mut self) {
        let input = self.state.input.buffer.trim().to_string();

        if input.is_empty() {
            return;
        }

        // Add to history
        if self.state.input.history.last() != Some(&input) {
            self.state.input.history.push(input.clone());
        }
        self.state.input.history_index = None;

        // Clear input
        self.state.input.buffer.clear();
        self.state.input.cursor = 0;

        // Echo command
        self.state.output.push(OutputBlock::command(&input));

        // Process line continuation
        let processed_input = join_continuation_lines(&input);

        // Parse command using shlex (handle quotes properly)
        let args = match shlex::split(&processed_input) {
            Some(args) => args,
            None => {
                self.state.output.push(OutputBlock::error("Invalid input syntax"));
                self.state.scroll_offset = 0;
                return;
            }
        };

        if args.is_empty() {
            self.state.scroll_offset = 0;
            return;
        }

        // Try to parse as clap command
        // Prepend empty string for program name (clap expects argv[0])
        let mut full_args = vec!["".to_string()];
        full_args.extend(args);

        match ReplCommand::try_parse_from(&full_args) {
            Ok(repl_cmd) => {
                // Execute the command
                match commands::execute(repl_cmd.command, self.ctx.clone()).await {
                    Ok(result) => self.handle_command_result(result),
                    Err(e) => {
                        self.state.output.push(OutputBlock::error(format!("Error: {}", e)));
                    }
                }
            }
            Err(e) => {
                // Clap error (help, version, or parse error)
                let error_str = e.to_string();
                // Check if it's help output (not really an error)
                if e.kind() == clap::error::ErrorKind::DisplayHelp
                    || e.kind() == clap::error::ErrorKind::DisplayVersion
                {
                    self.state.output.push(OutputBlock::text(error_str));
                } else {
                    self.state.output.push(OutputBlock::error(error_str));
                }
            }
        }

        // Auto-scroll to bottom
        self.state.scroll_offset = 0;
    }

    /// Handle a command result, converting it to output blocks
    fn handle_command_result(&mut self, result: CommandResult) {
        match result {
            CommandResult::Success(Some(msg)) => {
                self.state.output.push(OutputBlock::text(msg));
            }
            CommandResult::Success(None) => {
                // Silent success - no output
            }
            CommandResult::Output(msg) => {
                self.state.output.push(OutputBlock::text(msg));
            }
            CommandResult::Clear => {
                self.state.output.clear();
            }
            CommandResult::Exit => {
                self.should_quit = true;
            }
            CommandResult::Error(msg) => {
                self.state.output.push(OutputBlock::error(msg));
            }
        }
    }

    /// Navigate to previous command in history
    fn history_prev(&mut self) {
        if self.state.input.history.is_empty() {
            return;
        }

        match self.state.input.history_index {
            None => {
                // Save current input and go to last history item
                self.state.input.saved_input = self.state.input.buffer.clone();
                let idx = self.state.input.history.len() - 1;
                self.state.input.history_index = Some(idx);
                self.state.input.buffer = self.state.input.history[idx].clone();
                self.state.input.cursor = self.state.input.buffer.len();
            }
            Some(idx) if idx > 0 => {
                // Go to previous history item
                let new_idx = idx - 1;
                self.state.input.history_index = Some(new_idx);
                self.state.input.buffer = self.state.input.history[new_idx].clone();
                self.state.input.cursor = self.state.input.buffer.len();
            }
            _ => {}
        }
    }

    /// Navigate to next command in history
    fn history_next(&mut self) {
        match self.state.input.history_index {
            Some(idx) => {
                if idx + 1 < self.state.input.history.len() {
                    // Go to next history item
                    let new_idx = idx + 1;
                    self.state.input.history_index = Some(new_idx);
                    self.state.input.buffer = self.state.input.history[new_idx].clone();
                    self.state.input.cursor = self.state.input.buffer.len();
                } else {
                    // Return to saved input
                    self.state.input.history_index = None;
                    self.state.input.buffer = self.state.input.saved_input.clone();
                    self.state.input.cursor = self.state.input.buffer.len();
                }
            }
            None => {}
        }
    }

    /// Focus the previous block
    fn focus_prev_block(&mut self, current_id: BlockId) {
        let current_idx = self.state.output.iter().position(|b| b.id == current_id);
        if let Some(idx) = current_idx {
            if idx > 0 {
                self.state.focus = Focus::Block(self.state.output[idx - 1].id);
            }
        }
    }

    /// Focus the next block
    fn focus_next_block(&mut self, current_id: BlockId) {
        let current_idx = self.state.output.iter().position(|b| b.id == current_id);
        if let Some(idx) = current_idx {
            if idx + 1 < self.state.output.len() {
                self.state.focus = Focus::Block(self.state.output[idx + 1].id);
            } else {
                // At last block, return to input
                self.state.focus = Focus::Input;
            }
        }
    }

    /// Scroll up by n lines
    fn scroll_up(&mut self, n: usize) {
        let max_scroll = self.state.output.len().saturating_sub(1);
        self.state.scroll_offset = (self.state.scroll_offset + n).min(max_scroll);
    }

    /// Scroll down by n lines
    fn scroll_down(&mut self, n: usize) {
        self.state.scroll_offset = self.state.scroll_offset.saturating_sub(n);
    }
}
