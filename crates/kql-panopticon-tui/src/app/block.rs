use std::collections::HashMap;

pub type BlockSender = tokio::sync::mpsc::Sender<(String, BlockData)>;
pub type BlockReceiver = tokio::sync::mpsc::Receiver<(String, BlockData)>;

/// Unique identifier for blocks
#[derive(Debug)]
pub struct InterpreterBlock {
    /// Block identifier
    pub id: usize,
    /// Command run that's associated with this block
    pub command: String,
    /// Data contained in the block (key value store, keys are consumed by widget)
    pub data: HashMap<String, BlockData>,
    // Sender and reciever for updating block data asynchronously - uncomment after we get wire frame done.
    pub tx: Option<BlockSender>,
    pub rx: Option<BlockReceiver>,
}

/// A enum that represents data contained in a block that a widget reads, tpye infered by App state to the UI layer
#[derive(Debug)]
pub enum BlockData {
    String(String),
    Table {
        headings: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    Form {
        fields: Vec<(String, String)>,
    },
}

/// Each block can contain a sender / reciever pair for updating asynchronously.
pub struct BlockOrchestration {
    pub senders: HashMap<usize, BlockSender>,
    pub receivers: HashMap<usize, BlockReceiver>,
}

impl InterpreterBlock {
    /// Create a new Block instance
    pub fn new(id: usize, command: String) -> Self {
        Self {
            id,
            command,
            data: HashMap::new(),
            tx: None,
            rx: None,
        }
    }
}
