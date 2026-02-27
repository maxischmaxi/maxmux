use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub type PaneId = String;
pub type WindowId = String;
pub type SessionId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pane {
    pub id: PaneId,
    pub pid: Option<u32>,
    pub cwd: String,
    pub command: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SplitDirection {
    Horizontal, // left | right
    Vertical,   // top / bottom
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LayoutNode {
    Leaf {
        pane_id: PaneId,
    },
    Split {
        direction: SplitDirection,
        ratio: f64,
        children: Box<(LayoutNode, LayoutNode)>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Window {
    pub id: WindowId,
    pub name: String,
    pub panes: Vec<Pane>,
    pub layout: LayoutNode,
    pub active_pane: PaneId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub name: String,
    pub windows: Vec<Window>,
    pub active_window: WindowId,
    pub created_at: u64,
    pub attached_clients: Vec<String>,
}

pub struct SessionManager {
    sessions: HashMap<SessionId, Session>,
}

impl SessionManager {
    pub fn new() -> Self {
        SessionManager {
            sessions: HashMap::new(),
        }
    }

    pub fn create_session(&mut self, name: &str) -> &Session {
        let id = Uuid::new_v4().to_string();
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let session = Session {
            id: id.clone(),
            name: name.to_string(),
            windows: vec![],
            active_window: String::new(),
            created_at,
            attached_clients: vec![],
        };
        self.sessions.insert(id.clone(), session);
        self.sessions.get(&id).unwrap()
    }

    pub fn get(&self, id: &str) -> Option<&Session> {
        self.sessions.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Session> {
        self.sessions.get_mut(id)
    }

    pub fn find_by_name(&self, name: &str) -> Option<&Session> {
        self.sessions.values().find(|s| s.name == name)
    }

    pub fn all(&self) -> Vec<&Session> {
        self.sessions.values().collect()
    }

    pub fn remove(&mut self, id: &str) -> Option<Session> {
        self.sessions.remove(id)
    }

    pub fn add_window(&mut self, session_id: &str, window: Window) -> Option<&Window> {
        let session = self.sessions.get_mut(session_id)?;
        let window_id = window.id.clone();
        let is_first = session.windows.is_empty();
        session.windows.push(window);
        if is_first {
            session.active_window = window_id.clone();
        }
        session.windows.iter().find(|w| w.id == window_id)
    }

    pub fn add_pane_to_window(
        &mut self,
        session_id: &str,
        window_id: &str,
        pane: Pane,
    ) -> bool {
        let session = match self.sessions.get_mut(session_id) {
            Some(s) => s,
            None => return false,
        };
        let window = match session.windows.iter_mut().find(|w| w.id == window_id) {
            Some(w) => w,
            None => return false,
        };
        window.panes.push(pane);
        true
    }

    pub fn remove_pane(
        &mut self,
        session_id: &str,
        window_id: &str,
        pane_id: &str,
    ) -> bool {
        let session = match self.sessions.get_mut(session_id) {
            Some(s) => s,
            None => return false,
        };
        let window = match session.windows.iter_mut().find(|w| w.id == window_id) {
            Some(w) => w,
            None => return false,
        };
        let original_len = window.panes.len();
        window.panes.retain(|p| p.id != pane_id);
        window.panes.len() < original_len
    }

    pub fn remove_window(&mut self, session_id: &str, window_id: &str) -> Option<Window> {
        let session = self.sessions.get_mut(session_id)?;
        let idx = session.windows.iter().position(|w| w.id == window_id)?;
        Some(session.windows.remove(idx))
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_session() {
        let mut mgr = SessionManager::new();
        let session = mgr.create_session("main");
        assert_eq!(session.name, "main");
        assert!(!session.id.is_empty());
        assert_eq!(mgr.all().len(), 1);
    }

    #[test]
    fn test_find_by_name() {
        let mut mgr = SessionManager::new();
        mgr.create_session("work");
        assert!(mgr.find_by_name("work").is_some());
        assert!(mgr.find_by_name("personal").is_none());
    }

    #[test]
    fn test_add_window_sets_active() {
        let mut mgr = SessionManager::new();
        let sid = mgr.create_session("main").id.clone();
        let window = Window {
            id: "w1".into(),
            name: "shell".into(),
            panes: vec![],
            layout: LayoutNode::Leaf {
                pane_id: "p1".into(),
            },
            active_pane: "p1".into(),
        };
        mgr.add_window(&sid, window);
        assert_eq!(mgr.get(&sid).unwrap().active_window, "w1");
    }

    #[test]
    fn test_add_pane_to_window() {
        let mut mgr = SessionManager::new();
        let sid = mgr.create_session("main").id.clone();
        let window = Window {
            id: "w1".into(),
            name: "shell".into(),
            panes: vec![],
            layout: LayoutNode::Leaf {
                pane_id: "p1".into(),
            },
            active_pane: "p1".into(),
        };
        mgr.add_window(&sid, window);
        let pane = Pane {
            id: "p1".into(),
            pid: Some(1234),
            cwd: "/tmp".into(),
            command: "bash".into(),
            title: "bash".into(),
        };
        assert!(mgr.add_pane_to_window(&sid, "w1", pane));
        let w = mgr
            .get(&sid)
            .unwrap()
            .windows
            .iter()
            .find(|w| w.id == "w1")
            .unwrap();
        assert_eq!(w.panes.len(), 1);
    }

    #[test]
    fn test_remove_session() {
        let mut mgr = SessionManager::new();
        let sid = mgr.create_session("temp").id.clone();
        assert!(mgr.remove(&sid).is_some());
        assert_eq!(mgr.all().len(), 0);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let session = Session {
            id: "s1".into(),
            name: "test".into(),
            windows: vec![Window {
                id: "w1".into(),
                name: "shell".into(),
                panes: vec![Pane {
                    id: "p1".into(),
                    pid: Some(123),
                    cwd: "/".into(),
                    command: "zsh".into(),
                    title: "zsh".into(),
                }],
                layout: LayoutNode::Split {
                    direction: SplitDirection::Horizontal,
                    ratio: 0.5,
                    children: Box::new((
                        LayoutNode::Leaf {
                            pane_id: "p1".into(),
                        },
                        LayoutNode::Leaf {
                            pane_id: "p2".into(),
                        },
                    )),
                },
                active_pane: "p1".into(),
            }],
            active_window: "w1".into(),
            created_at: 12345,
            attached_clients: vec![],
        };
        let json = serde_json::to_string(&session).unwrap();
        let parsed: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "s1");
        assert_eq!(parsed.windows[0].panes.len(), 1);
    }
}
