use serde_json::Value;

use super::shell::esc;

pub fn render_editor_panel(slug: &str, token_json: &Value) -> String {
    let pretty = serde_json::to_string_pretty(token_json).unwrap_or_default();
    let value_rows = render_value_rows(token_json);
    // JSON string literal — embedded in JS as JSON.parse(...)
    let json_literal = serde_json::to_string(&pretty).unwrap_or_else(|_| r#""""#.into());

    format!(
        r#"<div class="bim-editor" data-slug="{slug}">
  <div class="bim-editor-header">
    <h1>Edit: <code>{slug}</code></h1>
    <cds-content-switcher id="bim-mode-switcher">
      <cds-content-switcher-item value="visual" selected>Visual</cds-content-switcher-item>
      <cds-content-switcher-item value="code">Code</cds-content-switcher-item>
    </cds-content-switcher>
  </div>

  <div id="bim-visual-pane" class="bim-editor-pane">
    <table class="bim-prop-table" id="bim-prop-table">
      <thead><tr><th>Field</th><th>Value</th></tr></thead>
      <tbody>{value_rows}</tbody>
    </table>
  </div>

  <div id="bim-code-pane" class="bim-editor-pane" hidden>
    <div id="bim-codemirror" class="bim-codemirror-host"></div>
  </div>

  <div class="bim-editor-actions">
    <cds-button id="bim-save-btn" kind="primary">Save</cds-button>
    <cds-inline-notification id="bim-save-status" style="display:none"></cds-inline-notification>
  </div>

  <script type="module">
    import {{ EditorState }} from '/static/codemirror.bundle.js';
    import {{ EditorView }} from '/static/codemirror.bundle.js';
    import {{ json }} from '/static/codemirror.bundle.js';

    // SchemaState — bidirectional sync hub between visual + code panes
    const SchemaState = {{
      data: JSON.parse({json_literal}),
      _listeners: [],
      replace(obj) {{
        this.data = obj;
        this._listeners.forEach(fn => fn(obj));
      }},
      subscribe(fn) {{ this._listeners.push(fn); }}
    }};
    window.SchemaState = SchemaState;

    // CodeMirror setup
    let _syncingFromCode = false;
    const view = new EditorView({{
      state: EditorState.create({{
        doc: JSON.stringify(SchemaState.data, null, 2),
        extensions: [
          json(),
          EditorView.lineWrapping,
          EditorView.updateListener.of(update => {{
            if (!update.docChanged) return;
            try {{
              const parsed = JSON.parse(update.state.doc.toString());
              _syncingFromCode = true;
              SchemaState.data = parsed;
              syncVisualFromValue(getFirstValue(parsed));
              _syncingFromCode = false;
            }} catch (_) {{ /* invalid JSON — skip */ }}
          }})
        ]
      }}),
      parent: document.getElementById('bim-codemirror')
    }});
    window.BimEditor = {{ view, slug: '{slug}' }};

    // Helpers to navigate bim.cat.slug.$value
    function getFirstValue(data) {{
      if (data && data.bim) {{
        for (const cat of Object.values(data.bim)) {{
          if (cat && typeof cat === 'object') {{
            for (const entity of Object.values(cat)) {{
              if (entity && entity['$value']) return entity['$value'];
            }}
          }}
        }}
      }}
      return data && data['$value'] ? data['$value'] : {{}};
    }}
    function setFirstValue(data, val) {{
      if (data && data.bim) {{
        for (const catKey of Object.keys(data.bim)) {{
          const cat = data.bim[catKey];
          if (cat && typeof cat === 'object') {{
            for (const eKey of Object.keys(cat)) {{
              if (cat[eKey] && cat[eKey]['$value']) {{
                cat[eKey]['$value'] = val;
                return;
              }}
            }}
          }}
        }}
      }}
      if (data && '$value' in data) data['$value'] = val;
    }}

    // Update visual inputs from a $value object
    function syncVisualFromValue(val) {{
      document.querySelectorAll('.bim-prop-input').forEach(input => {{
        const key = input.dataset.key;
        if (!key || !(key in val)) return;
        const v = val[key];
        input.value = (v !== null && typeof v === 'object') ? JSON.stringify(v) : String(v ?? '');
      }});
    }}

    // Update CodeMirror from visual inputs
    function syncCodeFromVisual() {{
      if (_syncingFromCode) return;
      const newVal = {{}};
      document.querySelectorAll('.bim-prop-input').forEach(input => {{
        const key = input.dataset.key;
        if (!key) return;
        let v = input.value;
        try {{ v = JSON.parse(v); }} catch (_) {{ /* keep string */ }}
        newVal[key] = v;
      }});
      const data = JSON.parse(JSON.stringify(SchemaState.data));
      setFirstValue(data, newVal);
      SchemaState.data = data;
      const docStr = JSON.stringify(data, null, 2);
      view.dispatch(view.state.update({{
        changes: {{ from: 0, to: view.state.doc.length, insert: docStr }}
      }}));
    }}

    // Wire visual input events
    document.querySelectorAll('.bim-prop-input').forEach(input => {{
      input.addEventListener('input', syncCodeFromVisual);
    }});
  </script>

  <script type="module" src="/static/bim-editor.js"></script>
</div>"#,
        slug = esc(slug),
        value_rows = value_rows,
        json_literal = json_literal,
    )
}

fn render_value_rows(token_json: &Value) -> String {
    let value = find_first_value(token_json);
    let Some(obj) = value.as_object() else {
        return "<tr><td colspan=\"2\" class=\"bim-prop-empty\">No $value fields found.</td></tr>"
            .into();
    };

    let mut rows = String::new();
    for (key, val) in obj {
        if key.starts_with('$') {
            continue;
        }
        let val_str = match val {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => String::new(),
            other => serde_json::to_string(other).unwrap_or_default(),
        };
        rows.push_str(&format!(
            "<tr>\
<td class=\"bim-prop-key\"><label for=\"bim-prop-{key}\">{key}</label></td>\
<td class=\"bim-prop-val\">\
<input id=\"bim-prop-{key}\" class=\"bim-prop-input\" type=\"text\" data-key=\"{key}\" value=\"{val_esc}\">\
</td>\
</tr>",
            key = esc(key),
            val_esc = esc(&val_str),
        ));
    }
    if rows.is_empty() {
        rows =
            "<tr><td colspan=\"2\" class=\"bim-prop-empty\">$value has no fields.</td></tr>".into();
    }
    rows
}

fn find_first_value(token_json: &Value) -> Value {
    if let Some(bim) = token_json.get("bim").and_then(|v| v.as_object()) {
        for cat in bim.values() {
            if let Some(entities) = cat.as_object() {
                for entity in entities.values() {
                    if let Some(val) = entity.get("$value") {
                        return val.clone();
                    }
                }
            }
        }
    }
    if let Some(val) = token_json.get("$value") {
        return val.clone();
    }
    Value::Null
}
