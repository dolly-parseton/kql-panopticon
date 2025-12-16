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

#[derive(Debug)]
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
