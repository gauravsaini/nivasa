//! # nivasa-http
//!
//! Nivasa framework — http.
//!
//! Placeholder — implementation coming in later phases.

#[cfg(debug_assertions)]
pub mod debug {
    use nivasa_statechart::StatechartSnapshot;

    pub const STATECHART_PATH: &str = "/_nivasa/statechart";
    pub const STATECHART_SCXML_PATH: &str = "/_nivasa/statechart/scxml";
    pub const STATECHART_TRANSITIONS_PATH: &str = "/_nivasa/statechart/transitions";

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct DebugEndpointResponse {
        pub status: u16,
        pub content_type: &'static str,
        pub body: String,
    }

    pub fn handle_statechart_debug_request(
        path: &str,
        snapshot: &StatechartSnapshot,
    ) -> Option<DebugEndpointResponse> {
        match path {
            STATECHART_PATH => Some(DebugEndpointResponse {
                status: 200,
                content_type: "application/json",
                body: serde_json::to_string_pretty(snapshot).expect("snapshot must serialize"),
            }),
            STATECHART_SCXML_PATH => Some(DebugEndpointResponse {
                status: 200,
                content_type: "application/xml",
                body: snapshot.raw_scxml.clone().unwrap_or_default(),
            }),
            STATECHART_TRANSITIONS_PATH => Some(DebugEndpointResponse {
                status: 200,
                content_type: "application/json",
                body: serde_json::to_string_pretty(&snapshot.recent_transitions)
                    .expect("transition log must serialize"),
            }),
            _ => None,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use nivasa_statechart::{TransitionKind, TransitionRecord};

        fn snapshot() -> StatechartSnapshot {
            StatechartSnapshot {
                statechart_name: "Demo".to_string(),
                current_state: "Running".to_string(),
                scxml_hash: "sha256:demo".to_string(),
                raw_scxml: Some("<scxml/>".to_string()),
                recent_transitions: vec![TransitionRecord {
                    kind: TransitionKind::Valid,
                    from: "Idle".to_string(),
                    event: "Start".to_string(),
                    to: Some("Running".to_string()),
                    valid_events: Vec::new(),
                }],
            }
        }

        #[test]
        fn snapshot_endpoint_returns_json() {
            let response = handle_statechart_debug_request(STATECHART_PATH, &snapshot()).unwrap();
            assert_eq!(response.status, 200);
            assert_eq!(response.content_type, "application/json");
            assert!(response.body.contains("\"current_state\": \"Running\""));
        }

        #[test]
        fn scxml_endpoint_returns_raw_document() {
            let response =
                handle_statechart_debug_request(STATECHART_SCXML_PATH, &snapshot()).unwrap();
            assert_eq!(response.content_type, "application/xml");
            assert_eq!(response.body, "<scxml/>");
        }

        #[test]
        fn transitions_endpoint_returns_json() {
            let response =
                handle_statechart_debug_request(STATECHART_TRANSITIONS_PATH, &snapshot()).unwrap();
            assert_eq!(response.content_type, "application/json");
            assert!(response.body.contains("\"event\": \"Start\""));
        }
    }
}
