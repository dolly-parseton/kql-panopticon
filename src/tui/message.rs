use crate::workspace::Workspace;

/// All possible messages that can update the application state
#[derive(Debug, Clone)]
pub enum Message {
    // === Navigation ===
    /// Switch to a different tab
    SwitchTab(Tab),
    /// Quit the application
    Quit,

    // === Settings ===
    /// Navigate settings list up
    SettingsPrevious,
    /// Navigate settings list down
    SettingsNext,
    /// Start editing the selected setting
    SettingsStartEdit,
    /// Update setting value input
    SettingsInputChar(char),
    /// Remove last character from setting input
    SettingsInputBackspace,
    /// Save the edited setting
    SettingsSave,
    /// Cancel editing without saving
    SettingsCancel,

    // === Workspaces ===
    /// Navigate workspace list up
    WorkspacesPrevious,
    /// Navigate workspace list down
    WorkspacesNext,
    /// Toggle selection of current workspace
    WorkspacesToggle,
    /// Select all workspaces
    WorkspacesSelectAll,
    /// Deselect all workspaces
    WorkspacesSelectNone,
    /// Refresh workspaces from Azure
    WorkspacesRefresh,
    /// Workspaces loaded successfully
    WorkspacesLoaded(Vec<Workspace>),

    // === Query ===
    /// Enter insert mode (vim-style)
    QueryEnterInsertMode,
    /// Exit insert mode (vim-style)
    QueryExitInsertMode,
    /// Enter visual mode (vim-style)
    QueryEnterVisualMode,
    /// Exit visual mode (vim-style)
    QueryExitVisualMode,
    /// Copy selected text (yank in vim)
    QueryYank,
    /// Delete selected text
    QueryDeleteSelection,
    /// Append after cursor (vim 'a')
    QueryAppend,
    /// Append at end of line (vim 'A')
    QueryAppendEnd,
    /// Open line below (vim 'o')
    QueryOpenBelow,
    /// Open line above (vim 'O')
    QueryOpenAbove,
    /// Delete character under cursor (vim 'x')
    QueryDeleteChar,
    /// Delete current line (vim 'dd' or Ctrl+D)
    QueryDeleteLine,
    /// Move cursor (vim hjkl or arrow keys)
    QueryMoveCursor(ratatui::crossterm::event::KeyCode),
    /// Move to top of file (vim 'gg')
    QueryMoveTop,
    /// Move to bottom of file (vim 'G')
    QueryMoveBottom,
    /// Undo last edit (vim 'u' or Ctrl+U)
    QueryUndo,
    /// Redo (vim Ctrl+R)
    QueryRedo,
    /// Pass raw input to tui-textarea
    QueryInput(ratatui::crossterm::event::KeyEvent),
    /// Clear query text
    QueryClear,
    /// Start job name input for query execution
    QueryStartExecution,
    /// Job name input character
    JobNameInputChar(char),
    /// Job name input backspace
    JobNameInputBackspace,
    /// Execute query with job name
    ExecuteQuery(String),
    /// Open load query panel
    QueryOpenLoadPanel,
    /// Navigate jobs in load panel
    QueryLoadPanelNavigate(i32), // +1 for down, -1 for up
    /// Cycle sort order in load panel (Tab key)
    QueryLoadPanelCycleSort,
    /// Invert sort order in load panel (i key)
    QueryLoadPanelInvertSort,
    /// Load selected query from load panel
    QueryLoadPanelConfirm,
    /// Cancel load panel (restore original query)
    QueryLoadPanelCancel,
    /// Navigate to next query in pack (] key)
    QueryNextPackQuery,
    /// Navigate to previous query in pack ([ key)
    QueryPrevPackQuery,

    // === Jobs ===
    /// Navigate jobs list up
    JobsPrevious,
    /// Navigate jobs list down
    JobsNext,
    /// View details of selected job
    JobsViewDetails,
    /// Clear completed and failed jobs
    JobsClearCompleted,
    /// Retry selected job
    JobsRetry,

    // === Sessions ===
    /// Navigate sessions list up
    SessionsPrevious,
    /// Navigate sessions list down
    SessionsNext,
    /// Refresh sessions list from disk
    SessionsRefresh,
    /// Start new session name input
    SessionsStartNew,
    /// Session name input character
    SessionNameInputChar(char),
    /// Session name input backspace
    SessionNameInputBackspace,
    /// Save current session (with optional new name)
    SessionsSave(Option<String>),
    /// Load selected session
    SessionsLoad,
    /// Delete selected session
    SessionsDelete,
    /// Export selected session as query pack
    SessionExportAsPack,

    // === Query Packs ===
    /// Navigate packs list up
    PacksPrevious,
    /// Navigate packs list down
    PacksNext,
    /// Refresh packs list from disk
    PacksRefresh,
    /// Load selected pack details
    #[allow(dead_code)]
    PacksLoadDetails,
    /// Load first query from selected pack into query editor
    PacksLoadQuery,
    /// Execute selected pack on selected workspaces
    PacksExecute,
    /// Save current query changes back to the loaded pack
    PacksSave,

    // === Popups ===
    /// Show an error popup (red)
    ShowError(String),
    /// Show a success popup (green)
    ShowSuccess(String),
    /// Close the current popup
    ClosePopup,

    // === System ===
    /// No operation (used for events that don't produce messages)
    NoOp,
    /// Authentication completed successfully
    AuthCompleted,
    /// Authentication failed
    AuthFailed(String),
    /// Initialization completed successfully
    InitCompleted,
}

/// Application tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Query,
    Packs,
    Workspaces,
    Settings,
    Jobs,
    Sessions,
}

impl Tab {
    pub fn next(self) -> Self {
        match self {
            Tab::Query => Tab::Packs,
            Tab::Packs => Tab::Workspaces,
            Tab::Workspaces => Tab::Settings,
            Tab::Settings => Tab::Jobs,
            Tab::Jobs => Tab::Sessions,
            Tab::Sessions => Tab::Query,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Tab::Query => Tab::Sessions,
            Tab::Packs => Tab::Query,
            Tab::Workspaces => Tab::Packs,
            Tab::Settings => Tab::Workspaces,
            Tab::Jobs => Tab::Settings,
            Tab::Sessions => Tab::Jobs,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Tab::Query => "Query (1)",
            Tab::Packs => "Packs (2)",
            Tab::Workspaces => "Workspaces (3)",
            Tab::Settings => "Settings (4)",
            Tab::Jobs => "Jobs (5)",
            Tab::Sessions => "Sessions (6)",
        }
    }
}
