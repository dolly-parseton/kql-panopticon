/// UI state for the interpreter view
pub struct UIState {
    /// Current scroll offset in lines from the top
    pub scroll_offset: usize,
    /// Whether to auto-scroll when new content is added
    /// Set to false when user manually scrolls up, re-enabled when scrolled to bottom
    pub auto_scroll: bool,
    /// Last known viewport height (updated during render)
    /// Used for auto-scroll calculations when blocks are added
    pub viewport_height: usize,
}

impl Default for UIState {
    fn default() -> Self {
        Self {
            scroll_offset: 0,
            auto_scroll: true,
            viewport_height: 0,
        }
    }
}
