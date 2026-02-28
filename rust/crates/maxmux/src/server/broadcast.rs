use maxmux_ipc::protocol::ServerMessage;
use std::collections::HashMap;
use tokio::sync::mpsc;

/// Routes messages to connected clients.
///
/// Maintains a mapping of client IDs to their message senders, and tracks
/// which session each client is attached to.
pub struct Broadcaster {
    clients: HashMap<String, mpsc::UnboundedSender<ServerMessage>>,
    client_sessions: HashMap<String, String>, // client_id -> session_id
}

impl Broadcaster {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
            client_sessions: HashMap::new(),
        }
    }

    /// Register a new client with its message sender channel.
    pub fn add_client(&mut self, client_id: String, tx: mpsc::UnboundedSender<ServerMessage>) {
        self.clients.insert(client_id, tx);
    }

    /// Remove a client and its session association.
    pub fn remove_client(&mut self, client_id: &str) {
        self.clients.remove(client_id);
        self.client_sessions.remove(client_id);
    }

    /// Associate a client with a session.
    pub fn set_client_session(&mut self, client_id: &str, session_id: &str) {
        self.client_sessions
            .insert(client_id.to_string(), session_id.to_string());
    }

    /// Get the session a client is attached to.
    pub fn get_client_session(&self, client_id: &str) -> Option<&str> {
        self.client_sessions.get(client_id).map(|s| s.as_str())
    }

    /// Send a message to a specific client.
    pub fn send(&self, client_id: &str, msg: ServerMessage) {
        if let Some(tx) = self.clients.get(client_id) {
            let _ = tx.send(msg);
        }
    }

    /// Broadcast a message to all clients attached to the given session.
    pub fn send_to_session(&self, session_id: &str, msg: ServerMessage) {
        for (cid, sid) in &self.client_sessions {
            if sid == session_id {
                self.send(cid, msg.clone());
            }
        }
    }

    /// Return the number of connected clients.
    #[allow(dead_code)]
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    /// Return all connected client IDs.
    #[allow(dead_code)]
    pub fn all_client_ids(&self) -> Vec<String> {
        self.clients.keys().cloned().collect()
    }

    /// Broadcast a message to all connected clients.
    pub fn send_to_all(&self, msg: ServerMessage) {
        for tx in self.clients.values() {
            let _ = tx.send(msg.clone());
        }
    }
}

impl Default for Broadcaster {
    fn default() -> Self {
        Self::new()
    }
}
