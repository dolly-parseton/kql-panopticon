//! Main TUI application state and loop

use crate::commands::{self, Command, CommandResult, ExecutionContext, ReplCommand, run};
use crate::completion;
use crate::context::SharedContext;
use kql_panopticon_core::Client;
use crate::tui::events::{TuiEvent, TuiEventReceiver, TuiEventSender, WidgetEvent, WidgetState};
use crate::tui::ui;
use crate::tui::ui::completion_popup::CompletionPopup;
use crate::tui::widgets::editor::{EditorMode, EditorWidget};
use crate::tui::widgets::input_def::InputDefWidget;
use crate::tui::widgets::input_form::InputFormWidget;
use crate::tui::widgets::results::ResultsWidget;
use crate::tui::widgets::run_progress::RunProgressWidget;
use crate::tui::widgets::workspace_selector::WorkspaceSelectorWidget;
use crate::tui::widgets::{Widget, WidgetResult};
use crate::validator::join_continuation_lines;
use anyhow::Result;
use clap::Parser;
use crossterm::event::{self, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io::Stdout;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use uuid::Uuid;

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
    /// Interactive editor widget
    Editor(EditorWidget),
    /// Interactive results table widget
    Results(ResultsWidget),
    /// Interactive input form widget (for collecting values at run time)
    InputForm(InputFormWidget),
    /// Interactive input definition widget (for defining input parameters)
    InputDef(InputDefWidget),
    /// Run progress widget (for monitoring execution)
    RunProgress(RunProgressWidget),
    /// Workspace selector widget (for multi-select workspace selection)
    WorkspaceSelector(WorkspaceSelectorWidget),
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

    /// Create an editor widget block
    pub fn editor(widget: EditorWidget) -> Self {
        Self {
            id: next_block_id(),
            content: BlockContent::Editor(widget),
            minimized: false,
        }
    }

    /// Create a results widget block
    pub fn results(widget: ResultsWidget) -> Self {
        Self {
            id: next_block_id(),
            content: BlockContent::Results(widget),
            minimized: false,
        }
    }

    /// Create an input form widget block
    pub fn input_form(widget: InputFormWidget) -> Self {
        Self {
            id: next_block_id(),
            content: BlockContent::InputForm(widget),
            minimized: false,
        }
    }

    /// Create an input definition widget block
    pub fn input_def(widget: InputDefWidget) -> Self {
        Self {
            id: next_block_id(),
            content: BlockContent::InputDef(widget),
            minimized: false,
        }
    }

    /// Create a run progress widget block
    pub fn run_progress(widget: RunProgressWidget) -> Self {
        Self {
            id: next_block_id(),
            content: BlockContent::RunProgress(widget),
            minimized: false,
        }
    }

    /// Create a workspace selector widget block
    pub fn workspace_selector(widget: WorkspaceSelectorWidget) -> Self {
        Self {
            id: next_block_id(),
            content: BlockContent::WorkspaceSelector(widget),
            minimized: false,
        }
    }

    /// Check if this block contains an active widget
    pub fn is_widget(&self) -> bool {
        matches!(
            self.content,
            BlockContent::Editor(_) | BlockContent::Results(_) | BlockContent::InputForm(_) | BlockContent::InputDef(_) | BlockContent::RunProgress(_) | BlockContent::WorkspaceSelector(_)
        )
    }

    /// Get mutable reference to editor widget if this is an editor block
    pub fn as_editor_mut(&mut self) -> Option<&mut EditorWidget> {
        match &mut self.content {
            BlockContent::Editor(editor) => Some(editor),
            _ => None,
        }
    }

    /// Get mutable reference to results widget if this is a results block
    pub fn as_results_mut(&mut self) -> Option<&mut ResultsWidget> {
        match &mut self.content {
            BlockContent::Results(results) => Some(results),
            _ => None,
        }
    }

    /// Get mutable reference to input form widget if this is an input form block
    pub fn as_input_form_mut(&mut self) -> Option<&mut InputFormWidget> {
        match &mut self.content {
            BlockContent::InputForm(form) => Some(form),
            _ => None,
        }
    }

    /// Get mutable reference to input definition widget if this is an input def block
    pub fn as_input_def_mut(&mut self) -> Option<&mut InputDefWidget> {
        match &mut self.content {
            BlockContent::InputDef(def) => Some(def),
            _ => None,
        }
    }

    /// Get mutable reference to run progress widget if this is a progress block
    pub fn as_run_progress_mut(&mut self) -> Option<&mut RunProgressWidget> {
        match &mut self.content {
            BlockContent::RunProgress(progress) => Some(progress),
            _ => None,
        }
    }

    /// Get mutable reference to workspace selector widget if this is a selector block
    pub fn as_workspace_selector_mut(&mut self) -> Option<&mut WorkspaceSelectorWidget> {
        match &mut self.content {
            BlockContent::WorkspaceSelector(selector) => Some(selector),
            _ => None,
        }
    }
}

/// Focus state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Focus {
    /// Input line has focus
    Input,
    /// A static block has focus (for minimize/remove)
    Block(BlockId),
    /// An interactive widget has focus (captures all input)
    Widget(BlockId),
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
    /// Accumulated lines during line continuation
    pub continuation_lines: Vec<String>,
}

impl InputState {
    /// Check if we're in line continuation mode
    pub fn is_continuation(&self) -> bool {
        !self.continuation_lines.is_empty()
    }

    /// Get the full input including continuation lines
    pub fn full_input(&self) -> String {
        if self.continuation_lines.is_empty() {
            self.buffer.clone()
        } else {
            let mut result = self.continuation_lines.join("\n");
            result.push('\n');
            result.push_str(&self.buffer);
            result
        }
    }

    /// Clear continuation state
    pub fn clear_continuation(&mut self) {
        self.continuation_lines.clear();
    }
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
    /// Completion popup (if active)
    pub completion: Option<CompletionPopup>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            output: Vec::new(),
            scroll_offset: 0,
            input: InputState::default(),
            focus: Focus::Input,
            completion: None,
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
    /// Pending execution context (waiting for input form submission)
    pub pending_execution: Option<ExecutionContext>,
    /// Event sender for background tasks to send updates to the UI
    pub event_tx: TuiEventSender,
    /// Event receiver for receiving updates from background tasks
    event_rx: TuiEventReceiver,
    /// Active job manager for tracking running executions
    active_jobs: crate::tui::ActiveJobManager,
    /// Widget events log (for undo/redo support)
    widget_events: Vec<WidgetEvent>,
    /// Cached widget states before modifications (for undo)
    widget_state_cache: std::collections::HashMap<BlockId, WidgetState>,
    /// Undo/redo stack for widget operations
    undo_stack: crate::tui::UndoStack,
}

impl App {
    /// Create a new application
    pub async fn new(ctx: SharedContext) -> Self {
        let mut state = AppState::default();

        // Mark context as being in TUI mode and start discovery
        {
            let mut ctx_write = ctx.write().await;
            ctx_write.set_tui_mode(true);
            ctx_write.start_discovery();
        }

        // Create event channel for background task communication
        let (event_tx, event_rx) = crate::tui::events::channel();

        // Add welcome message
        state.output.push(OutputBlock::system(
            "KQL Panopticon TUI Shell\nType 'help' for commands, 'exit' to quit.\nDiscovering workspaces..."
        ));

        // Spawn background workspace discovery
        {
            let event_tx_clone = event_tx.clone();
            let ctx_clone = ctx.clone();
            tokio::spawn(async move {
                match discover_workspaces_background().await {
                    Ok((client, workspaces)) => {
                        let count = workspaces.len();
                        {
                            let mut ctx = ctx_clone.write().await;
                            ctx.complete_discovery(client, workspaces);
                        }
                        let _ = event_tx_clone.send(TuiEvent::DiscoveryComplete {
                            workspace_count: count,
                        });
                    }
                    Err(e) => {
                        {
                            let mut ctx = ctx_clone.write().await;
                            ctx.fail_discovery(e.to_string());
                        }
                        let _ = event_tx_clone.send(TuiEvent::DiscoveryFailed {
                            error: e.to_string(),
                        });
                    }
                }
            });
        }

        Self {
            state,
            ctx,
            should_quit: false,
            pending_execution: None,
            event_tx,
            event_rx,
            active_jobs: crate::tui::ActiveJobManager::new(),
            widget_events: Vec::new(),
            widget_state_cache: std::collections::HashMap::new(),
            undo_stack: crate::tui::UndoStack::new(),
        }
    }

    /// Emit a widget event (logs it and stores for undo)
    fn emit_widget_event(&mut self, event: WidgetEvent) {
        log::debug!("Widget event: {}", event.description());
        // Push to undo stack if it's an undoable event
        self.undo_stack.push_event(&event);
        self.widget_events.push(event);
    }

    /// Cache a widget's state before modification (for undo)
    fn cache_widget_state(&mut self, block_id: BlockId, state: WidgetState) {
        self.widget_state_cache.insert(block_id, state);
    }

    /// Get cached widget state
    fn get_cached_state(&mut self, block_id: BlockId) -> Option<WidgetState> {
        self.widget_state_cache.remove(&block_id)
    }

    /// Get widget events log
    pub fn widget_events(&self) -> &[WidgetEvent] {
        &self.widget_events
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    /// Get the description of the next undo operation
    pub fn next_undo_description(&self) -> Option<&str> {
        self.undo_stack.next_undo_description()
    }

    /// Get the description of the next redo operation
    pub fn next_redo_description(&self) -> Option<&str> {
        self.undo_stack.next_redo_description()
    }

    /// Set focus to a widget and emit Focused event
    fn focus_widget(&mut self, block_id: BlockId, widget_name: String) {
        self.state.focus = Focus::Widget(block_id);
        self.emit_widget_event(WidgetEvent::Focused {
            block_id: block_id.0,
            widget_name,
        });
    }

    /// Get a clone of the event sender (for passing to background tasks)
    pub fn event_sender(&self) -> TuiEventSender {
        self.event_tx.clone()
    }

    /// Run the main application loop
    pub async fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        while !self.should_quit {
            // Render UI
            terminal.draw(|frame| {
                ui::render(frame, &self.state, &self.ctx);
            })?;

            // Process any pending TuiEvents (non-blocking)
            while let Ok(tui_event) = self.event_rx.try_recv() {
                self.handle_tui_event(tui_event).await;
            }

            // Poll for terminal events with timeout (for async updates)
            if event::poll(Duration::from_millis(100))? {
                if let event::Event::Key(key) = event::read()? {
                    self.handle_key_event(key).await;
                }
            }
        }

        // Flush events on shutdown
        {
            let mut ctx = self.ctx.write().await;
            if let Err(e) = ctx.flush_events() {
                log::warn!("Failed to flush events on shutdown: {}", e);
            }
        }

        Ok(())
    }

    /// Handle a TuiEvent from a background task
    async fn handle_tui_event(&mut self, event: TuiEvent) {
        match event {
            TuiEvent::ExecutionStarted { job_id, pack_name, step_count } => {
                log::debug!("Execution started: {} ({} steps)", pack_name, step_count);
                // A RunProgressWidget should already exist for this job
                // Update it if needed
                if let Some(block_id) = self.active_jobs.get(&job_id).map(|j| j.block_id) {
                    if let Some(block) = self.state.output.iter_mut().find(|b| b.id == block_id) {
                        if let Some(_progress) = block.as_run_progress_mut() {
                            // Widget already exists, execution started
                        }
                    }
                }
            }

            TuiEvent::StepStarted { job_id, step_name, step_index } => {
                log::debug!("Step {} ({}) started", step_index, step_name);
                if let Some(block_id) = self.active_jobs.get(&job_id).map(|j| j.block_id) {
                    if let Some(block) = self.state.output.iter_mut().find(|b| b.id == block_id) {
                        if let Some(progress) = block.as_run_progress_mut() {
                            progress.step_started(step_index);
                        }
                    }
                }
            }

            TuiEvent::StepCompleted { job_id, step_name, step_index, row_count, duration_ms } => {
                log::debug!("Step {} ({}) completed: {} rows in {}ms", step_index, step_name, row_count, duration_ms);
                if let Some(block_id) = self.active_jobs.get(&job_id).map(|j| j.block_id) {
                    if let Some(block) = self.state.output.iter_mut().find(|b| b.id == block_id) {
                        if let Some(progress) = block.as_run_progress_mut() {
                            progress.step_completed(step_index, row_count, duration_ms);
                        }
                    }
                }
            }

            TuiEvent::StepFailed { job_id, step_name, step_index, error } => {
                log::debug!("Step {} ({}) failed: {}", step_index, step_name, error);
                if let Some(block_id) = self.active_jobs.get(&job_id).map(|j| j.block_id) {
                    if let Some(block) = self.state.output.iter_mut().find(|b| b.id == block_id) {
                        if let Some(progress) = block.as_run_progress_mut() {
                            progress.step_failed(step_index, &error);
                        }
                    }
                }
            }

            TuiEvent::ExecutionCompleted { job_id, success, total_duration_ms } => {
                log::debug!("Execution completed: success={}, duration={}ms", success, total_duration_ms);
                if let Some(block_id) = self.active_jobs.get(&job_id).map(|j| j.block_id) {
                    if let Some(block) = self.state.output.iter_mut().find(|b| b.id == block_id) {
                        if let Some(progress) = block.as_run_progress_mut() {
                            progress.execution_complete(success);
                        }
                    }
                }
                // Remove from active jobs
                self.active_jobs.complete(&job_id);
            }

            TuiEvent::ExecutionCancelled { job_id } => {
                log::debug!("Execution cancelled: {}", job_id);
                if let Some(block_id) = self.active_jobs.get(&job_id).map(|j| j.block_id) {
                    if let Some(block) = self.state.output.iter_mut().find(|b| b.id == block_id) {
                        if let Some(progress) = block.as_run_progress_mut() {
                            progress.execution_complete(false);
                        }
                    }
                }
                self.active_jobs.complete(&job_id);
            }

            TuiEvent::DiscoveryComplete { workspace_count } => {
                // Context already updated by background task
                self.state.output.push(OutputBlock::system(format!(
                    "\x1b[32m✓\x1b[0m Discovered {} workspace(s). Use 'workspace select' to choose.",
                    workspace_count
                )));
            }

            TuiEvent::DiscoveryFailed { error } => {
                // Context already updated by background task
                self.state.output.push(OutputBlock::error(format!(
                    "Workspace discovery failed: {}", error
                )));
            }

            TuiEvent::WorkspaceConnected { workspace_name } => {
                self.state.output.push(OutputBlock::system(format!(
                    "Workspace connected: {}", workspace_name
                )));
            }

            TuiEvent::WorkspaceDisconnected { workspace_name, reason } => {
                self.state.output.push(OutputBlock::error(format!(
                    "Workspace disconnected: {} - {}", workspace_name, reason
                )));
            }

            TuiEvent::Refresh => {
                // Just trigger a redraw (happens anyway)
            }

            TuiEvent::Notification { message, is_error } => {
                if is_error {
                    self.state.output.push(OutputBlock::error(message));
                } else {
                    self.state.output.push(OutputBlock::system(message));
                }
            }
        }
    }

    /// Register an active job with its progress widget
    /// Returns a cancellation token that can be used to cancel the job
    pub fn register_job(
        &mut self,
        job_id: Uuid,
        pack_name: impl Into<String>,
        workspace: impl Into<String>,
        block_id: BlockId,
    ) -> crate::tui::CancellationToken {
        let job = crate::tui::ActiveJob::new(job_id, pack_name, workspace, block_id);
        self.active_jobs.start_job(job)
    }

    /// Request cancellation of a job by block ID
    pub fn cancel_job_by_block(&self, block_id: BlockId) -> bool {
        self.active_jobs.cancel_by_block(block_id)
    }

    /// Check if a block has an active (running) job
    pub fn is_job_active(&self, block_id: BlockId) -> bool {
        self.active_jobs.get_by_block(block_id).is_some()
    }

    /// Handle a key event
    async fn handle_key_event(&mut self, key: KeyEvent) {
        match self.state.focus.clone() {
            Focus::Input => self.handle_input_key(key).await,
            Focus::Block(id) => self.handle_block_key(key, id),
            Focus::Widget(id) => self.handle_widget_key(key, id).await,
        }
    }

    /// Handle key event when a widget has focus
    async fn handle_widget_key(&mut self, key: KeyEvent, block_id: BlockId) {
        // First, capture state BEFORE handling the key (for undo support)
        let before_state = self.capture_widget_state(block_id);

        // Track deferred events and actions to avoid borrow conflicts
        enum DeferredAction {
            EmitEvent(WidgetEvent),
            HandleSaved(crate::tui::widgets::WidgetOutput, WidgetState),
            RemoveBlock,
            RemoveBlockWithMessage(String),
            ConvertToText(String),
            ClearPendingExecution,
            ReturnToInput,
            RequestJobCancellation,
        }

        let mut deferred: Vec<DeferredAction> = Vec::new();

        // Find the widget block and handle the key
        if let Some(block) = self.state.output.iter_mut().find(|b| b.id == block_id) {
            // Handle editor widget
            if let Some(editor) = block.as_editor_mut() {
                let widget_name = editor.name.clone();
                editor.handle_key(key);

                match editor.result() {
                    WidgetResult::Saved(output) => {
                        let after_state = WidgetState::from(editor as &EditorWidget);
                        deferred.push(DeferredAction::EmitEvent(WidgetEvent::Saved {
                            block_id: block_id.0,
                            before: before_state.clone(),
                            after: after_state.clone(),
                        }));
                        deferred.push(DeferredAction::HandleSaved(output, after_state));
                    }
                    WidgetResult::Cancelled => {
                        deferred.push(DeferredAction::EmitEvent(WidgetEvent::Cancelled {
                            block_id: block_id.0,
                            widget_name,
                        }));
                        deferred.push(DeferredAction::RemoveBlock);
                        deferred.push(DeferredAction::ReturnToInput);
                    }
                    WidgetResult::Pending => {}
                }
            }
            // Handle results widget
            else if let Some(results) = block.as_results_mut() {
                let widget_name = results.name.clone();
                let row_count = results.rows.len();
                results.handle_key(key);

                match results.result() {
                    WidgetResult::Cancelled => {
                        deferred.push(DeferredAction::EmitEvent(WidgetEvent::Blurred {
                            block_id: block_id.0,
                            widget_name: widget_name.clone(),
                        }));
                        deferred.push(DeferredAction::ConvertToText(format!("📊 {} ({} rows)", widget_name, row_count)));
                        deferred.push(DeferredAction::ReturnToInput);
                    }
                    WidgetResult::Pending => {}
                    _ => {}
                }
            }
            // Handle input form widget
            else if let Some(form) = block.as_input_form_mut() {
                let widget_name = form.name.clone();
                form.handle_key(key);

                match form.result() {
                    WidgetResult::Saved(output) => {
                        let after_state = WidgetState::from(form as &InputFormWidget);
                        deferred.push(DeferredAction::EmitEvent(WidgetEvent::Saved {
                            block_id: block_id.0,
                            before: before_state.clone(),
                            after: after_state.clone(),
                        }));
                        deferred.push(DeferredAction::HandleSaved(output, after_state));
                    }
                    WidgetResult::Cancelled => {
                        deferred.push(DeferredAction::EmitEvent(WidgetEvent::Cancelled {
                            block_id: block_id.0,
                            widget_name,
                        }));
                        deferred.push(DeferredAction::RemoveBlock);
                        deferred.push(DeferredAction::ClearPendingExecution);
                        deferred.push(DeferredAction::RemoveBlockWithMessage("Cancelled".to_string()));
                        deferred.push(DeferredAction::ReturnToInput);
                    }
                    WidgetResult::Pending => {}
                }
            }
            // Handle input definition widget
            else if let Some(def_widget) = block.as_input_def_mut() {
                let widget_name = def_widget.name.clone();
                def_widget.handle_key(key);

                match def_widget.result() {
                    WidgetResult::Saved(output) => {
                        let after_state = WidgetState::from(def_widget as &InputDefWidget);
                        deferred.push(DeferredAction::EmitEvent(WidgetEvent::Saved {
                            block_id: block_id.0,
                            before: before_state.clone(),
                            after: after_state.clone(),
                        }));
                        deferred.push(DeferredAction::HandleSaved(output, after_state));
                    }
                    WidgetResult::Cancelled => {
                        deferred.push(DeferredAction::EmitEvent(WidgetEvent::Cancelled {
                            block_id: block_id.0,
                            widget_name,
                        }));
                        deferred.push(DeferredAction::RemoveBlock);
                        deferred.push(DeferredAction::RemoveBlockWithMessage("Cancelled".to_string()));
                        deferred.push(DeferredAction::ReturnToInput);
                    }
                    WidgetResult::Pending => {}
                }
            }
            // Handle run progress widget
            else if let Some(progress) = block.as_run_progress_mut() {
                let widget_name = progress.name.clone();
                let was_cancellation_requested_before = progress.is_cancellation_requested();
                progress.handle_key(key);

                // Check if cancellation was just requested
                if progress.is_cancellation_requested() && !was_cancellation_requested_before {
                    deferred.push(DeferredAction::RequestJobCancellation);
                }

                match progress.result() {
                    WidgetResult::Cancelled => {
                        deferred.push(DeferredAction::EmitEvent(WidgetEvent::Blurred {
                            block_id: block_id.0,
                            widget_name: widget_name.clone(),
                        }));
                        deferred.push(DeferredAction::ConvertToText(format!("Execution complete: {}", widget_name)));
                        deferred.push(DeferredAction::ReturnToInput);
                    }
                    WidgetResult::Pending => {}
                    _ => {}
                }
            }
            // Handle workspace selector widget
            else if let Some(selector) = block.as_workspace_selector_mut() {
                let widget_name = selector.name.clone();
                selector.handle_key(key);

                match selector.result() {
                    WidgetResult::Saved(output) => {
                        let after_state = WidgetState::from(selector as &WorkspaceSelectorWidget);
                        deferred.push(DeferredAction::EmitEvent(WidgetEvent::Saved {
                            block_id: block_id.0,
                            before: before_state.clone(),
                            after: after_state.clone(),
                        }));
                        deferred.push(DeferredAction::HandleSaved(output, after_state));
                    }
                    WidgetResult::Cancelled => {
                        deferred.push(DeferredAction::EmitEvent(WidgetEvent::Cancelled {
                            block_id: block_id.0,
                            widget_name,
                        }));
                        deferred.push(DeferredAction::RemoveBlock);
                        deferred.push(DeferredAction::ReturnToInput);
                    }
                    WidgetResult::Pending => {}
                }
            }
        }

        // Process deferred actions after the mutable borrow ends
        for action in deferred {
            match action {
                DeferredAction::EmitEvent(event) => {
                    self.emit_widget_event(event);
                }
                DeferredAction::HandleSaved(output, _after_state) => {
                    self.handle_widget_saved(block_id, output).await;
                }
                DeferredAction::RemoveBlock => {
                    self.state.output.retain(|b| b.id != block_id);
                }
                DeferredAction::RemoveBlockWithMessage(msg) => {
                    self.state.output.push(OutputBlock::system(msg));
                }
                DeferredAction::ConvertToText(text) => {
                    if let Some(block) = self.state.output.iter_mut().find(|b| b.id == block_id) {
                        block.content = BlockContent::Text(text);
                    }
                }
                DeferredAction::ClearPendingExecution => {
                    self.pending_execution = None;
                }
                DeferredAction::ReturnToInput => {
                    self.state.focus = Focus::Input;
                }
                DeferredAction::RequestJobCancellation => {
                    if self.cancel_job_by_block(block_id) {
                        log::debug!("Cancellation requested for job in block {:?}", block_id);
                    }
                }
            }
        }
    }

    /// Capture the current state of a widget block
    fn capture_widget_state(&self, block_id: BlockId) -> Option<WidgetState> {
        self.state.output.iter().find(|b| b.id == block_id).and_then(|block| {
            match &block.content {
                BlockContent::Editor(editor) => Some(WidgetState::from(editor)),
                BlockContent::InputForm(form) => Some(WidgetState::from(form)),
                BlockContent::InputDef(def) => Some(WidgetState::from(def)),
                BlockContent::Results(results) => Some(WidgetState::from(results)),
                BlockContent::RunProgress(progress) => Some(WidgetState::from(progress)),
                BlockContent::WorkspaceSelector(selector) => Some(WidgetState::from(selector)),
                _ => None,
            }
        })
    }

    /// Handle a saved widget result
    async fn handle_widget_saved(&mut self, block_id: BlockId, output: crate::tui::widgets::WidgetOutput) {
        use crate::session::StepDef;
        use crate::tui::widgets::WidgetOutput;

        match output {
            WidgetOutput::EditorContent { name, content, is_new } => {
                let mut ctx = self.ctx.write().await;

                if is_new {
                    // Create a new step
                    let step = StepDef::new(name.clone(), content);
                    let deps = step.depends_on.clone();
                    let refs = step.references_inputs.clone();

                    ctx.commit_state(format!("query {}", name));
                    ctx.pack_session_mut().add_step(step);

                    // Build response
                    let mut response = format!("✓ Step defined: {}", name);
                    if !refs.is_empty() {
                        response.push_str(&format!("\n  refs: [{}]", refs.join(", ")));
                    }
                    if !deps.is_empty() {
                        response.push_str(&format!("\n  depends: [{}]", deps.join(", ")));
                    }

                    if let Some(block) = self.state.output.iter_mut().find(|b| b.id == block_id) {
                        block.content = BlockContent::Text(response);
                    }
                } else {
                    // Update existing step
                    ctx.commit_state(format!("edit {}", name));
                    ctx.pack_session_mut().update_step_query(&name, &content);

                    if let Some(block) = self.state.output.iter_mut().find(|b| b.id == block_id) {
                        block.content = BlockContent::Text(format!("✓ Updated step: {}", name));
                    }
                }

                self.state.focus = Focus::Input;
            }
            WidgetOutput::FormValues(values) => {
                // Remove the form block
                self.state.output.retain(|b| b.id != block_id);

                // Check if we have a pending execution to continue
                if let Some(context) = self.pending_execution.take() {
                    // Get workspace name for progress display
                    let workspace_name = {
                        let ctx = self.ctx.read().await;
                        ctx.selected_workspaces()
                            .first()
                            .map(|w| w.name.clone())
                            .unwrap_or_else(|| "All Workspaces".to_string())
                    };

                    // Get pack name and step names for progress widget
                    let pack_name = context.pack.name.clone();
                    let step_names: Vec<String> = context.pack.steps
                        .iter()
                        .map(|s| s.name.clone())
                        .collect();

                    // Create RunProgressWidget
                    let progress_widget = RunProgressWidget::new(&pack_name, &workspace_name)
                        .with_steps(&step_names);
                    let progress_block = OutputBlock::run_progress(progress_widget);
                    let progress_block_id = progress_block.id;
                    self.state.output.push(progress_block);

                    // Spawn background execution with real-time progress events
                    match run::spawn_tui_execution(
                        context.pack,
                        context.pack_path,
                        values,
                        context.all_workspaces,
                        self.ctx.clone(),
                        self.event_tx.clone(),
                    ).await {
                        Ok((job_id, _step_names)) => {
                            // Register job with ActiveJobManager
                            let job = crate::tui::ActiveJob::new(
                                job_id,
                                pack_name,
                                workspace_name,
                                progress_block_id,
                            );
                            self.active_jobs.start_job(job);
                        }
                        Err(e) => {
                            // Remove progress widget and show error
                            self.state.output.retain(|b| b.id != progress_block_id);
                            self.state.output.push(OutputBlock::error(format!("Execution error: {}", e)));
                        }
                    }
                }

                self.state.focus = Focus::Input;
            }
            WidgetOutput::Selection(_selection) => {
                // Generic selection (not used currently)
                self.state.focus = Focus::Input;
            }
            WidgetOutput::WorkspaceSelection(workspace_ids) => {
                // Remove the widget block
                self.state.output.retain(|b| b.id != block_id);

                // Apply the workspace selection
                let count = workspace_ids.len();
                {
                    let mut ctx = self.ctx.write().await;
                    ctx.set_workspace_selection_by_ids(&workspace_ids);
                }

                // Show confirmation message
                if count == 1 {
                    // Get the workspace name for a single selection
                    let name = {
                        let ctx = self.ctx.read().await;
                        ctx.selected_workspaces()
                            .first()
                            .map(|w| w.name.clone())
                            .unwrap_or_else(|| "workspace".to_string())
                    };
                    self.state.output.push(OutputBlock::text(format!(
                        "\x1b[32m✓\x1b[0m Selected workspace: {}",
                        name
                    )));
                } else {
                    self.state.output.push(OutputBlock::text(format!(
                        "\x1b[32m✓\x1b[0m Selected {} workspaces",
                        count
                    )));
                }

                self.state.focus = Focus::Input;
            }
            WidgetOutput::InputDef(input_def) => {
                // Remove the widget block
                self.state.output.retain(|b| b.id != block_id);

                // Save the input definition
                let name = input_def.name.clone();
                let type_str = input_def.input_type.to_string();
                let req_str = if input_def.required { "required" } else { "optional" };

                {
                    let mut ctx = self.ctx.write().await;
                    ctx.commit_state(format!("input {}", name));
                    ctx.pack_session_mut().add_input(input_def);
                }

                self.state.output.push(OutputBlock::text(format!(
                    "\x1b[32m✓\x1b[0m Input defined: {} ({}, {})",
                    name, type_str, req_str
                )));

                self.state.focus = Focus::Input;
            }
        }
    }

    /// Handle key event when input line has focus
    async fn handle_input_key(&mut self, key: KeyEvent) {
        // Check if completion popup is active
        if self.state.completion.is_some() {
            if self.handle_completion_key(key).await {
                return; // Key was handled by completion
            }
        }

        match (key.modifiers, key.code) {
            // Quit: Ctrl+D on empty line or Ctrl+C
            (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
                if self.state.input.buffer.is_empty() {
                    self.should_quit = true;
                }
            }
            (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                // Clear current input and cancel continuation
                self.state.input.buffer.clear();
                self.state.input.cursor = 0;
                self.state.input.clear_continuation();
                self.state.completion = None;
            }

            // Clear screen: Ctrl+L
            (KeyModifiers::CONTROL, KeyCode::Char('l')) => {
                self.state.output.clear();
                self.state.scroll_offset = 0;
                self.state.completion = None;
            }

            // Line editing: Ctrl+A (start), Ctrl+E (end), Ctrl+U (clear)
            (KeyModifiers::CONTROL, KeyCode::Char('a')) => {
                self.state.input.cursor = 0;
                self.state.completion = None;
            }
            (KeyModifiers::CONTROL, KeyCode::Char('e')) => {
                self.state.input.cursor = self.state.input.buffer.len();
                self.state.completion = None;
            }
            (KeyModifiers::CONTROL, KeyCode::Char('u')) => {
                self.state.input.buffer.clear();
                self.state.input.cursor = 0;
                self.state.input.clear_continuation();
                self.state.completion = None;
            }

            // Enter: Execute command
            (_, KeyCode::Enter) => {
                self.execute_input().await;
            }

            // Character input
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                self.state.input.buffer.insert(self.state.input.cursor, c);
                self.state.input.cursor += 1;
                self.state.completion = None;
            }

            // Backspace
            (_, KeyCode::Backspace) => {
                if self.state.input.cursor > 0 {
                    self.state.input.cursor -= 1;
                    self.state.input.buffer.remove(self.state.input.cursor);
                }
                self.state.completion = None;
            }

            // Delete
            (_, KeyCode::Delete) => {
                if self.state.input.cursor < self.state.input.buffer.len() {
                    self.state.input.buffer.remove(self.state.input.cursor);
                }
                self.state.completion = None;
            }

            // Cursor movement
            (_, KeyCode::Left) => {
                if self.state.input.cursor > 0 {
                    self.state.input.cursor -= 1;
                }
                self.state.completion = None;
            }
            (_, KeyCode::Right) => {
                if self.state.input.cursor < self.state.input.buffer.len() {
                    self.state.input.cursor += 1;
                }
                self.state.completion = None;
            }
            (_, KeyCode::Home) => {
                self.state.input.cursor = 0;
                self.state.completion = None;
            }
            (_, KeyCode::End) => {
                self.state.input.cursor = self.state.input.buffer.len();
                self.state.completion = None;
            }

            // History navigation
            (_, KeyCode::Up) => {
                self.history_prev();
                self.state.completion = None;
            }
            (_, KeyCode::Down) => {
                self.history_next();
                self.state.completion = None;
            }

            // Tab: Trigger completion
            (KeyModifiers::NONE, KeyCode::Tab) => {
                self.trigger_completion().await;
            }

            // Shift+Tab: Focus first block (no completion context)
            (KeyModifiers::SHIFT, KeyCode::BackTab) => {
                if let Some(block) = self.state.output.first() {
                    self.state.focus = Focus::Block(block.id);
                }
                self.state.completion = None;
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

    /// Handle key event when completion popup is active
    /// Returns true if the key was handled
    async fn handle_completion_key(&mut self, key: KeyEvent) -> bool {
        let completion = match &mut self.state.completion {
            Some(c) => c,
            None => return false,
        };

        match (key.modifiers, key.code) {
            // Escape: Dismiss completion
            (_, KeyCode::Esc) => {
                self.state.completion = None;
                true
            }

            // Up: Select previous suggestion
            (_, KeyCode::Up) => {
                completion.select_prev();
                true
            }

            // Down: Select next suggestion
            (_, KeyCode::Down) => {
                completion.select_next();
                true
            }

            // Tab or Enter: Accept selected suggestion
            (KeyModifiers::NONE, KeyCode::Tab) | (_, KeyCode::Enter) => {
                self.accept_completion();
                true
            }

            // Any other key: dismiss and let normal handling proceed
            _ => {
                self.state.completion = None;
                false
            }
        }
    }

    /// Trigger completion for current input
    async fn trigger_completion(&mut self) {
        let ctx = self.ctx.read().await;
        let suggestions = completion::get_completions(
            &self.state.input.buffer,
            self.state.input.cursor,
            &ctx,
        );

        if suggestions.is_empty() {
            self.state.completion = None;
        } else {
            self.state.completion = Some(CompletionPopup::new(suggestions));
        }
    }

    /// Accept the currently selected completion
    fn accept_completion(&mut self) {
        let completion = match self.state.completion.take() {
            Some(c) => c,
            None => return,
        };

        let suggestion = match completion.selected_suggestion() {
            Some(s) => s.clone(),
            None => return,
        };

        // Replace the text at the suggestion's span
        let before = &self.state.input.buffer[..suggestion.replace_start];
        let after = &self.state.input.buffer[suggestion.replace_end..];
        self.state.input.buffer = format!("{}{}{}", before, suggestion.value, after);
        self.state.input.cursor = suggestion.replace_start + suggestion.value.len();
    }

    /// Handle key event when a block has focus
    fn handle_block_key(&mut self, key: KeyEvent, block_id: BlockId) {
        match (key.modifiers, key.code) {
            // Escape: Return to input
            (_, KeyCode::Esc) => {
                self.state.focus = Focus::Input;
            }

            // Enter: Expand if minimized, otherwise return to input
            (_, KeyCode::Enter) => {
                if let Some(block) = self.state.output.iter_mut().find(|b| b.id == block_id) {
                    if block.minimized {
                        block.minimized = false;
                    } else {
                        self.state.focus = Focus::Input;
                    }
                }
            }

            // Space: Toggle minimize
            (_, KeyCode::Char(' ')) => {
                if let Some(block) = self.state.output.iter_mut().find(|b| b.id == block_id) {
                    block.minimized = !block.minimized;
                }
            }

            // Minimize toggle (- or m)
            (_, KeyCode::Char('-')) | (_, KeyCode::Char('m')) => {
                if let Some(block) = self.state.output.iter_mut().find(|b| b.id == block_id) {
                    block.minimized = !block.minimized;
                }
            }

            // Remove block (x, d, or Delete)
            (_, KeyCode::Char('x')) | (_, KeyCode::Char('d')) | (_, KeyCode::Delete) => {
                let idx = self.state.output.iter().position(|b| b.id == block_id);
                self.state.output.retain(|b| b.id != block_id);
                // Focus next block or input
                if let Some(idx) = idx {
                    if idx < self.state.output.len() {
                        self.state.focus = Focus::Block(self.state.output[idx].id);
                    } else if idx > 0 && !self.state.output.is_empty() {
                        self.state.focus = Focus::Block(self.state.output[idx - 1].id);
                    } else {
                        self.state.focus = Focus::Input;
                    }
                } else {
                    self.state.focus = Focus::Input;
                }
            }

            // Navigate to previous block (Up, k)
            (_, KeyCode::Up) | (_, KeyCode::Char('k')) => {
                self.focus_prev_block(block_id);
            }

            // Navigate to next block (Down, j)
            (_, KeyCode::Down) | (_, KeyCode::Char('j')) => {
                self.focus_next_block(block_id);
            }

            // Tab: Focus next block or return to input
            (KeyModifiers::NONE, KeyCode::Tab) => {
                self.focus_next_block(block_id);
            }

            // Shift+Tab: Focus previous block or return to input
            (KeyModifiers::SHIFT, KeyCode::BackTab) => {
                self.focus_prev_block(block_id);
            }

            _ => {}
        }
    }

    /// Check if input needs line continuation (ends with unescaped backslash)
    fn needs_continuation(input: &str) -> bool {
        let trimmed = input.trim_end();
        if trimmed.ends_with('\\') {
            // Count consecutive backslashes at the end
            let backslash_count = trimmed
                .chars()
                .rev()
                .take_while(|&c| c == '\\')
                .count();
            // Odd number means line continuation
            backslash_count % 2 == 1
        } else {
            false
        }
    }

    /// Execute the current input as a command
    async fn execute_input(&mut self) {
        let current_line = self.state.input.buffer.trim().to_string();

        // Handle empty input
        if current_line.is_empty() && !self.state.input.is_continuation() {
            return;
        }

        // Check for line continuation
        if Self::needs_continuation(&current_line) {
            // Add to continuation buffer
            self.state.input.continuation_lines.push(current_line);
            self.state.input.buffer.clear();
            self.state.input.cursor = 0;
            return;
        }

        // Get the full input (may include continuation lines)
        let full_input = if self.state.input.is_continuation() {
            self.state.input.continuation_lines.push(current_line.clone());
            self.state.input.continuation_lines.join("\n")
        } else {
            current_line.clone()
        };

        // Add to history (the full command)
        if self.state.input.history.last() != Some(&full_input) {
            self.state.input.history.push(full_input.clone());
        }
        self.state.input.history_index = None;

        // Clear input and continuation state
        self.state.input.buffer.clear();
        self.state.input.cursor = 0;
        self.state.input.clear_continuation();

        // Process line continuation (joins backslash-continued lines)
        let processed_input = join_continuation_lines(&full_input);

        // Echo the processed command (cleaner, no continuation markers)
        self.state.output.push(OutputBlock::command(&processed_input));

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
                    Ok(result) => self.handle_command_result(result).await,
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
    async fn handle_command_result(&mut self, result: CommandResult) {
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
            CommandResult::EditStep { name, content } => {
                // Create an embedded editor widget for editing an existing step
                let mut editor = EditorWidget::new(&name, EditorMode::Kql)
                    .with_content(&content);
                editor.is_new = false; // Editing existing step
                let widget_name = name.clone();
                let block = OutputBlock::editor(editor);
                let block_id = block.id;
                self.state.output.push(block);
                self.focus_widget(block_id, widget_name);
            }
            CommandResult::NewStep { name } => {
                // Create an embedded editor widget for a new step
                let mut editor = EditorWidget::new(&name, EditorMode::Kql);
                editor.is_new = true; // Creating new step
                let widget_name = name.clone();
                let block = OutputBlock::editor(editor);
                let block_id = block.id;
                self.state.output.push(block);
                self.focus_widget(block_id, widget_name);
            }
            CommandResult::EditInput { name, content } => {
                // Create an embedded editor widget for the input (YAML mode)
                let editor = EditorWidget::new(&name, EditorMode::Yaml)
                    .with_content(&content);
                let widget_name = name.clone();
                let block = OutputBlock::editor(editor);
                let block_id = block.id;
                self.state.output.push(block);
                self.focus_widget(block_id, widget_name);
            }
            CommandResult::ViewResults { name, columns, rows } => {
                // Create a results widget to display step output
                use crate::tui::widgets::results::ResultsWidget;
                let results = ResultsWidget::new(&name).with_data(columns, rows);
                let widget_name = name.clone();
                let block = OutputBlock::results(results);
                let block_id = block.id;
                self.state.output.push(block);
                self.focus_widget(block_id, widget_name);
            }
            CommandResult::InputsRequired { inputs, context } => {
                // Store execution context for when form is submitted
                self.pending_execution = Some(context);

                // Create an input form widget
                let form = InputFormWidget::new("Enter Input Values")
                    .with_inputs(&inputs);
                let widget_name = "Enter Input Values".to_string();
                let block = OutputBlock::input_form(form);
                let block_id = block.id;
                self.state.output.push(block);
                self.focus_widget(block_id, widget_name);
            }
            CommandResult::DefineInput { name, existing } => {
                // Create an input definition widget
                let widget = match existing {
                    Some(input_def) => InputDefWidget::from_existing(&input_def),
                    None => InputDefWidget::new(&name),
                };
                let widget_name = name.clone();
                let block = OutputBlock::input_def(widget);
                let block_id = block.id;
                self.state.output.push(block);
                self.focus_widget(block_id, widget_name);
            }
            CommandResult::ExecutionComplete { summary, step_results } => {
                // Add summary text block
                self.state.output.push(OutputBlock::text(summary));

                // Add brief step results info (use `peek <step>` to view full data)
                for step_data in step_results {
                    if step_data.success {
                        if step_data.row_count > 0 {
                            self.state.output.push(OutputBlock::text(format!(
                                "  ✓ {} ({} rows)", step_data.name, step_data.row_count
                            )));
                        }
                    } else {
                        let error_msg = step_data.error.unwrap_or_else(|| "Unknown error".to_string());
                        self.state.output.push(OutputBlock::error(format!(
                            "  ✗ {} - {}", step_data.name, error_msg
                        )));
                    }
                }

                // Stay on input focus - user can use `peek <step>` to view results
            }
            CommandResult::StartExecution { context, inputs } => {
                // Start execution immediately without showing a form
                // Get workspace name for progress display
                let workspace_name = {
                    let ctx = self.ctx.read().await;
                    ctx.selected_workspaces()
                        .first()
                        .map(|w| w.name.clone())
                        .unwrap_or_else(|| "All Workspaces".to_string())
                };

                // Get pack name and step names for progress widget
                let pack_name = context.pack.name.clone();
                let step_names: Vec<String> = context.pack.steps
                    .iter()
                    .map(|s| s.name.clone())
                    .collect();

                // Create RunProgressWidget
                let progress_widget = RunProgressWidget::new(&pack_name, &workspace_name)
                    .with_steps(&step_names);
                let progress_block = OutputBlock::run_progress(progress_widget);
                let progress_block_id = progress_block.id;
                self.state.output.push(progress_block);

                // Spawn background execution with real-time progress events
                match run::spawn_tui_execution(
                    context.pack,
                    context.pack_path,
                    inputs,
                    context.all_workspaces,
                    self.ctx.clone(),
                    self.event_tx.clone(),
                ).await {
                    Ok((job_id, _step_names)) => {
                        // Register job with ActiveJobManager
                        let job = crate::tui::ActiveJob::new(
                            job_id,
                            pack_name,
                            workspace_name,
                            progress_block_id,
                        );
                        self.active_jobs.start_job(job);
                    }
                    Err(e) => {
                        // Remove progress widget and show error
                        self.state.output.retain(|b| b.id != progress_block_id);
                        self.state.output.push(OutputBlock::error(format!("Execution error: {}", e)));
                    }
                }
            }
            CommandResult::SelectWorkspaces => {
                // Create workspace selector widget
                let (workspaces, selected_ids) = {
                    let ctx = self.ctx.read().await;
                    let workspaces = ctx.available_workspaces().to_vec();
                    let selected_ids: std::collections::HashSet<String> = ctx
                        .selected_workspaces()
                        .iter()
                        .map(|w| w.workspace_id.clone())
                        .collect();
                    (workspaces, selected_ids)
                };

                if workspaces.is_empty() {
                    self.state.output.push(OutputBlock::error(
                        "No workspaces available. Run 'workspace list' first."
                    ));
                } else {
                    let widget = WorkspaceSelectorWidget::new("Select Workspaces")
                        .with_workspaces(&workspaces, &selected_ids);
                    let widget_name = "Select Workspaces".to_string();
                    let block = OutputBlock::workspace_selector(widget);
                    let block_id = block.id;
                    self.state.output.push(block);
                    self.focus_widget(block_id, widget_name);
                }
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

    /// Focus the previous block (or return to input if at first block)
    fn focus_prev_block(&mut self, current_id: BlockId) {
        let current_idx = self.state.output.iter().position(|b| b.id == current_id);
        if let Some(idx) = current_idx {
            if idx > 0 {
                self.state.focus = Focus::Block(self.state.output[idx - 1].id);
            } else {
                // At first block, return to input
                self.state.focus = Focus::Input;
            }
        }
    }

    /// Focus the next block (or return to input if at last block)
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

/// Discover workspaces in the background
async fn discover_workspaces_background() -> anyhow::Result<(Client, Vec<kql_panopticon_core::Workspace>)> {
    let client = Client::new()?;
    let workspaces = client.list_workspaces().await?;
    Ok((client, workspaces))
}
