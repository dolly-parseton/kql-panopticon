//! Exploring interpreter state graph
//!
//! Implements the exploring interpreter pattern from van Binsbergen et al. (2020),
//! enabling backtracking, checkpoints, and non-linear exploration of investigation
//! sessions.
//!
//! Each state-modifying command creates a new state snapshot, allowing users to:
//! - Revert to any previous state
//! - Save named checkpoints for significant points
//! - View the execution history as a tree
//!
//! Reference: https://doi.org/10.1145/3426428.3426917

use crate::session::PackSession;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Unique identifier for a state in the execution graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct StateId(pub u64);

impl StateId {
    /// Get the numeric value
    pub fn value(&self) -> u64 {
        self.0
    }

    /// Create a new state ID with the next sequential value
    pub fn next(&self) -> Self {
        Self(self.0 + 1)
    }
}

impl std::fmt::Display for StateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// A snapshot of the session state at a point in time
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// The session state at this point
    pub session: PackSession,

    /// When this state was created
    pub timestamp: DateTime<Utc>,

    /// The command that created this state (None for initial state)
    pub command: Option<String>,
}

impl Snapshot {
    /// Create a new snapshot
    pub fn new(session: PackSession, command: Option<String>) -> Self {
        Self {
            session,
            timestamp: Utc::now(),
            command,
        }
    }

    /// Create the initial snapshot (state #0)
    pub fn initial() -> Self {
        Self::new(PackSession::new(), None)
    }
}

/// The state graph tracking all snapshots and their relationships
#[derive(Debug)]
pub struct StateGraph {
    /// All state snapshots indexed by ID
    snapshots: HashMap<StateId, Snapshot>,

    /// Parent state for each state (forms the ancestry tree)
    ancestry: HashMap<StateId, StateId>,

    /// Named checkpoints pointing to state IDs
    checkpoints: HashMap<String, StateId>,

    /// The current state ID
    current: StateId,

    /// Counter for generating new state IDs
    next_id: StateId,
}

impl Default for StateGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl StateGraph {
    /// Create a new state graph with initial state #0
    pub fn new() -> Self {
        let initial_id = StateId(0);
        let mut snapshots = HashMap::new();
        snapshots.insert(initial_id, Snapshot::initial());

        Self {
            snapshots,
            ancestry: HashMap::new(),
            checkpoints: HashMap::new(),
            current: initial_id,
            next_id: StateId(1),
        }
    }

    /// Get the current state ID
    pub fn current_id(&self) -> StateId {
        self.current
    }

    /// Get the current snapshot
    pub fn current_snapshot(&self) -> &Snapshot {
        self.snapshots
            .get(&self.current)
            .expect("current state must exist")
    }

    /// Get the current session (convenience method)
    pub fn current_session(&self) -> &PackSession {
        &self.current_snapshot().session
    }

    /// Get a mutable reference to the current session
    ///
    /// Note: Changes made here are NOT automatically snapshotted.
    /// Call `commit` after modifications to create a new state.
    pub fn current_session_mut(&mut self) -> &mut PackSession {
        &mut self
            .snapshots
            .get_mut(&self.current)
            .expect("current state must exist")
            .session
    }

    /// Commit the current session changes as a new state
    ///
    /// This creates a new snapshot with the current session state,
    /// advances the state ID, and returns the new ID.
    pub fn commit(&mut self, command: impl Into<String>) -> StateId {
        let new_id = self.next_id;
        self.next_id = self.next_id.next();

        // Clone current session into new snapshot
        let session = self.current_session().clone();
        let snapshot = Snapshot::new(session, Some(command.into()));

        // Record ancestry
        self.ancestry.insert(new_id, self.current);

        // Insert new snapshot and update current
        self.snapshots.insert(new_id, snapshot);
        self.current = new_id;

        new_id
    }

    /// Revert to a previous state by ID
    ///
    /// Returns the reverted-to snapshot, or None if state doesn't exist.
    /// This does NOT create a new state - it moves the current pointer.
    pub fn revert_to(&mut self, state_id: StateId) -> Option<&Snapshot> {
        if self.snapshots.contains_key(&state_id) {
            self.current = state_id;
            Some(self.current_snapshot())
        } else {
            None
        }
    }

    /// Revert to the previous state (parent of current)
    ///
    /// Returns None if already at initial state.
    pub fn revert(&mut self) -> Option<&Snapshot> {
        if let Some(&parent_id) = self.ancestry.get(&self.current) {
            self.current = parent_id;
            Some(self.current_snapshot())
        } else {
            None
        }
    }

    /// Save a named checkpoint at the current state
    ///
    /// Returns the previous checkpoint with this name if it existed.
    pub fn save_checkpoint(&mut self, name: impl Into<String>) -> Option<StateId> {
        self.checkpoints.insert(name.into(), self.current)
    }

    /// Restore a named checkpoint
    ///
    /// Returns the snapshot at the checkpoint, or None if not found.
    pub fn restore_checkpoint(&mut self, name: &str) -> Option<&Snapshot> {
        if let Some(&state_id) = self.checkpoints.get(name) {
            self.current = state_id;
            Some(self.current_snapshot())
        } else {
            None
        }
    }

    /// Get the checkpoint name for the current state, if any
    pub fn current_checkpoint(&self) -> Option<&str> {
        self.checkpoints
            .iter()
            .find(|(_, &id)| id == self.current)
            .map(|(name, _)| name.as_str())
    }

    /// List all checkpoints
    pub fn list_checkpoints(&self) -> Vec<(&str, StateId)> {
        let mut checkpoints: Vec<_> = self
            .checkpoints
            .iter()
            .map(|(name, &id)| (name.as_str(), id))
            .collect();
        checkpoints.sort_by_key(|(_, id)| id.0);
        checkpoints
    }

    /// Delete a checkpoint by name
    pub fn delete_checkpoint(&mut self, name: &str) -> Option<StateId> {
        self.checkpoints.remove(name)
    }

    /// Check if a state ID exists
    pub fn state_exists(&self, state_id: StateId) -> bool {
        self.snapshots.contains_key(&state_id)
    }

    /// Get a snapshot by ID
    pub fn get_snapshot(&self, state_id: StateId) -> Option<&Snapshot> {
        self.snapshots.get(&state_id)
    }

    /// Get the parent state ID for a given state
    pub fn parent_of(&self, state_id: StateId) -> Option<StateId> {
        self.ancestry.get(&state_id).copied()
    }

    /// Get the ancestry chain from a state back to initial state
    pub fn ancestry_chain(&self, state_id: StateId) -> Vec<StateId> {
        let mut chain = vec![state_id];
        let mut current = state_id;

        while let Some(parent) = self.ancestry.get(&current) {
            chain.push(*parent);
            current = *parent;
        }

        chain.reverse();
        chain
    }

    /// Build the execution trace as a list of (state_id, command, is_current, checkpoint_name)
    pub fn trace(&self) -> Vec<TraceEntry> {
        // Get ancestry chain to current state
        let chain = self.ancestry_chain(self.current);
        let chain_set: std::collections::HashSet<_> = chain.iter().copied().collect();

        // Build entries for all states, marking which are in the current chain
        let mut entries: Vec<TraceEntry> = self
            .snapshots
            .iter()
            .map(|(&id, snapshot)| {
                let checkpoint = self
                    .checkpoints
                    .iter()
                    .find(|(_, &cp_id)| cp_id == id)
                    .map(|(name, _)| name.clone());

                TraceEntry {
                    state_id: id,
                    command: snapshot.command.clone(),
                    timestamp: snapshot.timestamp,
                    is_current: id == self.current,
                    in_current_chain: chain_set.contains(&id),
                    checkpoint,
                    parent: self.ancestry.get(&id).copied(),
                }
            })
            .collect();

        entries.sort_by_key(|e| e.state_id.0);
        entries
    }

    /// Reset to initial state, clearing all history
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Reset and start with a new session
    pub fn reset_with_session(&mut self, session: PackSession) {
        let initial_id = StateId(0);
        let mut snapshots = HashMap::new();
        snapshots.insert(initial_id, Snapshot::new(session, None));

        self.snapshots = snapshots;
        self.ancestry.clear();
        self.checkpoints.clear();
        self.current = initial_id;
        self.next_id = StateId(1);
    }

    /// Get the total number of states
    pub fn state_count(&self) -> usize {
        self.snapshots.len()
    }
}

/// An entry in the execution trace
#[derive(Debug, Clone)]
pub struct TraceEntry {
    /// The state ID
    pub state_id: StateId,

    /// The command that created this state (None for initial)
    pub command: Option<String>,

    /// When this state was created
    pub timestamp: DateTime<Utc>,

    /// Whether this is the current state
    pub is_current: bool,

    /// Whether this state is in the ancestry chain to current
    pub in_current_chain: bool,

    /// Checkpoint name if this state has one
    pub checkpoint: Option<String>,

    /// Parent state ID
    pub parent: Option<StateId>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{InputDef, InputType, StepDef};

    #[test]
    fn test_initial_state() {
        let graph = StateGraph::new();
        assert_eq!(graph.current_id(), StateId(0));
        assert!(graph.current_session().is_empty());
    }

    #[test]
    fn test_commit_advances_state() {
        let mut graph = StateGraph::new();

        // Commit first (creates new state), then modify
        let new_id = graph.commit("input test");
        graph.current_session_mut().add_input(InputDef {
            name: "test".to_string(),
            input_type: InputType::String,
            description: None,
            required: true,
            default: None,
        });

        assert_eq!(new_id, StateId(1));
        assert_eq!(graph.current_id(), StateId(1));
        assert!(!graph.current_session().is_empty());
    }

    #[test]
    fn test_revert_to_previous() {
        let mut graph = StateGraph::new();

        // Create state #1: commit first, then modify
        graph.commit("input a");
        graph.current_session_mut().add_input(InputDef {
            name: "a".to_string(),
            input_type: InputType::String,
            description: None,
            required: true,
            default: None,
        });

        // Create state #2: commit first, then modify
        graph.commit("input b");
        graph.current_session_mut().add_input(InputDef {
            name: "b".to_string(),
            input_type: InputType::String,
            description: None,
            required: true,
            default: None,
        });

        assert_eq!(graph.current_session().inputs.len(), 2);

        // Revert to #1
        graph.revert_to(StateId(1));
        assert_eq!(graph.current_id(), StateId(1));
        assert_eq!(graph.current_session().inputs.len(), 1);

        // Revert to #0
        graph.revert_to(StateId(0));
        assert_eq!(graph.current_id(), StateId(0));
        assert!(graph.current_session().is_empty());
    }

    #[test]
    fn test_revert_without_arg() {
        let mut graph = StateGraph::new();

        // Commit first, then modify
        graph.commit("input a");
        graph.current_session_mut().add_input(InputDef {
            name: "a".to_string(),
            input_type: InputType::String,
            description: None,
            required: true,
            default: None,
        });

        graph.commit("input b");
        graph.current_session_mut().add_input(InputDef {
            name: "b".to_string(),
            input_type: InputType::String,
            description: None,
            required: true,
            default: None,
        });

        assert_eq!(graph.current_id(), StateId(2));

        // Revert once
        graph.revert();
        assert_eq!(graph.current_id(), StateId(1));

        // Revert again
        graph.revert();
        assert_eq!(graph.current_id(), StateId(0));

        // Can't revert past initial
        assert!(graph.revert().is_none());
        assert_eq!(graph.current_id(), StateId(0));
    }

    #[test]
    fn test_checkpoints() {
        let mut graph = StateGraph::new();

        // Commit first, then modify
        graph.commit("input a");
        graph.current_session_mut().add_input(InputDef {
            name: "a".to_string(),
            input_type: InputType::String,
            description: None,
            required: true,
            default: None,
        });

        // Save checkpoint
        graph.save_checkpoint("before-analysis");
        assert_eq!(graph.current_checkpoint(), Some("before-analysis"));

        // Add more states
        graph.commit("query step1");
        graph.current_session_mut().add_step(StepDef::new("step1", "T | take 10"));

        graph.commit("query step2");
        graph.current_session_mut().add_step(StepDef::new("step2", "T | take 20"));

        assert_eq!(graph.current_id(), StateId(3));
        assert_eq!(graph.current_session().steps.len(), 2);

        // Restore checkpoint
        graph.restore_checkpoint("before-analysis");
        assert_eq!(graph.current_id(), StateId(1));
        assert!(graph.current_session().steps.is_empty());
    }

    #[test]
    fn test_list_checkpoints() {
        let mut graph = StateGraph::new();

        graph.commit("cmd1");
        graph.save_checkpoint("cp1");

        graph.commit("cmd2");
        graph.save_checkpoint("cp2");

        let checkpoints = graph.list_checkpoints();
        assert_eq!(checkpoints.len(), 2);
        assert_eq!(checkpoints[0].0, "cp1");
        assert_eq!(checkpoints[0].1, StateId(1));
        assert_eq!(checkpoints[1].0, "cp2");
        assert_eq!(checkpoints[1].1, StateId(2));
    }

    #[test]
    fn test_ancestry_chain() {
        let mut graph = StateGraph::new();

        graph.commit("cmd1");
        graph.commit("cmd2");
        graph.commit("cmd3");

        let chain = graph.ancestry_chain(StateId(3));
        assert_eq!(chain, vec![StateId(0), StateId(1), StateId(2), StateId(3)]);
    }

    #[test]
    fn test_trace() {
        let mut graph = StateGraph::new();

        graph.commit("input a");
        graph.save_checkpoint("checkpoint1");
        graph.commit("query b");

        let trace = graph.trace();
        assert_eq!(trace.len(), 3);

        assert_eq!(trace[0].state_id, StateId(0));
        assert!(trace[0].command.is_none());
        assert!(!trace[0].is_current);

        assert_eq!(trace[1].state_id, StateId(1));
        assert_eq!(trace[1].command.as_deref(), Some("input a"));
        assert_eq!(trace[1].checkpoint.as_deref(), Some("checkpoint1"));

        assert_eq!(trace[2].state_id, StateId(2));
        assert!(trace[2].is_current);
    }

    #[test]
    fn test_branching() {
        let mut graph = StateGraph::new();

        // Linear path: #0 -> #1 -> #2
        graph.commit("cmd1");
        graph.commit("cmd2");

        // Go back to #1
        graph.revert_to(StateId(1));

        // Create branch: #1 -> #3
        graph.commit("cmd3-branch");

        assert_eq!(graph.current_id(), StateId(3));

        // Both paths exist
        assert!(graph.state_exists(StateId(2)));
        assert!(graph.state_exists(StateId(3)));

        // Parent of #3 is #1
        assert_eq!(graph.parent_of(StateId(3)), Some(StateId(1)));
        // Parent of #2 is also #1
        assert_eq!(graph.parent_of(StateId(2)), Some(StateId(1)));
    }

    #[test]
    fn test_reset() {
        let mut graph = StateGraph::new();

        graph.commit("cmd1");
        graph.commit("cmd2");
        graph.save_checkpoint("cp");

        graph.reset();

        assert_eq!(graph.current_id(), StateId(0));
        assert_eq!(graph.state_count(), 1);
        assert!(graph.list_checkpoints().is_empty());
    }
}
