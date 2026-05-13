use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

use crate::config::ConfigFile;
use crate::error::{OccError, OccResult};
use crate::output;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ConfigHtmlMetadata {
    pub cwd: PathBuf,
    pub target: String,
    pub recommended_path: PathBuf,
    pub loaded_paths: Vec<PathBuf>,
    pub search_paths: Vec<PathBuf>,
    pub doc_root: PathBuf,
    pub default_profile: Option<String>,
    pub init_command: String,
}

pub fn write_html(path: &Path, initial_toml: &str) -> OccResult<()> {
    write_html_with_metadata(path, initial_toml, None)
}

pub fn write_static_html(
    path: &Path,
    initial_toml: &str,
    metadata: &ConfigHtmlMetadata,
) -> OccResult<()> {
    write_html_with_metadata(path, initial_toml, Some(metadata))
}

fn write_html_with_metadata(
    path: &Path,
    initial_toml: &str,
    metadata: Option<&ConfigHtmlMetadata>,
) -> OccResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            OccError::io(
                "doc_root_not_writable",
                format!("Failed to create '{}'", output::display_path(parent)),
                error,
            )
        })?;
    }
    fs::write(path, html_with_metadata(initial_toml, metadata)).map_err(|error| {
        OccError::io(
            "doc_root_not_writable",
            format!("Failed to write '{}'", output::display_path(path)),
            error,
        )
    })
}

pub fn serve(initial_toml: &str, save_path: &Path) -> OccResult<()> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|error| {
        OccError::io(
            "network_error",
            "Failed to start config UI server on 127.0.0.1",
            error,
        )
    })?;
    let address = listener.local_addr().map_err(|error| {
        OccError::io(
            "network_error",
            "Failed to read config UI server address",
            error,
        )
    })?;
    let url = format!("http://{}/", address);
    println!("ui: {}", url);
    println!("config: {}", output::display_path(save_path));
    let _ = open::that(&url);
    for stream in listener.incoming() {
        let stream = stream.map_err(|error| {
            OccError::io(
                "network_error",
                "Failed to accept config UI connection",
                error,
            )
        })?;
        if handle_connection(stream, initial_toml, save_path)? {
            break;
        }
    }
    Ok(())
}

fn html_with_metadata(initial_toml: &str, metadata: Option<&ConfigHtmlMetadata>) -> String {
    html_with_save_path(initial_toml, None, metadata)
}

fn html_with_save_path(
    initial_toml: &str,
    save_path: Option<&Path>,
    metadata: Option<&ConfigHtmlMetadata>,
) -> String {
    let save_button = if save_path.is_some() {
        r#"<button id="save-config" aria-label="Save configuration to file">Save to Config File</button>"#
    } else {
        ""
    };
    let close_button = if save_path.is_some() {
        r#"<button id="close-server" class="secondary" aria-label="Stop the config server">Close Server</button>"#
    } else {
        ""
    };
    let save_path_text = save_path
        .map(|path| {
            format!(
                "<p>Saving config: <code>{}</code></p>",
                escape_html(&output::display_path(path))
            )
        })
        .unwrap_or_default();
    let intro_text = if save_path.is_some() {
        "Edit TOML, then save it through the local server, copy it, download it, or save it with browsers that support the File System Access API."
    } else {
        "Edit TOML, then open a config file, save to an authorized file, copy it, or download it. Browsers cannot silently write local config files."
    };
    let metadata_json = metadata
        .and_then(|metadata| serde_json::to_string(metadata).ok())
        .unwrap_or_else(|| "null".to_string());
    let metadata_block = metadata.map(render_metadata).unwrap_or_default();
    let static_buttons = if save_path.is_none() {
        r#"<button id="open-file" class="secondary" aria-label="Open a config file from disk">Open Config File</button>
<button id="save-opened" class="secondary" aria-label="Save to the previously opened file">Save to Opened File</button>
<button id="copy-path" class="secondary" aria-label="Copy recommended config path">Copy Recommended Path</button>
<button id="copy-init" class="secondary" aria-label="Copy init command">Copy Init Command</button>"#
    } else {
        ""
    };
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>One Code CLI Config</title>
<style>
:root {{ color-scheme: light dark; font-family: Inter, ui-sans-serif, system-ui, sans-serif; }}
body {{ margin: 0; background: #0f172a; color: #e2e8f0; }}
main {{ max-width: 1120px; margin: 0 auto; padding: 40px 24px; }}
.card {{ background: rgba(15,23,42,.86); border: 1px solid #334155; border-radius: 20px; padding: 24px; box-shadow: 0 20px 60px rgba(0,0,0,.25); }}
h1 {{ margin: 0 0 8px; font-size: 32px; }}
p {{ color: #94a3b8; }}
code {{ color: #bae6fd; }}
pre {{ overflow: auto; background: #020617; border: 1px solid #334155; border-radius: 12px; padding: 12px; }}
textarea {{ width: 100%; min-height: 560px; box-sizing: border-box; border-radius: 14px; border: 1px solid #475569; background: #020617; color: #e2e8f0; padding: 16px; font: 14px/1.55 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; }}
.actions {{ display: flex; flex-wrap: wrap; gap: 12px; margin: 16px 0; }}
button {{ border: 0; border-radius: 999px; padding: 10px 16px; cursor: pointer; background: #38bdf8; color: #082f49; font-weight: 700; }}
button.secondary {{ background: #334155; color: #e2e8f0; }}
.status {{ min-height: 24px; color: #86efac; }}
</style>
</head>
<body>
<main>
<section class="card">
<h1>One Code CLI Config</h1>
<p>{}</p>
{}
{}
<div class="actions">
{}
{}
<button id="copy">Copy TOML</button>
<button id="download" class="secondary">Download TOML</button>
<button id="save" class="secondary">Save As</button>
{}
</div>
<label for="toml" class="sr-only">Configuration TOML</label>
<textarea id="toml" spellcheck="false" aria-label="Configuration TOML editor">{}</textarea>
<p id="status" class="status" role="status" aria-live="polite"></p>
</section>
</main>
<script>
const metadata = {};
const textarea = document.getElementById('toml');
const status = document.getElementById('status');
let openedFileHandle = null;
function setStatus(text) {{ status.textContent = text; setTimeout(() => status.textContent = '', 4000); }}
function validateTomlText() {{
  const text = textarea.value.trim();
  if (!text) {{ setStatus('Config TOML is empty.'); return false; }}
  if (!text.includes('version') && !text.includes('[[agents]]')) {{ setStatus('Config should contain version or agents. Run occ config validate after saving.'); }}
  return true;
}}
const saveConfig = document.getElementById('save-config');
if (saveConfig) {{
  saveConfig.addEventListener('click', async () => {{
    if (!validateTomlText()) return;
    const response = await fetch('/config', {{ method: 'POST', headers: {{ 'Content-Type': 'text/plain; charset=utf-8' }}, body: textarea.value }});
    const text = await response.text();
    if (!response.ok) {{ setStatus(text); return; }}
    setStatus(text);
  }});
}}
const openFile = document.getElementById('open-file');
if (openFile) {{
  openFile.addEventListener('click', async () => {{
    if (!window.showOpenFilePicker) {{ setStatus('File System Access API is not available in this browser. Use copy or download instead.'); return; }}
    const [handle] = await window.showOpenFilePicker({{ types: [{{ description: 'TOML', accept: {{ 'application/toml': ['.toml'] }} }}] }});
    const file = await handle.getFile();
    textarea.value = await file.text();
    openedFileHandle = handle;
    setStatus('Opened ' + file.name + '.');
  }});
}}
const saveOpened = document.getElementById('save-opened');
if (saveOpened) {{
  saveOpened.addEventListener('click', async () => {{
    if (!openedFileHandle) {{ setStatus('Open a config file first.'); return; }}
    if (!validateTomlText()) return;
    const writable = await openedFileHandle.createWritable();
    await writable.write(textarea.value);
    await writable.close();
    setStatus('Saved to opened file. Run occ config validate to verify.');
  }});
}}
document.getElementById('copy').addEventListener('click', async () => {{
  await navigator.clipboard.writeText(textarea.value);
  setStatus('Copied TOML to clipboard.');
}});
document.getElementById('download').addEventListener('click', () => {{
  if (!validateTomlText()) return;
  const blob = new Blob([textarea.value], {{ type: 'application/toml' }});
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = 'config.toml';
  a.click();
  URL.revokeObjectURL(url);
  setStatus('Downloaded config.toml.');
}});
document.getElementById('save').addEventListener('click', async () => {{
  if (!window.showSaveFilePicker) {{ setStatus('File System Access API is not available in this browser.'); return; }}
  if (!validateTomlText()) return;
  const handle = await window.showSaveFilePicker({{ suggestedName: 'config.toml', types: [{{ description: 'TOML', accept: {{ 'application/toml': ['.toml'] }} }}] }});
  const writable = await handle.createWritable();
  await writable.write(textarea.value);
  await writable.close();
  setStatus('Saved TOML.');
}});
const copyPath = document.getElementById('copy-path');
if (copyPath) {{
  copyPath.addEventListener('click', async () => {{
    await navigator.clipboard.writeText(metadata?.recommended_path || '');
    setStatus('Copied recommended path.');
  }});
}}
const copyInit = document.getElementById('copy-init');
if (copyInit) {{
  copyInit.addEventListener('click', async () => {{
    await navigator.clipboard.writeText(metadata?.init_command || 'occ config init --user');
    setStatus('Copied init command.');
  }});
}}
const closeServer = document.getElementById('close-server');
if (closeServer) {{
  closeServer.addEventListener('click', async () => {{
    await fetch('/shutdown', {{ method: 'POST' }});
    setStatus('Server closed. You can close this tab.');
  }});
}}
</script>
</body>
</html>
"#,
        intro_text,
        save_path_text,
        metadata_block,
        save_button,
        static_buttons,
        close_button,
        escape_html(initial_toml),
        metadata_json
    )
}

fn render_metadata(metadata: &ConfigHtmlMetadata) -> String {
    let loaded_paths = path_list(&metadata.loaded_paths);
    let search_paths = path_list(&metadata.search_paths);
    format!(
        r#"<div class="metadata">
<p>Recommended config target (<code>{}</code>): <code>{}</code></p>
<p>Current cwd: <code>{}</code></p>
<p>doc_root: <code>{}</code></p>
<p>default_agent: <code>{}</code></p>
<p>Loaded config paths:</p>
<pre>{}</pre>
<p>Search paths:</p>
<pre>{}</pre>
</div>"#,
        escape_html(&metadata.target),
        escape_html(&output::display_path(&metadata.recommended_path)),
        escape_html(&output::display_path(&metadata.cwd)),
        escape_html(&output::display_path(&metadata.doc_root)),
        escape_html(metadata.default_profile.as_deref().unwrap_or("")),
        escape_html(&loaded_paths),
        escape_html(&search_paths),
    )
}

fn path_list(paths: &[PathBuf]) -> String {
    if paths.is_empty() {
        return "(none)".to_string();
    }
    paths
        .iter()
        .map(|path| output::display_path(path))
        .collect::<Vec<_>>()
        .join("\n")
}

fn handle_connection(
    mut stream: TcpStream,
    initial_toml: &str,
    save_path: &Path,
) -> OccResult<bool> {
    let request = read_request(&mut stream)?;
    let Some(header_end) = find_header_end(&request) else {
        write_response(
            &mut stream,
            "400 Bad Request",
            "text/plain; charset=utf-8",
            "Bad request",
        )?;
        return Ok(false);
    };
    let headers = String::from_utf8_lossy(&request[..header_end]);
    let first_line = headers.lines().next().unwrap_or("");
    let body = &request[header_end..];
    if first_line.starts_with("GET / ") || first_line.starts_with("GET /index.html ") {
        let page = html_with_save_path(initial_toml, Some(save_path), None);
        write_response(&mut stream, "200 OK", "text/html; charset=utf-8", &page)?;
        return Ok(false);
    }
    if first_line.starts_with("POST /config ") {
        // CSRF protection: reject POST requests from unexpected origins.
        if !is_localhost_origin(&headers) {
            write_response(
                &mut stream,
                "403 Forbidden",
                "text/plain; charset=utf-8",
                "Cross-origin POST requests are not allowed.",
            )?;
            return Ok(false);
        }
        let text = String::from_utf8_lossy(body).into_owned();
        if let Err(error) = toml::from_str::<ConfigFile>(&text) {
            write_response(
                &mut stream,
                "400 Bad Request",
                "text/plain; charset=utf-8",
                &format!("Config TOML is invalid: {}", error),
            )?;
            return Ok(false);
        }
        if let Some(parent) = save_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                OccError::io(
                    "doc_root_not_writable",
                    format!("Failed to create '{}'", output::display_path(parent)),
                    error,
                )
            })?;
        }
        fs::write(save_path, text).map_err(|error| {
            OccError::io(
                "doc_root_not_writable",
                format!("Failed to write '{}'", output::display_path(save_path)),
                error,
            )
        })?;
        write_response(
            &mut stream,
            "200 OK",
            "text/plain; charset=utf-8",
            "Saved config.toml.",
        )?;
        return Ok(false);
    }
    if first_line.starts_with("POST /shutdown ") {
        if !is_localhost_origin(&headers) {
            write_response(
                &mut stream,
                "403 Forbidden",
                "text/plain; charset=utf-8",
                "Cross-origin POST requests are not allowed.",
            )?;
            return Ok(false);
        }
        write_response(
            &mut stream,
            "200 OK",
            "text/plain; charset=utf-8",
            "Server closed.",
        )?;
        return Ok(true);
    }
    write_response(
        &mut stream,
        "404 Not Found",
        "text/plain; charset=utf-8",
        "Not found",
    )?;
    Ok(false)
}

fn read_request(stream: &mut TcpStream) -> OccResult<Vec<u8>> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = stream.read(&mut buffer).map_err(|error| {
            OccError::io("network_error", "Failed to read config UI request", error)
        })?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        if let Some(header_end) = find_header_end(&request) {
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_length = content_length(&headers);
            if request.len() >= header_end + content_length {
                request.truncate(header_end + content_length);
                break;
            }
        }
        if request.len() > 10 * 1024 * 1024 {
            return Err(OccError::new(
                "request_too_large",
                "Config UI request is too large.",
            ));
        }
    }
    Ok(request)
}

fn find_header_end(request: &[u8]) -> Option<usize> {
    request
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
}

fn content_length(headers: &str) -> usize {
    headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0)
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &str,
) -> OccResult<()> {
    let response = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        content_type,
        body.as_bytes().len(),
        body
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|error| OccError::io("network_error", "Failed to write config UI response", error))
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Check that a POST request originates from localhost.
/// Accepts if the Origin or Referer header starts with http://127.0.0.1 or http://localhost.
/// If neither header is present, the request is also accepted (same-origin requests
/// from some browsers may omit Origin).
fn is_localhost_origin(headers: &str) -> bool {
    let origin = headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.eq_ignore_ascii_case("origin") {
            Some(value.trim())
        } else {
            None
        }
    });
    let referer = headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.eq_ignore_ascii_case("referer") {
            Some(value.trim())
        } else {
            None
        }
    });
    match (origin, referer) {
        (Some(origin), _) => {
            origin.starts_with("http://127.0.0.1") || origin.starts_with("http://localhost")
        }
        (None, Some(referer)) => {
            referer.starts_with("http://127.0.0.1") || referer.starts_with("http://localhost")
        }
        (None, None) => true, // Same-origin requests may omit both headers.
    }
}
