//! Azure connection state management
//!
//! Handles async client authentication and workspace discovery with
//! non-blocking communication back to the synchronous TUI event loop.

use kql_panopticon::{Client, Workspace};
use tokio::sync::mpsc;

/// Connection status for Azure client
#[derive(Debug, Clone)]
pub enum ConnectionStatus {
    /// Currently authenticating with Azure
    Authenticating,
    /// Authentication succeeded, discovering workspaces
    Discovering,
    /// Ready with N workspaces discovered
    Ready { workspace_count: usize },
    /// Authentication or discovery failed
    Error { message: String },
}

impl Default for ConnectionStatus {
    fn default() -> Self {
        Self::Authenticating
    }
}

/// Result from background connection task
#[derive(Debug)]
pub enum ConnectionResult {
    /// Authentication validation succeeded
    AuthSuccess,
    /// Authentication validation failed
    AuthFailed(String),
    /// Workspace discovery completed
    WorkspacesDiscovered(Vec<Workspace>),
    /// Workspace discovery failed
    DiscoveryFailed(String),
}

/// Receiver for connection results
pub type ConnectionResultReceiver = mpsc::UnboundedReceiver<ConnectionResult>;

/// Sender for connection results (internal use)
type ConnectionResultSender = mpsc::UnboundedSender<ConnectionResult>;

/// Spawn background task for client authentication and workspace discovery
///
/// The task performs two phases:
/// 1. Validate Azure authentication
/// 2. Discover all accessible workspaces
///
/// Results are communicated back via the unbounded channel. Returns both the
/// task handle and the receiver.
pub fn spawn_connection_task(
    client: Client,
) -> (tokio::task::JoinHandle<()>, ConnectionResultReceiver) {
    let (result_tx, result_rx) = mpsc::unbounded_channel();

    let task_handle = tokio::spawn(async move {
        // Phase 1: Validate authentication
        match client.force_validate_auth().await {
            Ok(()) => {
                // Signal auth success
                let _ = result_tx.send(ConnectionResult::AuthSuccess);

                // Phase 2: Discover workspaces
                match client.list_workspaces().await {
                    Ok(workspaces) => {
                        let _ = result_tx.send(ConnectionResult::WorkspacesDiscovered(workspaces));
                    }
                    Err(e) => {
                        let _ = result_tx.send(ConnectionResult::DiscoveryFailed(e.to_string()));
                    }
                }
            }
            Err(e) => {
                let _ = result_tx.send(ConnectionResult::AuthFailed(e.to_string()));
            }
        }
        // Channel closes when result_tx drops
    });

    (task_handle, result_rx)
}
