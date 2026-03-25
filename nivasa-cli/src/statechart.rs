use nivasa_statechart::{validate_scxml_schema, validator, ScxmlDocument};
use serde_json::Value;
use std::fmt::Write as _;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum DiagramFormat {
    Svg,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub reason: String,
    pub body: String,
}

pub fn validate_statechart_file(path: &Path) -> Result<String, String> {
    validate_scxml_schema(path)
        .map_err(|err| format!("{}: schema validation failed: {err}", path.display()))?;

    let source = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {}", path.display(), err))?;
    let document = ScxmlDocument::from_str(&source)
        .map_err(|err| format!("failed to parse {}: {}", path.display(), err))?;
    let result = validator::validate(&document);

    for warning in &result.warnings {
        println!("warning: {}: {}", path.display(), warning.message);
    }

    if !result.errors.is_empty() {
        for error in &result.errors {
            println!("error: {}: {}", path.display(), error.message);
        }
        return Err(format!("{}: validation failed", path.display()));
    }

    Ok(format!("{}: valid", path.display()))
}

pub fn render_statechart_svg(path: &Path) -> Result<String, String> {
    let source = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {}", path.display(), err))?;
    let document = ScxmlDocument::from_str(&source)
        .map_err(|err| format!("failed to parse {}: {}", path.display(), err))?;
    Ok(render_svg(&document))
}

pub fn render_svg(doc: &ScxmlDocument) -> String {
    let nodes = layout_nodes(doc);
    let edges = collect_edges(doc, &nodes);

    let width = nodes
        .iter()
        .map(|node| node.x + node.width)
        .fold(0.0_f64, f64::max)
        + 80.0;
    let height = nodes
        .iter()
        .map(|node| node.y + node.height)
        .fold(0.0_f64, f64::max)
        + 80.0;

    let mut out = String::new();
    let _ = writeln!(
        out,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img" aria-label="{} statechart">"##,
        escape_xml(doc.metadata.name.as_deref().unwrap_or("Unnamed"))
    );
    out.push_str(
        r##"<defs><marker id="arrowhead" markerWidth="10" markerHeight="7" refX="9" refY="3.5" orient="auto"><polygon points="0 0, 10 3.5, 0 7" fill="#334155"/></marker></defs>"##,
    );
    out.push_str(r##"<rect x="0" y="0" width="100%" height="100%" fill="#f8fafc"/>"##);
    let _ = writeln!(
        out,
        r##"<text x="32" y="32" font-family="Inter, system-ui, sans-serif" font-size="18" font-weight="700" fill="#0f172a">{}</text>"##,
        escape_xml(doc.metadata.name.as_deref().unwrap_or("Unnamed"))
    );
    let _ = writeln!(
        out,
        r##"<text x="32" y="54" font-family="Inter, system-ui, sans-serif" font-size="11" fill="#475569">SCXML hash: {}</text>"##,
        escape_xml(&doc.content_hash())
    );

    for edge in edges {
        let _ = writeln!(
            out,
            r##"<path d="M {:.1} {:.1} L {:.1} {:.1}" fill="none" stroke="#64748b" stroke-width="1.5" marker-end="url(#arrowhead)"/>"##,
            edge.from_x, edge.from_y, edge.to_x, edge.to_y
        );
        let label_x = (edge.from_x + edge.to_x) / 2.0;
        let label_y = (edge.from_y + edge.to_y) / 2.0 - 4.0;
        let _ = writeln!(
            out,
            r##"<text x="{:.1}" y="{:.1}" font-family="Inter, system-ui, sans-serif" font-size="10" fill="#475569">{}</text>"##,
            label_x,
            label_y,
            escape_xml(&edge.label)
        );
    }

    for node in nodes {
        let fill = match node.kind {
            NodeKind::Final => "#d1fae5",
            NodeKind::Parallel => "#e0e7ff",
            NodeKind::Compound => "#fef3c7",
            NodeKind::Atomic => "#ffffff",
        };
        let stroke = match node.kind {
            NodeKind::Final => "#10b981",
            NodeKind::Parallel => "#6366f1",
            NodeKind::Compound => "#d97706",
            NodeKind::Atomic => "#94a3b8",
        };
        let _ = writeln!(
            out,
            r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="12" ry="12" fill="{}" stroke="{}" stroke-width="1.5"/>"#,
            node.x, node.y, node.width, node.height, fill, stroke
        );
        let _ = writeln!(
            out,
            r##"<text x="{:.1}" y="{:.1}" text-anchor="middle" font-family="Inter, system-ui, sans-serif" font-size="12" font-weight="600" fill="#0f172a">{}</text>"##,
            node.x + node.width / 2.0,
            node.y + 22.0,
            escape_xml(&node.label)
        );
        let _ = writeln!(
            out,
            r##"<text x="{:.1}" y="{:.1}" text-anchor="middle" font-family="Inter, system-ui, sans-serif" font-size="10" fill="#475569">{}</text>"##,
            node.x + node.width / 2.0,
            node.y + 39.0,
            escape_xml(&node.subtitle)
        );
    }

    out.push_str("</svg>\n");
    out
}

pub fn diff_statecharts(rev: &str) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(repo_root())
        .args(git_diff_args(rev))
        .output()
        .map_err(|err| format!("failed to run git diff: {err}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if stdout.trim().is_empty() {
        Ok(format!("No SCXML differences found against {rev}."))
    } else {
        Ok(stdout)
    }
}

pub fn inspect_statechart(host: &str, port: u16) -> Result<String, String> {
    let endpoints = [
        "/_nivasa/statechart",
        "/_nivasa/statechart/scxml",
        "/_nivasa/statechart/transitions",
    ];
    let mut rendered = Vec::new();
    let mut last_error = None;

    for endpoint in endpoints {
        match http_get(host, port, endpoint) {
            Ok(response) if response.status == 200 => {
                rendered.push(format!(
                    "== {}:{}{} ==\n{}",
                    host,
                    port,
                    endpoint,
                    format_inspection_body(&response.body)
                ));
            }
            Ok(response) => {
                last_error = Some(format!(
                    "{}:{}{} returned HTTP {} {}",
                    host, port, endpoint, response.status, response.reason
                ));
            }
            Err(err) => {
                last_error = Some(err);
            }
        }
    }

    if rendered.is_empty() {
        Err(last_error.unwrap_or_else(|| {
            format!(
                "no statechart debug endpoint responded on {}:{}",
                host, port
            )
        }))
    } else {
        Ok(rendered.join("\n\n"))
    }
}

pub fn collect_statechart_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let entries =
        fs::read_dir(dir).map_err(|err| format!("failed to read {}: {}", dir.display(), err))?;

    for entry in entries {
        let entry = entry.map_err(|err| format!("failed to read {}: {}", dir.display(), err))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("scxml") {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

pub fn resolve_statechart_path(root: &Path, file: &str) -> Result<PathBuf, String> {
    let candidate = PathBuf::from(file);
    if candidate.exists() {
        return Ok(candidate);
    }

    let from_statecharts = root.join(file);
    if from_statecharts.exists() {
        Ok(from_statecharts)
    } else {
        Err(format!("statechart file not found: {file}"))
    }
}

pub fn statecharts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../statecharts")
}

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

pub fn registry_map(
) -> std::collections::HashMap<&'static str, &'static nivasa_statechart::GeneratedStatechart> {
    let mut map = std::collections::HashMap::new();
    for entry in nivasa_statechart::GENERATED_STATECHARTS {
        map.insert(entry.file_name, entry);
    }
    map
}

pub fn parse_raw_http_response(raw: &str) -> Result<HttpResponse, String> {
    let mut parts = raw.splitn(2, "\r\n\r\n");
    let header_block = parts
        .next()
        .ok_or_else(|| "empty HTTP response".to_string())?;
    let body = parts.next().unwrap_or("").to_string();
    let mut lines = header_block.lines();
    let status_line = lines
        .next()
        .ok_or_else(|| "missing HTTP status line".to_string())?;
    let mut status_parts = status_line.splitn(3, ' ');
    let _http_version = status_parts
        .next()
        .ok_or_else(|| "invalid HTTP status line".to_string())?;
    let status = status_parts
        .next()
        .ok_or_else(|| "invalid HTTP status code".to_string())?
        .parse::<u16>()
        .map_err(|err| format!("invalid HTTP status code: {err}"))?;
    let reason = status_parts.next().unwrap_or("").to_string();

    Ok(HttpResponse {
        status,
        reason,
        body,
    })
}

pub fn format_inspection_body(body: &str) -> String {
    match serde_json::from_str::<Value>(body) {
        Ok(value) => serde_json::to_string_pretty(&value).unwrap_or_else(|_| body.to_string()),
        Err(_) => body.to_string(),
    }
}

fn http_get(host: &str, port: u16, path: &str) -> Result<HttpResponse, String> {
    let mut stream = TcpStream::connect((host, port))
        .map_err(|err| format!("failed to connect to {}:{}: {}", host, port, err))?;
    stream
        .write_all(
            format!(
                "GET {} HTTP/1.1\r\nHost: {}:{}\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
                path, host, port
            )
            .as_bytes(),
        )
        .map_err(|err| format!("failed to send HTTP request: {err}"))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|err| format!("failed to read HTTP response: {err}"))?;

    parse_raw_http_response(&response)
}

#[derive(Debug, Clone)]
struct LayoutNode {
    label: String,
    subtitle: String,
    kind: NodeKind,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Debug, Clone)]
struct LayoutEdge {
    from_x: f64,
    from_y: f64,
    to_x: f64,
    to_y: f64,
    label: String,
}

#[derive(Debug, Clone, Copy)]
enum NodeKind {
    Atomic,
    Compound,
    Parallel,
    Final,
}

fn layout_nodes(doc: &ScxmlDocument) -> Vec<LayoutNode> {
    let mut ordered = Vec::new();
    let mut y = 90.0;
    let mut visited = std::collections::BTreeSet::new();

    fn visit(
        doc: &ScxmlDocument,
        state_id: &str,
        depth: usize,
        y: &mut f64,
        ordered: &mut Vec<LayoutNode>,
        visited: &mut std::collections::BTreeSet<String>,
    ) {
        if !visited.insert(state_id.to_string()) {
            return;
        }

        if let Some(state) = doc.states.get(state_id) {
            let label = state.id.clone();
            let subtitle = match state.state_type {
                nivasa_statechart::StateType::Atomic => "atomic".to_string(),
                nivasa_statechart::StateType::Compound => "compound".to_string(),
                nivasa_statechart::StateType::Parallel => "parallel".to_string(),
                nivasa_statechart::StateType::Final => "final".to_string(),
            };
            let width = (label.len().max(subtitle.len()) as f64 * 8.0).clamp(120.0, 220.0);
            let height = 54.0;
            ordered.push(LayoutNode {
                label,
                subtitle,
                kind: match state.state_type {
                    nivasa_statechart::StateType::Atomic => NodeKind::Atomic,
                    nivasa_statechart::StateType::Compound => NodeKind::Compound,
                    nivasa_statechart::StateType::Parallel => NodeKind::Parallel,
                    nivasa_statechart::StateType::Final => NodeKind::Final,
                },
                x: 40.0 + depth as f64 * 240.0,
                y: *y,
                width,
                height,
            });
            *y += 92.0;

            for child in &state.children {
                visit(doc, child, depth + 1, y, ordered, visited);
            }
        }
    }

    for state_id in &doc.top_level_states {
        visit(doc, state_id, 0, &mut y, &mut ordered, &mut visited);
    }

    ordered
}

fn collect_edges(doc: &ScxmlDocument, nodes: &[LayoutNode]) -> Vec<LayoutEdge> {
    let node_map: std::collections::HashMap<_, _> = nodes
        .iter()
        .map(|node| (node.label.clone(), node))
        .collect();
    let mut edges = Vec::new();

    for state in doc.states.values() {
        let Some(from) = node_map.get(&state.id) else {
            continue;
        };
        for transition in &state.transitions {
            let Some(target) = transition.target.first() else {
                continue;
            };
            let Some(to) = node_map.get(target) else {
                continue;
            };
            edges.push(LayoutEdge {
                from_x: from.x + from.width,
                from_y: from.y + from.height / 2.0,
                to_x: to.x,
                to_y: to.y + to.height / 2.0,
                label: transition
                    .event
                    .clone()
                    .unwrap_or_else(|| "eventless".to_string()),
            });
        }
    }

    edges
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn git_diff_args(rev: &str) -> Vec<String> {
    vec![
        "diff".to_string(),
        "--no-ext-diff".to_string(),
        "--unified=1".to_string(),
        rev.to_string(),
        "--".to_string(),
        "statecharts".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_http_response_extracts_status_and_body() {
        let raw = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"ok\":true}";
        let response = parse_raw_http_response(raw).unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.reason, "OK");
        assert_eq!(response.body, "{\"ok\":true}");
    }

    #[test]
    fn format_inspection_body_pretty_prints_json() {
        let formatted = format_inspection_body("{\"ok\":true}");
        assert!(formatted.contains("\"ok\": true"));
    }

    #[test]
    fn render_svg_contains_state_names() {
        let scxml = r#"<?xml version="1.0"?>
<scxml version="1.0" name="Demo" initial="idle" xmlns="http://www.w3.org/2005/07/scxml">
  <state id="idle">
    <transition event="go" target="running"/>
  </state>
  <final id="running"/>
</scxml>"#;
        let doc = ScxmlDocument::from_str(scxml).unwrap();
        let svg = render_svg(&doc);
        assert!(svg.contains("<svg"));
        assert!(svg.contains("idle"));
        assert!(svg.contains("running"));
        assert!(svg.contains("arrowhead"));
    }

    #[test]
    fn diff_args_include_statecharts_path() {
        assert_eq!(
            git_diff_args("HEAD~1"),
            vec![
                "diff".to_string(),
                "--no-ext-diff".to_string(),
                "--unified=1".to_string(),
                "HEAD~1".to_string(),
                "--".to_string(),
                "statecharts".to_string(),
            ]
        );
    }
}
