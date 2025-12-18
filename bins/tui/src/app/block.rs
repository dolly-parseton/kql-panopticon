/// Unique identifier for blocks
#[derive(Debug)]
pub struct InterpreterBlock {
    /// Block identifier
    pub id: usize,
    /// Command run that's associated with this block
    pub command: String,
    /// The result of running that command
    pub results: String,
    /// The status of a block, determines icon shown
    pub status: BlockStatus,
}

#[derive(Debug, PartialEq)]
pub enum BlockStatus {
    Running,
    Completed,
    Failed,
}

impl InterpreterBlock {
    /// Create a new Block instance
    pub fn new(id: usize, command: String) -> Self {
        Self {
            id,
            command,
            results: String::new(),
            status: BlockStatus::Running,
        }
    }

    /// Calculate the rendered height of this block in lines
    /// Layout: command line (1) + result lines (N) + spacing (1)
    pub fn rendered_height(&self) -> usize {
        let result_lines = if self.results.is_empty() {
            0
        } else {
            self.results.lines().count()
        };
        1 + result_lines + 1 // command + results + spacing
    }

    /// Mark block as completed with result message
    pub fn complete(&mut self, results: String) {
        self.results = results;
        self.status = BlockStatus::Completed;
    }

    /// Mark block as failed with error message
    pub fn fail(&mut self, error: String) {
        self.results = error;
        self.status = BlockStatus::Failed;
    }

    /// Update results while keeping current status
    pub fn update_results(&mut self, results: String) {
        self.results = results;
    }

    /// Append to existing results (for progress updates)
    pub fn append_results(&mut self, message: &str) {
        if !self.results.is_empty() {
            self.results.push('\n');
        }
        self.results.push_str(message);
    }

    /// Set status explicitly
    pub fn set_status(&mut self, status: BlockStatus) {
        self.status = status;
    }
}

impl BlockStatus {
    pub fn icon(&self) -> &'static str {
        match self {
            BlockStatus::Running => "⏳",
            BlockStatus::Completed => "✅",
            BlockStatus::Failed => "❌",
        }
    }
}
