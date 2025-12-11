//! Core execution engine trait
//!
//! Defines the contract for all execution engines (query, investigation, etc.)

use crate::error::Result;
use crate::workspace::Workspace;
use super::progress::ProgressSender;
use async_trait::async_trait;
use std::fmt::Debug;

/// Core trait for execution engines
///
/// This trait defines the contract that all execution engines must implement,
/// enabling uniform handling of different execution types (queries, investigations,
/// future extensions).
///
/// ## Type Parameters
///
/// - `Config` - The configuration type for this executor (e.g., QueryPack, InvestigationPack)
/// - `Result` - The result type produced by execution
///
/// ## Example
///
/// ```rust,ignore
/// use kql_panopticon_core::execution::{ExecutionEngine, ProgressSender};
///
/// let executor = QueryExecutor::new(client);
/// let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
///
/// let result = executor.execute(
///     config,
///     workspaces,
///     Some(ProgressSender::new(tx)),
/// ).await?;
/// ```
#[async_trait]
pub trait ExecutionEngine: Send + Sync {
    /// Configuration type for this executor
    type Config: Send + Sync;

    /// Result type produced by execution
    type Result: Send + Sync + Debug;

    /// Execute the configured operation across the given workspaces
    ///
    /// ## Arguments
    ///
    /// * `config` - Execution configuration (pack definition, settings, etc.)
    /// * `workspaces` - Target workspaces to execute against
    /// * `progress` - Optional channel for progress updates
    ///
    /// ## Returns
    ///
    /// The execution result, or an error if execution failed.
    async fn execute(
        &self,
        config: Self::Config,
        workspaces: Vec<Workspace>,
        progress: Option<ProgressSender>,
    ) -> Result<Self::Result>;

    /// Validate configuration before execution
    ///
    /// This should be called before `execute()` to catch configuration
    /// errors early. Returns `Ok(())` if valid, or an error describing
    /// the validation failure.
    fn validate(&self, config: &Self::Config) -> Result<()>;

    /// Get the name of this executor type (for logging/debugging)
    fn executor_name(&self) -> &'static str;
}

/// Execution mode for controlling behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionMode {
    /// Normal execution
    #[default]
    Normal,
    /// Validate only, don't execute
    ValidateOnly,
    /// Dry run - log what would happen without executing
    DryRun,
}

/// Common execution options
#[derive(Debug, Clone)]
pub struct ExecutionOptions {
    /// Execution mode
    pub mode: ExecutionMode,
    /// Output directory for results
    pub output_dir: Option<std::path::PathBuf>,
    /// Timeout for individual operations (in seconds)
    pub timeout_secs: Option<u64>,
    /// Maximum concurrent operations
    pub max_concurrency: Option<usize>,
    /// Enable verbose debugging output
    pub verbose: bool,
}

impl Default for ExecutionOptions {
    fn default() -> Self {
        Self {
            mode: ExecutionMode::Normal,
            output_dir: None,
            timeout_secs: Some(120), // 2 minutes default
            max_concurrency: None,    // Unlimited by default
            verbose: false,
        }
    }
}

impl ExecutionOptions {
    /// Create options for validation only
    pub fn validate_only() -> Self {
        Self {
            mode: ExecutionMode::ValidateOnly,
            ..Default::default()
        }
    }

    /// Create options for dry run
    pub fn dry_run() -> Self {
        Self {
            mode: ExecutionMode::DryRun,
            ..Default::default()
        }
    }

    /// Set output directory
    pub fn with_output_dir(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        self.output_dir = Some(path.into());
        self
    }

    /// Set timeout
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    /// Set concurrency limit
    pub fn with_max_concurrency(mut self, limit: usize) -> Self {
        self.max_concurrency = Some(limit);
        self
    }

    /// Enable verbose output
    pub fn verbose(mut self) -> Self {
        self.verbose = true;
        self
    }
}
