/// Standalone GraphQL playground HTML helper.
///
/// Kept separate from transport wiring so tests can prove the surface without
/// touching core GraphQL runtime glue.
pub fn graphql_playground_html(title: &str, endpoint_path: &str) -> String {
    let title = html_escape(title);
    let endpoint_display = html_escape(endpoint_path);
    let endpoint =
        serde_json::to_string(endpoint_path).expect("playground endpoint must serialize");

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}</title>
  <style>
    :root {{
      color-scheme: light;
      --bg: #f5f7fb;
      --panel: #ffffff;
      --border: #d6dbea;
      --ink: #0f172a;
      --muted: #475569;
      --accent: #1d4ed8;
    }}
    body {{
      margin: 0;
      font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      background: linear-gradient(180deg, #eef2ff 0%, var(--bg) 100%);
      color: var(--ink);
    }}
    main {{
      max-width: 960px;
      margin: 0 auto;
      padding: 32px 20px 48px;
    }}
    .card {{
      background: var(--panel);
      border: 1px solid var(--border);
      border-radius: 18px;
      box-shadow: 0 20px 45px rgba(15, 23, 42, 0.08);
      overflow: hidden;
    }}
    header {{
      padding: 24px 24px 16px;
      border-bottom: 1px solid var(--border);
    }}
    h1 {{
      margin: 0;
      font-size: 28px;
    }}
    p {{
      margin: 8px 0 0;
      color: var(--muted);
    }}
    .content {{
      display: grid;
      gap: 16px;
      padding: 24px;
    }}
    textarea {{
      width: 100%;
      min-height: 240px;
      border: 1px solid var(--border);
      border-radius: 14px;
      padding: 16px;
      font: inherit;
      resize: vertical;
      box-sizing: border-box;
    }}
    button {{
      justify-self: start;
      border: 0;
      border-radius: 999px;
      padding: 12px 18px;
      font: inherit;
      font-weight: 700;
      background: var(--accent);
      color: white;
      cursor: pointer;
    }}
    pre {{
      margin: 0;
      padding: 16px;
      border-radius: 14px;
      background: #0f172a;
      color: #e2e8f0;
      overflow: auto;
    }}
  </style>
</head>
<body>
  <main>
    <section class="card">
      <header>
        <h1>{title}</h1>
        <p>GraphQL playground for {endpoint_display}</p>
      </header>
      <div class="content">
        <textarea id="query">query Demo {{ __typename }}</textarea>
        <button id="run">Run query</button>
        <pre id="result">{{
  "hint": "Run a query to see the JSON response"
}}</pre>
      </div>
    </section>
  </main>
  <script>
    const endpoint = {endpoint};
    const query = document.getElementById("query");
    const run = document.getElementById("run");
    const result = document.getElementById("result");

    run.addEventListener("click", async () => {{
      const response = await fetch(endpoint, {{
        method: "POST",
        headers: {{ "content-type": "application/json" }},
        body: JSON.stringify({{ query: query.value }})
      }});
      result.textContent = await response.text();
    }});
  </script>
</body>
</html>"#,
    )
}

fn html_escape(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '&' => "&amp;".chars().collect::<Vec<_>>(),
            '<' => "&lt;".chars().collect::<Vec<_>>(),
            '>' => "&gt;".chars().collect::<Vec<_>>(),
            '"' => "&quot;".chars().collect::<Vec<_>>(),
            '\'' => "&#39;".chars().collect::<Vec<_>>(),
            other => vec![other],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::graphql_playground_html;

    #[test]
    fn playground_html_includes_controls_and_endpoint() {
        let html = graphql_playground_html("Nivasa GraphQL", "/graphql");

        assert!(html.contains("<title>Nivasa GraphQL</title>"));
        assert!(html.contains("<h1>Nivasa GraphQL</h1>"));
        assert!(html.contains("<p>GraphQL playground for /graphql</p>"));
        assert!(html.contains(r#"<textarea id="query">query Demo { __typename }</textarea>"#));
        assert!(html.contains(r#"<button id="run">Run query</button>"#));
        assert!(html.contains(r#"const endpoint = "/graphql";"#));
        assert!(html.contains(r#"body: JSON.stringify({ query: query.value })"#));
    }

    #[test]
    fn playground_html_escapes_title_and_endpoint() {
        let html = graphql_playground_html(
            r#"GraphQL <Playground> & "Safe""#,
            r#"/graphql?topic=demo&mode="play""#,
        );

        assert!(html.contains("&lt;Playground&gt;"));
        assert!(html.contains("&amp;"));
        assert!(html.contains("&quot;Safe&quot;"));
        assert!(html
            .contains(r#"GraphQL playground for /graphql?topic=demo&amp;mode=&quot;play&quot;"#));
        assert!(html.contains(r#"const endpoint = "/graphql?topic=demo&mode=\"play\"";"#));
    }
}
