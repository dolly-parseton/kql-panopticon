use crate::query_job::{QueryJobBuilder, QueryJobResult, QuerySettings};
use crate::tui::message::{Message, Tab};
use crate::tui::model::{query::EditorMode, Model, Popup};
use log::error;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

/// Sanitize a string to be safe for use as a filename
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_whitespace() => '-',
            c if c.is_alphanumeric() || c == '-' || c == '_' => c,
            _ => '-',
        })
        .collect::<String>()
        .trim_matches('-')
        .to_lowercase()
}

/// Create a failed QueryJobResult for when execution fails
fn create_failed_result(
    retry_ctx: crate::tui::model::jobs::RetryContext,
    error_msg: String,
) -> QueryJobResult {
    QueryJobResult {
        workspace_id: retry_ctx.workspace.workspace_id.clone(),
        workspace_name: retry_ctx.workspace.name.clone(),
        query: retry_ctx.query,
        result: Err(crate::error::KqlPanopticonError::Other(error_msg)),
        elapsed: Duration::from_secs(0),
        timestamp: chrono::Local::now(),
    }
}

/// Update the model based on a message
/// Returns a list of additional messages to process
pub fn update(model: &mut Model, message: Message) -> Vec<Message> {
    match message {
        // === Navigation ===
        Message::SwitchTab(tab) => {
            model.current_tab = tab;
            // Clear any editing/popup state when switching tabs
            if model.current_tab != Tab::Settings {
                model.settings.editing = None;
            }
            if model.current_tab != Tab::Query {
                model.query.mode = EditorMode::Normal;
            }
            model.popup = None;
            vec![]
        }

        Message::Quit => {
            // Quit is handled in the main loop
            vec![]
        }

        // === Settings ===
        Message::SettingsPrevious => {
            if model.settings.selected_index > 0 {
                model.settings.selected_index -= 1;
                model
                    .settings
                    .list_state
                    .select(Some(model.settings.selected_index));
            }
            vec![]
        }

        Message::SettingsNext => {
            if model.settings.selected_index < 6 {
                model.settings.selected_index += 1;
                model
                    .settings
                    .list_state
                    .select(Some(model.settings.selected_index));
            }
            vec![]
        }

        Message::SettingsStartEdit => {
            // For toggle settings, toggle them directly instead of showing edit popup
            if model.settings.is_selected_toggle() {
                model.settings.toggle_selected();
                vec![]
            } else {
                let current_value = model.settings.get_selected_value();
                model.settings.editing = Some(current_value);
                model.popup = Some(Popup::SettingsEdit);
                vec![]
            }
        }

        Message::SettingsInputChar(c) => {
            if let Some(ref mut input) = model.settings.editing {
                input.push(c);
            }
            vec![]
        }

        Message::SettingsInputBackspace => {
            if let Some(ref mut input) = model.settings.editing {
                input.pop();
            }
            vec![]
        }

        Message::SettingsSave => {
            if let Some(value) = model.settings.editing.take() {
                if !value.trim().is_empty() {
                    match model.settings.save_edit(value) {
                        Ok(()) => {
                            model.popup = None;
                            // Rebuild client with new settings (timeout, retry_count, validation_interval)
                            if let Err(e) = model.rebuild_client() {
                                return vec![Message::ShowError(format!(
                                    "Failed to update client settings: {}",
                                    e
                                ))];
                            }
                            // Mark session as dirty when settings change
                            model.sessions.mark_dirty();
                            vec![]
                        }
                        Err(err_msg) => {
                            model.popup = None;
                            vec![Message::ShowError(err_msg)]
                        }
                    }
                } else {
                    model.popup = None;
                    vec![]
                }
            } else {
                vec![]
            }
        }

        Message::SettingsCancel => {
            model.settings.editing = None;
            model.popup = None;
            vec![]
        }

        // === Workspaces ===
        Message::WorkspacesPrevious => {
            let selected = model.workspaces.table_state.selected().unwrap_or(0);
            if selected > 0 {
                model.workspaces.table_state.select(Some(selected - 1));
            }
            vec![]
        }

        Message::WorkspacesNext => {
            let selected = model.workspaces.table_state.selected().unwrap_or(0);
            let max = model.workspaces.workspaces.len().saturating_sub(1);
            if selected < max {
                model.workspaces.table_state.select(Some(selected + 1));
            }
            vec![]
        }

        Message::WorkspacesToggle => {
            if let Some(selected) = model.workspaces.table_state.selected() {
                model.workspaces.toggle_selection(selected);
            }
            vec![]
        }

        Message::WorkspacesSelectAll => {
            model.workspaces.select_all();
            vec![]
        }

        Message::WorkspacesSelectNone => {
            model.workspaces.select_none();
            vec![]
        }

        Message::WorkspacesRefresh => {
            // This will be handled asyncronously in the main loop
            // The main loop will detect this message and trigger an async operation
            vec![]
        }

        Message::WorkspacesLoaded(workspaces) => {
            model.workspaces.load_workspaces(workspaces);
            vec![]
        }

        // === Query ===
        Message::QueryEnterInsertMode => {
            model.query.mode = EditorMode::Insert;
            vec![]
        }

        Message::QueryExitInsertMode => {
            model.query.mode = EditorMode::Normal;
            vec![]
        }

        Message::QueryEnterVisualMode => {
            model.query.textarea.start_selection();
            model.query.mode = EditorMode::Visual;
            vec![]
        }

        Message::QueryExitVisualMode => {
            model.query.textarea.cancel_selection();
            model.query.mode = EditorMode::Normal;
            vec![]
        }

        Message::QueryYank => {
            model.query.textarea.copy();
            model.query.textarea.cancel_selection();
            model.query.mode = EditorMode::Normal;
            vec![]
        }

        Message::QueryDeleteSelection => {
            model.query.textarea.delete_char(); // Deletes selection if active
            model.query.mode = EditorMode::Normal;
            vec![]
        }

        Message::QueryInput(key_event) => {
            model.query.textarea.input(key_event);
            vec![]
        }

        Message::QueryMoveCursor(direction) => {
            use ratatui::crossterm::event::KeyCode;
            use tui_textarea::CursorMove;
            let cursor_move = match direction {
                KeyCode::Left => CursorMove::Back,
                KeyCode::Right => CursorMove::Forward,
                KeyCode::Up => CursorMove::Up,
                KeyCode::Down => CursorMove::Down,
                KeyCode::Home => CursorMove::Head,
                KeyCode::End => CursorMove::End,
                _ => return vec![],
            };
            model.query.textarea.move_cursor(cursor_move);
            vec![]
        }

        Message::QueryAppend => {
            model
                .query
                .textarea
                .move_cursor(tui_textarea::CursorMove::Forward);
            model.query.mode = EditorMode::Insert;
            vec![]
        }

        Message::QueryAppendEnd => {
            model
                .query
                .textarea
                .move_cursor(tui_textarea::CursorMove::End);
            model.query.mode = EditorMode::Insert;
            vec![]
        }

        Message::QueryOpenBelow => {
            model
                .query
                .textarea
                .move_cursor(tui_textarea::CursorMove::End);
            model.query.textarea.insert_newline();
            model.query.mode = EditorMode::Insert;
            vec![]
        }

        Message::QueryOpenAbove => {
            model
                .query
                .textarea
                .move_cursor(tui_textarea::CursorMove::Head);
            model.query.textarea.insert_newline();
            model
                .query
                .textarea
                .move_cursor(tui_textarea::CursorMove::Up);
            model.query.mode = EditorMode::Insert;
            vec![]
        }

        Message::QueryDeleteChar => {
            model.query.textarea.delete_char();
            vec![]
        }

        Message::QueryDeleteLine => {
            model.query.textarea.delete_line_by_head();
            vec![]
        }

        Message::QueryUndo => {
            model.query.textarea.undo();
            vec![]
        }

        Message::QueryRedo => {
            model.query.textarea.redo();
            vec![]
        }

        Message::QueryMoveTop => {
            model
                .query
                .textarea
                .move_cursor(tui_textarea::CursorMove::Top);
            vec![]
        }

        Message::QueryMoveBottom => {
            model
                .query
                .textarea
                .move_cursor(tui_textarea::CursorMove::Bottom);
            vec![]
        }

        Message::QueryClear => {
            model.query.clear();
            vec![]
        }

        Message::QueryStartExecution => {
            model.query.job_name_input = Some(String::new());
            model.popup = Some(Popup::JobNameInput);
            vec![]
        }

        Message::JobNameInputChar(c) => {
            if let Some(ref mut input) = model.query.job_name_input {
                input.push(c);
            }
            vec![]
        }

        Message::JobNameInputBackspace => {
            if let Some(ref mut input) = model.query.job_name_input {
                input.pop();
            }
            vec![]
        }

        Message::ExecuteQuery(job_name) => {
            let selected_workspaces = model.workspaces.get_selected_workspaces();

            if selected_workspaces.is_empty() {
                model.query.job_name_input = None;
                model.popup = None;
                return vec![Message::ShowError("No workspaces selected".to_string())];
            }

            let query_text = model.query.get_text();
            if query_text.trim().is_empty() {
                model.query.job_name_input = None;
                model.popup = None;
                return vec![Message::ShowError("Query is empty".to_string())];
            }

            let settings = QuerySettings::with_formats(
                &model.settings.output_folder,
                &job_name,
                model.settings.export_csv,
                model.settings.export_json,
                model.settings.parse_dynamics,
            );

            // Create job entries with retry context and capture their IDs
            let mut job_ids = Vec::new();
            for workspace in &selected_workspaces {
                // Use 200 chars for preview to show more KQL query context
                let preview = model.query.get_preview(200);
                let retry_context = crate::tui::model::jobs::RetryContext {
                    workspace: workspace.clone(),
                    query: query_text.clone(),
                    settings: settings.clone(),
                };
                let job_id =
                    model
                        .jobs
                        .add_job_with_context(workspace.name.clone(), preview, retry_context);
                job_ids.push(job_id);
            }

            // Clear popup and input
            model.query.job_name_input = None;
            model.popup = None;

            // Clear pack origin since this is a manual query
            model.sessions.set_pack_origin(None);

            // Mark session as dirty when jobs are added
            model.sessions.mark_dirty();

            // Execute in background
            let client = model.client.clone();
            let query = query_text;
            let workspaces = selected_workspaces;
            let job_settings = settings;
            let update_tx = model.job_update_tx.clone();

            tokio::spawn(async move {
                let results = QueryJobBuilder::new()
                    .workspaces(workspaces)
                    .queries(vec![query])
                    .settings(job_settings)
                    .execute(&client)
                    .await;

                // Send results through the channel using job IDs (not indices!)
                match results {
                    Ok(results) => {
                        for (idx, result) in results.into_iter().enumerate() {
                            if let Some(&job_id) = job_ids.get(idx) {
                                let _ = update_tx.send(
                                    crate::tui::model::JobUpdateMessage::Completed(job_id, result),
                                );
                            }
                        }
                    }
                    Err(e) => {
                        error!("Query execution error: {}", e);
                    }
                }
            });

            vec![]
        }

        Message::QueryOpenLoadPanel => {
            // Open load panel if we have jobs
            if model.jobs.jobs.is_empty() {
                return vec![Message::ShowError("No jobs to load from".to_string())];
            }

            // Check if any jobs have retry context (loadable queries)
            let has_loadable_jobs = model
                .jobs
                .jobs
                .iter()
                .any(|job| job.retry_context.is_some());
            if !has_loadable_jobs {
                return vec![Message::ShowError(
                    "No loadable jobs found (jobs must have queries to load)".to_string(),
                )];
            }

            // Save current query text
            let original_query = model.query.get_text();

            // Create load panel state
            let mut panel_state = crate::tui::model::query::LoadPanelState {
                selected: 0,
                sort: crate::tui::model::query::LoadPanelSort::Chronological,
                inverted: false,
                original_query,
                sorted_indices: vec![],
            };

            // Compute sorted indices
            panel_state.sorted_indices = panel_state.compute_sorted_indices(&model.jobs.jobs);

            // Preview the first job's query (using sorted index)
            // Try to find first job with a retry_context
            let mut found_query = false;
            for &job_idx in &panel_state.sorted_indices {
                if let Some(job) = model.jobs.jobs.get(job_idx) {
                    if let Some(ctx) = &job.retry_context {
                        model.query.set_text(ctx.query.clone());
                        found_query = true;
                        break;
                    }
                }
            }

            // If no query found, this shouldn't happen since we checked above, but handle it
            if !found_query {
                return vec![Message::ShowError("No loadable queries found".to_string())];
            }

            model.query.load_panel = Some(panel_state);
            vec![]
        }

        Message::QueryLoadPanelNavigate(delta) => {
            if let Some(panel) = &mut model.query.load_panel {
                let max_idx = panel.sorted_indices.len().saturating_sub(1);
                let new_selected = if delta > 0 {
                    (panel.selected + 1).min(max_idx)
                } else {
                    panel.selected.saturating_sub(1)
                };

                if new_selected != panel.selected {
                    panel.selected = new_selected;

                    // Preview the selected job's query (using sorted index)
                    if let Some(&job_idx) = panel.sorted_indices.get(new_selected) {
                        if let Some(job) = model.jobs.jobs.get(job_idx) {
                            if let Some(ctx) = &job.retry_context {
                                model.query.set_text(ctx.query.clone());
                            }
                        }
                    }
                }
            }
            vec![]
        }

        Message::QueryLoadPanelCycleSort => {
            if let Some(panel) = &mut model.query.load_panel {
                panel.sort = panel.sort.next();
                // Recompute sorted indices
                panel.sorted_indices = panel.compute_sorted_indices(&model.jobs.jobs);
                // Reset selection to first item when changing sort
                panel.selected = 0;

                // Preview first job with new sort (using sorted index)
                if let Some(&first_idx) = panel.sorted_indices.first() {
                    if let Some(job) = model.jobs.jobs.get(first_idx) {
                        if let Some(ctx) = &job.retry_context {
                            model.query.set_text(ctx.query.clone());
                        }
                    }
                }
            }
            vec![]
        }

        Message::QueryLoadPanelInvertSort => {
            if let Some(panel) = &mut model.query.load_panel {
                panel.inverted = !panel.inverted;
                // Recompute sorted indices with new inversion
                panel.sorted_indices = panel.compute_sorted_indices(&model.jobs.jobs);

                // Keep selection at same visual position (which now points to different job)
                // Preview the job at current selection with new sort
                if let Some(&job_idx) = panel.sorted_indices.get(panel.selected) {
                    if let Some(job) = model.jobs.jobs.get(job_idx) {
                        if let Some(ctx) = &job.retry_context {
                            model.query.set_text(ctx.query.clone());
                        }
                    }
                }
            }
            vec![]
        }

        Message::QueryLoadPanelConfirm => {
            // Load is already previewed, just close the panel
            if model.query.load_panel.is_some() {
                model.query.load_panel = None;
            }
            vec![]
        }

        Message::QueryLoadPanelCancel => {
            // Restore original query
            if let Some(panel) = model.query.load_panel.take() {
                model.query.set_text(panel.original_query);
            }
            vec![]
        }

        Message::QueryNextPackQuery => {
            if let Some(pack_context) = &mut model.query.pack_context {
                if let Some(next_query) = pack_context.next_query() {
                    // Replace query text with next query
                    model.query.textarea.select_all();
                    let len = model.query.textarea.yank_text().len();
                    model.query.textarea.delete_str(len);
                    model.query.textarea.insert_str(&next_query.query);
                }
            }
            vec![]
        }

        Message::QueryPrevPackQuery => {
            if let Some(pack_context) = &mut model.query.pack_context {
                if let Some(prev_query) = pack_context.prev_query() {
                    // Replace query text with previous query
                    model.query.textarea.select_all();
                    let len = model.query.textarea.yank_text().len();
                    model.query.textarea.delete_str(len);
                    model.query.textarea.insert_str(&prev_query.query);
                }
            }
            vec![]
        }

        // === Jobs ===
        Message::JobsPrevious => {
            let selected = model.jobs.table_state.selected().unwrap_or(0);
            if selected > 0 {
                model.jobs.table_state.select(Some(selected - 1));
            }
            vec![]
        }

        Message::JobsNext => {
            let selected = model.jobs.table_state.selected().unwrap_or(0);
            let max = model.jobs.jobs.len().saturating_sub(1);
            if selected < max {
                model.jobs.table_state.select(Some(selected + 1));
            }
            vec![]
        }

        Message::JobsViewDetails => {
            if model.jobs.get_selected_job().is_some() {
                if let Some(selected) = model.jobs.table_state.selected() {
                    model.popup = Some(Popup::JobDetails(selected));
                }
            }
            vec![]
        }

        Message::JobsClearCompleted => {
            model.jobs.clear_completed();
            // Mark session as dirty when jobs are cleared
            model.sessions.mark_dirty();
            // Close job details popup if it was open, as indices have shifted
            if matches!(model.popup, Some(Popup::JobDetails(_))) {
                model.popup = None;
            }
            vec![]
        }

        Message::JobsRetry => {
            // Get the selected job
            let Some(selected_idx) = model.jobs.table_state.selected() else {
                return vec![Message::ShowError("No job selected".to_string())];
            };

            let Some(job) = model.jobs.jobs.get(selected_idx) else {
                return vec![Message::ShowError("Invalid job selection".to_string())];
            };

            // Only retry failed or completed jobs
            use crate::tui::model::jobs::JobStatus;
            if !matches!(job.status, JobStatus::Failed | JobStatus::Completed) {
                return vec![Message::ShowError(
                    "Can only retry failed or completed jobs".to_string(),
                )];
            }

            // Extract retry context and clone it before borrowing model mutably
            let Some(retry_ctx) = job.retry_context.clone() else {
                return vec![Message::ShowError(
                    "Job cannot be retried (missing context)".to_string(),
                )];
            };

            // Create new job entry with retry context and capture its ID
            let preview = retry_ctx.query.chars().take(200).collect(); // Use 200 chars like elsewhere
            let new_job_id = model.jobs.add_job_with_context(
                retry_ctx.workspace.name.clone(),
                preview,
                retry_ctx.clone(),
            );

            // Auto-select the new job for visibility (it's at the end of the list)
            let new_job_idx = model.jobs.jobs.len() - 1;
            model.jobs.table_state.select(Some(new_job_idx));

            // Mark session as dirty when retrying jobs
            model.sessions.mark_dirty();

            // Execute in background (same pattern as QueryExecute)
            let client = model.client.clone();
            let workspace = retry_ctx.workspace.clone();
            let query = retry_ctx.query.clone();
            let settings = retry_ctx.settings.clone();
            let update_tx = model.job_update_tx.clone();

            tokio::spawn(async move {
                let results = QueryJobBuilder::new()
                    .workspaces(vec![workspace])
                    .queries(vec![query])
                    .settings(settings)
                    .execute(&client)
                    .await;

                match results {
                    Ok(mut results) if !results.is_empty() => {
                        let result = results.remove(0);
                        let _ = update_tx.send(crate::tui::model::JobUpdateMessage::Completed(
                            new_job_id, // Use job ID, not index!
                            result,
                        ));
                    }
                    Err(e) => {
                        error!("Retry execution error: {}", e);
                    }
                    _ => {}
                }
            });

            // Close popup, switch to Jobs tab to show progress
            vec![Message::ClosePopup, Message::SwitchTab(Tab::Jobs)]
        }

        // === Sessions ===
        Message::SessionsPrevious => {
            let selected = model.sessions.table_state.selected().unwrap_or(0);
            if selected > 0 {
                model.sessions.table_state.select(Some(selected - 1));
            }
            vec![]
        }

        Message::SessionsNext => {
            let selected = model.sessions.table_state.selected().unwrap_or(0);
            let max = model.sessions.sessions.len().saturating_sub(1);
            if selected < max {
                model.sessions.table_state.select(Some(selected + 1));
            }
            vec![]
        }

        Message::SessionsRefresh => {
            // Handled in main loop to avoid blocking
            vec![]
        }

        Message::SessionsStartNew => {
            model.sessions.name_input = Some(String::new());
            model.popup = Some(Popup::SessionNameInput);
            vec![]
        }

        Message::SessionNameInputChar(c) => {
            if let Some(ref mut input) = model.sessions.name_input {
                input.push(c);
            }
            vec![]
        }

        Message::SessionNameInputBackspace => {
            if let Some(ref mut input) = model.sessions.name_input {
                input.pop();
            }
            vec![]
        }

        Message::SessionsSave(name_override) => {
            // Determine session name: use override if provided, otherwise current session, otherwise ask
            let session_name = if let Some(name) = name_override {
                name
            } else if let Some(name) = model.sessions.name_input.take() {
                // Name from popup
                model.popup = None;
                if name.trim().is_empty() {
                    return vec![Message::ShowError(
                        "Session name cannot be empty".to_string(),
                    )];
                }
                name
            } else if let Some(name) = model.sessions.current_session_name.clone() {
                // Save to current session
                name
            } else {
                // No current session and no name provided - show input popup
                model.sessions.name_input = Some(String::new());
                model.popup = Some(Popup::SessionNameInput);
                return vec![];
            };

            // CRITICAL: Drain all pending job updates before saving session
            // This ensures we capture the latest state of all jobs, including
            // completion messages that may have arrived while the UI was busy
            model.process_job_updates();

            // Warn if there are running jobs that might complete after save
            let running_count = model
                .jobs
                .jobs
                .iter()
                .filter(|j| matches!(j.status, crate::tui::model::jobs::JobStatus::Running))
                .count();
            if running_count > 0 {
                log::warn!(
                    "Saving session '{}' with {} running jobs - state may be inconsistent",
                    session_name,
                    running_count
                );
            }

            // Create or update session
            let mut session = crate::session::Session::new_with_pack(
                session_name.clone(),
                &model.settings,
                &model.jobs.jobs,
                model.sessions.current_pack_origin.clone(),
            );

            // If we're saving to the current session, update the timestamp
            if Some(&session_name) == model.sessions.current_session_name.as_ref() {
                session.touch();
            }

            // Save to disk
            match session.save() {
                Ok(_) => {
                    model.sessions.set_current_session(Some(session_name));
                    model.sessions.mark_saved();
                    model.popup = None;
                    vec![Message::SessionsRefresh]
                }
                Err(e) => {
                    model.popup = None;
                    vec![Message::ShowError(format!("Failed to save session: {}", e))]
                }
            }
        }

        Message::SessionsLoad => {
            let Some(selected_session) = model.sessions.get_selected_session() else {
                return vec![Message::ShowError("No session selected".to_string())];
            };

            // Don't reload if already current
            if Some(&selected_session.name) == model.sessions.current_session_name.as_ref() {
                return vec![];
            }

            let session_name = selected_session.name.clone();

            // Load session from disk
            match crate::session::Session::load(&session_name) {
                Ok(session) => {
                    // Apply settings
                    session.apply_to_settings(&mut model.settings);

                    // Rebuild client with loaded settings
                    if let Err(e) = model.rebuild_client() {
                        return vec![Message::ShowError(format!(
                            "Failed to update client settings: {}",
                            e
                        ))];
                    }

                    // Load jobs - pass mutable reference to next_id generator
                    model.jobs.jobs = session.to_job_states(model.jobs.next_job_id_mut());
                    // Sort jobs by timestamp (newest first)
                    model.jobs.sort_by_timestamp();
                    // If jobs were loaded, select the first one
                    if !model.jobs.jobs.is_empty() {
                        model.jobs.table_state.select(Some(0));
                    } else {
                        model.jobs.table_state.select(None);
                    }

                    // Load pack origin (if any)
                    model
                        .sessions
                        .set_pack_origin(session.created_from_pack.clone());

                    // Set as current session
                    model.sessions.set_current_session(Some(session_name));
                    vec![Message::SessionsRefresh]
                }
                Err(e) => vec![Message::ShowError(format!("Failed to load session: {}", e))],
            }
        }

        Message::SessionsDelete => {
            let Some(selected_session) = model.sessions.get_selected_session() else {
                return vec![Message::ShowError("No session selected".to_string())];
            };

            let session_name = selected_session.name.clone();

            // Don't delete current session if it's unsaved
            if Some(&session_name) == model.sessions.current_session_name.as_ref() {
                // Clear current session instead of deleting
                model.sessions.set_current_session(None);
                model.jobs.jobs.clear();
                model.jobs.table_state.select(None);
            }

            // Delete from disk
            match crate::session::Session::delete(&session_name) {
                Ok(()) => vec![Message::SessionsRefresh],
                Err(e) => vec![Message::ShowError(format!(
                    "Failed to delete session: {}",
                    e
                ))],
            }
        }

        Message::SessionExportAsPack => {
            let Some(selected_session) = model.sessions.get_selected_session() else {
                return vec![Message::ShowError("No session selected".to_string())];
            };

            let session_name = selected_session.name.clone();

            // Load session from disk
            let session = match crate::session::Session::load(&session_name) {
                Ok(s) => s,
                Err(e) => {
                    return vec![Message::ShowError(format!("Failed to load session: {}", e))]
                }
            };

            // Convert to query pack
            let pack = match session.to_query_pack() {
                Ok(p) => p,
                Err(e) => {
                    return vec![Message::ShowError(format!(
                        "Failed to convert to pack: {}",
                        e
                    ))]
                }
            };

            // Generate output filename (remove timestamp suffix if present)
            let pack_name = session_name
                .rsplit_once('_')
                .and_then(|(prefix, suffix)| {
                    if suffix.chars().all(|c| c.is_ascii_digit()) && suffix.len() >= 6 {
                        Some(prefix)
                    } else {
                        None
                    }
                })
                .unwrap_or(&session_name);

            let output_path = match crate::query_pack::QueryPack::get_library_path(&format!(
                "{}.yaml",
                pack_name
            )) {
                Ok(p) => p,
                Err(e) => {
                    return vec![Message::ShowError(format!(
                        "Failed to get output path: {}",
                        e
                    ))]
                }
            };

            // Ensure parent directory exists
            if let Some(parent) = output_path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    return vec![Message::ShowError(format!(
                        "Failed to create directory: {}",
                        e
                    ))];
                }
            }

            // Save pack
            match pack.save_to_file(&output_path) {
                Ok(()) => {
                    // Refresh packs list to show the new pack
                    // Note: Success is indicated by the pack appearing in the Packs tab
                    vec![Message::PacksRefresh]
                }
                Err(e) => vec![Message::ShowError(format!("Failed to save pack: {}", e))],
            }
        }

        // === Query Packs ===
        Message::PacksPrevious => {
            model.packs.previous();
            vec![]
        }

        Message::PacksNext => {
            model.packs.next();
            vec![]
        }

        Message::PacksRefresh => {
            model.packs.refresh();
            vec![]
        }

        Message::PacksLoadDetails => {
            // Lazy load the selected pack
            if let Err(e) = model.packs.load_selected_pack() {
                vec![Message::ShowError(format!("Failed to load pack: {}", e))]
            } else {
                vec![]
            }
        }

        Message::PacksLoadQuery => {
            // First ensure the pack is loaded
            if let Err(e) = model.packs.load_selected_pack() {
                return vec![Message::ShowError(format!("Failed to load pack: {}", e))];
            }

            // Now get the loaded pack and extract query
            if let Some(entry) = model.packs.get_selected_entry() {
                if let Some(pack) = &entry.pack {
                    let queries = pack.get_queries();
                    if let Some(first_query) = queries.first() {
                        // Set the query text in the editor
                        model.query.textarea.select_all();
                        let len = model.query.textarea.yank_text().len();
                        model.query.textarea.delete_str(len);
                        model.query.textarea.insert_str(&first_query.query);

                        // Set pack context for navigation
                        model.query.pack_context = Some(crate::tui::model::query::PackContext {
                            pack_name: pack.name.clone(),
                            pack_path: entry.relative_path.clone(),
                            queries: queries.clone(),
                            current_index: 0,
                        });

                        // Switch to Query tab
                        vec![Message::SwitchTab(Tab::Query)]
                    } else {
                        vec![Message::ShowError("Pack contains no queries".to_string())]
                    }
                } else {
                    vec![Message::ShowError(
                        "Failed to load pack details".to_string(),
                    )]
                }
            } else {
                vec![Message::ShowError("No pack selected".to_string())]
            }
        }

        Message::PacksExecute => {
            // First ensure the pack is loaded
            if let Err(e) = model.packs.load_selected_pack() {
                return vec![Message::ShowError(format!("Failed to load pack: {}", e))];
            }

            // Now execute the pack
            if let Some(entry) = model.packs.get_selected_entry() {
                if let Some(pack) = &entry.pack {
                    let selected_workspaces: Vec<_> = model
                        .workspaces
                        .workspaces
                        .iter()
                        .filter(|ws| ws.selected)
                        .map(|ws| ws.workspace.clone())
                        .collect();

                    if selected_workspaces.is_empty() {
                        return vec![Message::ShowError(
                            "No workspaces selected. Go to Workspaces tab and select some."
                                .to_string(),
                        )];
                    }

                    let queries = pack.get_queries();
                    if queries.is_empty() {
                        return vec![Message::ShowError("Pack contains no queries".to_string())];
                    }

                    // Get base settings from pack or use current settings
                    let base_settings = pack.settings.clone().unwrap_or_else(|| QuerySettings {
                        job_name: "query".to_string(), // Will be overridden per query
                        export_csv: model.settings.export_csv,
                        export_json: model.settings.export_json,
                        parse_dynamics: model.settings.parse_dynamics,
                        output_folder: model.settings.output_folder.clone().into(),
                    });

                    // Create jobs for all queries x workspaces
                    // Collect job IDs for tracking completion
                    let mut job_ids = Vec::new();
                    let job_count_before = model.jobs.jobs.len();

                    for pack_query in &queries {
                        // Create unique settings for each query with sanitized name
                        let query_job_name = sanitize_filename(&pack_query.name);
                        let mut query_settings = base_settings.clone();
                        query_settings.job_name = query_job_name;

                        for workspace in &selected_workspaces {
                            // Create a better preview for KQL queries (200 chars to show more context)
                            let query_preview = pack_query.query.chars().take(200).collect();

                            let retry_context = crate::tui::model::jobs::RetryContext {
                                workspace: workspace.clone(),
                                query: pack_query.query.clone(),
                                settings: query_settings.clone(),
                            };

                            // Capture the job ID for this job
                            let job_id = model.jobs.add_job_with_context(
                                workspace.name.clone(),
                                query_preview,
                                retry_context.clone(),
                            );

                            job_ids.push((job_id, retry_context));
                        }
                    }

                    // Track pack origin for session
                    model
                        .sessions
                        .set_pack_origin(Some(entry.relative_path.clone()));

                    // Mark session as dirty
                    model.sessions.mark_dirty();

                    // Execute each job individually to preserve per-query settings
                    // (QueryJobBuilder applies a single settings to all jobs, losing our sanitized names)
                    let client = model.client.clone();
                    let update_tx = model.job_update_tx.clone();

                    log::info!("Spawning {} tasks for pack execution", job_ids.len());

                    // Create semaphore to limit concurrent query execution
                    // This prevents resource exhaustion with large packs across many workspaces
                    const MAX_CONCURRENT_QUERIES: usize = 15;
                    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_QUERIES));

                    // Spawn individual tasks for each job using stable job IDs
                    for (job_id, retry_ctx) in job_ids {
                        let client = client.clone();
                        let tx = update_tx.clone();
                        let semaphore = semaphore.clone();

                        log::debug!("Spawning task for job ID {}", job_id);

                        tokio::spawn(async move {
                            // Acquire semaphore permit before executing query
                            let _permit = semaphore.acquire().await.expect("Semaphore closed");
                            log::debug!("Job {} acquired semaphore permit, executing", job_id);

                            // Clone retry_ctx for error cases (will be moved into builder)
                            let retry_ctx_for_errors = retry_ctx.clone();

                            let results = QueryJobBuilder::new()
                                .workspaces(vec![retry_ctx.workspace])
                                .queries(vec![retry_ctx.query])
                                .settings(retry_ctx.settings)
                                .execute(&client)
                                .await;

                            // Send completion message in ALL cases (success or failure)
                            match results {
                                Ok(mut results) if !results.is_empty() => {
                                    let result = results.remove(0);
                                    log::debug!(
                                        "Job {} completed successfully, sending completion message",
                                        job_id
                                    );
                                    let _ =
                                        tx.send(crate::tui::model::JobUpdateMessage::Completed(
                                            job_id, result,
                                        ));
                                }
                                Ok(_) => {
                                    // Empty results - shouldn't happen but handle it
                                    log::error!("Job {} produced no results (empty vec), sending failed message", job_id);
                                    // Create a failed result to update the UI
                                    let failed_result = create_failed_result(
                                        retry_ctx_for_errors,
                                        "Query execution returned no results".to_string(),
                                    );
                                    let _ =
                                        tx.send(crate::tui::model::JobUpdateMessage::Completed(
                                            job_id,
                                            failed_result,
                                        ));
                                }
                                Err(e) => {
                                    // Execution error - create failed result
                                    log::error!(
                                        "Job {} failed: {}, sending failed message",
                                        job_id,
                                        e
                                    );
                                    let failed_result =
                                        create_failed_result(retry_ctx_for_errors, e.to_string());
                                    let _ =
                                        tx.send(crate::tui::model::JobUpdateMessage::Completed(
                                            job_id,
                                            failed_result,
                                        ));
                                }
                            }
                            // Permit is automatically released when _permit is dropped
                        });
                    }

                    // Mark all newly created jobs as running
                    for i in job_count_before..model.jobs.jobs.len() {
                        if let Some(job) = model.jobs.jobs.get_mut(i) {
                            job.status = crate::tui::model::jobs::JobStatus::Running;
                        }
                    }

                    vec![
                        Message::SwitchTab(Tab::Jobs),
                        Message::ShowError(format!(
                            "Executing {} queries across {} workspaces",
                            queries.len(),
                            selected_workspaces.len()
                        )),
                    ]
                } else {
                    vec![Message::ShowError(
                        "Failed to load pack details".to_string(),
                    )]
                }
            } else {
                vec![Message::ShowError("No pack selected".to_string())]
            }
        }

        Message::PacksSave => {
            // Check if there's a pack loaded in the query editor
            if let Some(pack_context) = &model.query.pack_context {
                // Clone necessary data to avoid borrow checker issues
                let pack_path = pack_context.pack_path.clone();
                let current_index = pack_context.current_index;

                // Get the current query text from the editor
                let current_query_text = model.query.get_text();

                // Find the pack entry that matches the loaded pack
                let pack_entry = model
                    .packs
                    .packs
                    .iter_mut()
                    .find(|entry| entry.relative_path == pack_path);

                if let Some(entry) = pack_entry {
                    // Ensure the pack is loaded
                    if entry.pack.is_none() {
                        if let Err(e) = crate::query_pack::QueryPack::load_from_file(&entry.path) {
                            return vec![Message::ShowError(format!("Failed to load pack: {}", e))];
                        }
                    }

                    if let Some(pack) = &mut entry.pack {
                        // Update the specific query in the pack
                        let queries = pack.get_queries();

                        if current_index >= queries.len() {
                            return vec![Message::ShowError("Invalid query index".to_string())];
                        }

                        // Reconstruct the pack with the updated query
                        if let Some(pack_queries) = &mut pack.queries {
                            // Multiple queries format
                            if current_index < pack_queries.len() {
                                pack_queries[current_index].query = current_query_text.clone();
                            }
                        } else if pack.query.is_some() && current_index == 0 {
                            // Single query format
                            pack.query = Some(current_query_text.clone());
                        }

                        // Save the pack to disk
                        let pack_name = pack.name.clone();
                        match pack.save_to_file(&entry.path) {
                            Ok(_) => {
                                // Update the pack_context with the saved query
                                if let Some(ctx) = &mut model.query.pack_context {
                                    if current_index < ctx.queries.len() {
                                        ctx.queries[current_index].query = current_query_text;
                                    }
                                }

                                vec![Message::ShowSuccess(format!(
                                    "Saved changes to pack: {}",
                                    pack_name
                                ))]
                            }
                            Err(e) => {
                                vec![Message::ShowError(format!("Failed to save pack: {}", e))]
                            }
                        }
                    } else {
                        vec![Message::ShowError("Pack not loaded".to_string())]
                    }
                } else {
                    vec![Message::ShowError("Pack not found in list".to_string())]
                }
            } else {
                vec![Message::ShowError(
                    "No pack loaded. Load a pack from the Packs tab first.".to_string(),
                )]
            }
        }

        // === Popups ===
        Message::ShowError(msg) => {
            model.popup = Some(Popup::Error(msg));
            vec![]
        }

        Message::ShowSuccess(msg) => {
            model.popup = Some(Popup::Success(msg));
            vec![]
        }

        Message::ClosePopup => {
            model.popup = None;
            model.settings.editing = None;
            model.query.job_name_input = None;
            model.sessions.name_input = None;
            vec![]
        }

        // === System ===
        Message::NoOp => vec![],

        Message::AuthCompleted => {
            // Authentication completed, proceed to load workspaces
            vec![]
        }

        Message::AuthFailed(error) => {
            model.init_state = crate::tui::model::InitState::Failed;
            vec![Message::ShowError(format!(
                "Authentication failed: {}",
                error
            ))]
        }

        Message::InitCompleted => {
            model.init_state = crate::tui::model::InitState::Ready;
            vec![]
        }
    }
}
