use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---- Client -> Server ----

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    #[serde(rename = "attach")]
    Attach {
        session_id: Option<String>,
        cwd: Option<String>,
    },
    #[serde(rename = "detach")]
    Detach,
    #[serde(rename = "input")]
    Input {
        pane_id: String,
        data: String, // base64 encoded
    },
    #[serde(rename = "resize")]
    Resize { cols: u16, rows: u16 },
    #[serde(rename = "command")]
    Command {
        id: String,
        #[serde(default)]
        args: HashMap<String, String>,
    },
    #[serde(rename = "preview")]
    Preview {
        session_id: String,
        cols: u16,
        rows: u16,
    },
    #[serde(rename = "preview-stop")]
    PreviewStop,
    #[serde(rename = "remote-command")]
    RemoteCommand {
        command: String,
        args: Option<Vec<String>>,
        target: Option<String>,
    },
    #[serde(rename = "notes:list")]
    NotesList,
    #[serde(rename = "notes:save")]
    NotesSave {
        id: Option<String>,
        title: String,
        content: String,
    },
    #[serde(rename = "notes:delete")]
    NotesDelete { id: String },
}

// ---- Server -> Client ----

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    #[serde(rename = "output")]
    Output { pane_id: String, data: String },
    #[serde(rename = "state")]
    State {
        sessions: Vec<SessionState>,
        active_session: Option<String>,
    },
    #[serde(rename = "layout")]
    Layout {
        layout: serde_json::Value, // LayoutNode serialized
        pane_rects: HashMap<String, RectData>,
    },
    #[serde(rename = "pane:exited")]
    PaneExited { pane_id: String, exit_code: i32 },
    #[serde(rename = "metrics")]
    Metrics { data: MetricsData },
    #[serde(rename = "cursor-state")]
    CursorState {
        panes: HashMap<String, CursorInfo>,
    },
    #[serde(rename = "process-info")]
    ProcessInfo {
        panes: HashMap<String, String>,
        full: Option<HashMap<String, String>>,
    },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "result")]
    Result {
        success: bool,
        data: Option<serde_json::Value>,
        error: Option<String>,
    },
    #[serde(rename = "preview-layout")]
    PreviewLayout {
        layout: serde_json::Value,
        pane_rects: HashMap<String, RectData>,
    },
    #[serde(rename = "preview-output")]
    PreviewOutput { pane_id: String, data: String },
    #[serde(rename = "notes:data")]
    NotesData { notes: Vec<NoteData> },
    #[serde(rename = "notes:saved")]
    NotesSaved { note: NoteData },
    #[serde(rename = "notes:deleted")]
    NotesDeleted { id: String },
}

// ---- Supporting Types ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub id: String,
    pub name: String,
    pub windows: Vec<WindowState>,
    pub active_window: String,
    pub attached_clients: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    pub id: String,
    pub name: String,
    pub pane_count: usize,
    pub active_pane: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RectData {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorInfo {
    pub visible: bool,
    pub style: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MetricsData {
    pub cpu: f64,
    pub memory: f64,
    pub battery: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteData {
    pub id: String,
    pub title: String,
    pub content: String,
    pub created_at: u64,
    pub updated_at: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_attach_roundtrip() {
        let msg = ClientMessage::Attach {
            session_id: Some("s1".into()),
            cwd: Some("/tmp".into()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"attach\""));
        let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
        assert!(
            matches!(parsed, ClientMessage::Attach { session_id: Some(s), .. } if s == "s1")
        );
    }

    #[test]
    fn test_client_input_roundtrip() {
        let msg = ClientMessage::Input {
            pane_id: "p1".into(),
            data: "aGVsbG8=".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
        assert!(
            matches!(parsed, ClientMessage::Input { pane_id, data } if pane_id == "p1" && data == "aGVsbG8=")
        );
    }

    #[test]
    fn test_server_output_roundtrip() {
        let msg = ServerMessage::Output {
            pane_id: "p1".into(),
            data: "hello".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"output\""));
        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(
            matches!(parsed, ServerMessage::Output { pane_id, data } if pane_id == "p1" && data == "hello")
        );
    }

    #[test]
    fn test_server_state_roundtrip() {
        let msg = ServerMessage::State {
            sessions: vec![SessionState {
                id: "s1".into(),
                name: "main".into(),
                windows: vec![WindowState {
                    id: "w1".into(),
                    name: "shell".into(),
                    pane_count: 2,
                    active_pane: "p1".into(),
                }],
                active_window: "w1".into(),
                attached_clients: vec![],
            }],
            active_session: Some("s1".into()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, ServerMessage::State { sessions, .. } if sessions.len() == 1));
    }

    #[test]
    fn test_all_client_message_variants() {
        // Verify all variants serialize without errors
        let messages: Vec<ClientMessage> = vec![
            ClientMessage::Attach {
                session_id: None,
                cwd: None,
            },
            ClientMessage::Detach,
            ClientMessage::Input {
                pane_id: "p1".into(),
                data: "x".into(),
            },
            ClientMessage::Resize { cols: 80, rows: 24 },
            ClientMessage::Command {
                id: "test".into(),
                args: HashMap::new(),
            },
            ClientMessage::Preview {
                session_id: "s1".into(),
                cols: 80,
                rows: 24,
            },
            ClientMessage::PreviewStop,
            ClientMessage::RemoteCommand {
                command: "test".into(),
                args: None,
                target: None,
            },
            ClientMessage::NotesList,
            ClientMessage::NotesSave {
                id: None,
                title: "t".into(),
                content: "c".into(),
            },
            ClientMessage::NotesDelete { id: "n1".into() },
        ];
        for msg in &messages {
            let json = serde_json::to_string(msg).unwrap();
            let _: ClientMessage = serde_json::from_str(&json).unwrap();
        }
    }
}
