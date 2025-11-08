use crate::query_job::{QueryJobBuilder, QuerySettings};
use crate::tui::message::{Message, Tab};
use crate::tui::model::{query::EditorMode, Model, Popup};
use log::error;

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

            // Create job entries with retry context
            let start_idx = model.jobs.jobs.len();
            for workspace in &selected_workspaces {
                let preview = model.query.get_preview(50);
                let retry_context = crate::tui::model::jobs::RetryContext {
                    workspace: workspace.clone(),
                    query: query_text.clone(),
                    settings: settings.clone(),
                };
                model
                    .jobs
                    .add_job_with_context(workspace.name.clone(), preview, retry_context);
            }

            // Clear popup and input
            model.query.job_name_input = None;
            model.popup = None;

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

                // Send results through the channel
                match results {
                    Ok(results) => {
                        for (idx, result) in results.into_iter().enumerate() {
                            let job_idx = start_idx + idx;
                            let _ = update_tx.send(crate::tui::model::JobUpdateMessage::Completed(
                                job_idx, result,
                            ));
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
            let has_loadable_jobs = model.jobs.jobs.iter().any(|job| job.retry_context.is_some());
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
                return vec![Message::ShowError(
                    "No loadable queries found".to_string(),
                )];
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

            // Create new job entry with retry context
            let new_job_idx = model.jobs.jobs.len();
            let preview = retry_ctx.query.chars().take(50).collect();
            model.jobs.add_job_with_context(
                retry_ctx.workspace.name.clone(),
                preview,
                retry_ctx.clone(),
            );

            // Auto-select the new job for visibility
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
                            new_job_idx,
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
                    return vec![Message::ShowError("Session name cannot be empty".to_string())];
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

            // Create or update session
            let mut session = crate::session::Session::new(
                session_name.clone(),
                &model.settings,
                &model.jobs.jobs,
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

                    // Load jobs
                    model.jobs.jobs = session.to_job_states();
                    // Sort jobs by timestamp (newest first)
                    model.jobs.sort_by_timestamp();
                    // If jobs were loaded, select the first one
                    if !model.jobs.jobs.is_empty() {
                        model.jobs.table_state.select(Some(0));
                    } else {
                        model.jobs.table_state.select(None);
                    }

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
                Err(e) => vec![Message::ShowError(format!("Failed to delete session: {}", e))],
            }
        }

        // === Popups ===
        Message::ShowError(msg) => {
            model.popup = Some(Popup::Error(msg));
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
