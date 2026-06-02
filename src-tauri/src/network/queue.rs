use std::collections::VecDeque;
use std::sync::Mutex;

/// A single transfer as tracked by the in-memory queue for the UI.
#[derive(Debug, Clone, serde::Serialize)]
pub struct QueueEntry {
    pub transfer_id:    String,
    pub filename:       String,
    pub remote_path:    String,
    pub peer_name:      String,
    pub status:         QueueStatus,
    pub bytes_received: u64,
    pub total:          u64,
    pub started_at:     i64,
}

/// Live status of a queued transfer, serialized for the frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub enum QueueStatus {
    Queued,
    InProgress,
    Complete,
    Failed { reason: String },
    Partial,
}

/// In-memory ring of recent and active transfers, most recent first.
pub struct TransferQueue {
    entries:     Mutex<VecDeque<QueueEntry>>,
    max_history: usize,
}

impl TransferQueue {
    /// Create an empty queue retaining at most `max_history` entries.
    pub fn new(max_history: usize) -> Self {
        Self {
            entries: Mutex::new(VecDeque::new()),
            max_history,
        }
    }

    /// Add an entry to the front, or replace an existing entry with the same
    /// transfer_id in place; trims the oldest entries past max_history.
    pub fn push(&self, entry: QueueEntry) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(slot) = entries.iter_mut().find(|e| e.transfer_id == entry.transfer_id) {
            *slot = entry;
            return;
        }
        entries.push_front(entry);
        while entries.len() > self.max_history {
            entries.pop_back();
        }
    }

    /// Update the status and bytes_received of an existing entry by transfer_id.
    pub fn update(&self, transfer_id: &str, status: QueueStatus, bytes_received: u64) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = entries.iter_mut().find(|e| e.transfer_id == transfer_id) {
            entry.status = status;
            entry.bytes_received = bytes_received;
        }
    }

    /// Return a clone of all entries, most recent first.
    pub fn list(&self) -> Vec<QueueEntry> {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries.iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str) -> QueueEntry {
        QueueEntry {
            transfer_id:    id.to_string(),
            filename:       format!("{id}.bin"),
            remote_path:    format!("/remote/{id}.bin"),
            peer_name:      "peer".into(),
            status:         QueueStatus::Queued,
            bytes_received: 0,
            total:          100,
            started_at:     0,
        }
    }

    #[test]
    fn push_adds_entry() {
        let q = TransferQueue::new(10);
        q.push(entry("a"));
        assert_eq!(q.list().len(), 1);
        assert_eq!(q.list()[0].transfer_id, "a");
    }

    #[test]
    fn update_modifies_existing_entry_by_transfer_id() {
        let q = TransferQueue::new(10);
        q.push(entry("a"));
        q.update("a", QueueStatus::InProgress, 42);
        let list = q.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].bytes_received, 42);
        assert!(matches!(list[0].status, QueueStatus::InProgress));
    }

    #[test]
    fn list_returns_most_recent_first() {
        let q = TransferQueue::new(10);
        q.push(entry("a"));
        q.push(entry("b"));
        q.push(entry("c"));
        let ids: Vec<String> = q.list().into_iter().map(|e| e.transfer_id).collect();
        assert_eq!(ids, vec!["c", "b", "a"]);
    }

    #[test]
    fn push_trims_entries_over_max_history() {
        let q = TransferQueue::new(2);
        q.push(entry("a"));
        q.push(entry("b"));
        q.push(entry("c"));
        let ids: Vec<String> = q.list().into_iter().map(|e| e.transfer_id).collect();
        assert_eq!(ids, vec!["c", "b"]);
    }
}
