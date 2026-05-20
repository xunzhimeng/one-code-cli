use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

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

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn form_html_includes_system_mode_controls() {
        let html = form_html("{}", "{}", "{}", Path::new("config.toml"));
        assert!(html.contains("agent.system_mode"));
        assert!(html.contains("CLAUDE_CONFIG_DIR"));
        assert!(html.contains("CODEX_HOME"));
        assert!(html.contains("OPENCODE_CONFIG_DIR"));
        assert!(html.contains("HOME / USERPROFILE / APPDATA / LOCALAPPDATA"));
    }

    #[test]
    fn form_html_saves_from_active_toml_editor() {
        let html = form_html("{}", "{}", "{}", Path::new("config.toml"));
        assert!(html.contains("configFromActiveEditor"));
        assert!(html.contains("RAW_TOML_DIRTY"));
    }

    #[test]
    fn form_html_models_effort_as_cli_specific_capability() {
        let html = form_html("{}", "{}", "{}", Path::new("config.toml"));
        assert!(html.contains("supports_effort: true"));
        assert!(html.contains("supportsEffort(agent)"));
    }

    #[test]
    fn form_html_distinguishes_cli_type_aliases_from_agent_aliases() {
        let html = form_html("{}", "{}", "{}", Path::new("config.toml"));
        assert!(html.contains("CLI 类型别名"));
        assert!(html.contains("occ run --cli"));
        assert!(html.contains("/cli"));
        assert!(html.contains("Agent 别名"));
        assert!(html.contains("--agent"));
        assert!(html.contains("--agents"));
        assert!(html.contains("alias === cli"));
        assert!(html.contains("alias !== cli"));
    }
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
        body.len(),
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

pub fn serve_form(
    initial_file: &ConfigFile,
    save_path: &Path,
    port: Option<u16>,
    open_browser: bool,
    metadata: ConfigHtmlMetadata,
) -> OccResult<()> {
    let bind_port = port.unwrap_or(0);
    let listener = TcpListener::bind(("127.0.0.1", bind_port)).map_err(|error| {
        OccError::io(
            "network_error",
            format!(
                "Failed to start config UI server on 127.0.0.1:{}",
                bind_port
            ),
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
    println!("press Ctrl+C or click 'Stop Server' in the UI to exit.");
    if open_browser {
        let _ = open::that(&url);
    }

    let initial_json = serde_json::to_string(&initial_file).map_err(|error| {
        OccError::new(
            "serialization_failed",
            format!("Failed to serialize config to JSON: {}", error),
        )
    })?;
    let metadata_json = serde_json::to_string(&metadata).map_err(|error| {
        OccError::new(
            "serialization_failed",
            format!("Failed to serialize metadata to JSON: {}", error),
        )
    })?;
    let detected = crate::cli_defaults::detect();
    let detected_json = serde_json::to_string(&detected).map_err(|error| {
        OccError::new(
            "serialization_failed",
            format!("Failed to serialize detected defaults: {}", error),
        )
    })?;

    let initial_json = Arc::new(initial_json);
    let metadata_json = Arc::new(metadata_json);
    let detected_json = Arc::new(detected_json);
    let save_path = Arc::new(save_path.to_path_buf());
    let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));

    for stream in listener.incoming() {
        if shutdown.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
        let stream = match stream {
            Ok(s) => s,
            Err(error) => {
                eprintln!("config_ui: accept failed: {}", error);
                continue;
            }
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(15)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(15)));

        let initial_json = Arc::clone(&initial_json);
        let metadata_json = Arc::clone(&metadata_json);
        let detected_json = Arc::clone(&detected_json);
        let save_path = Arc::clone(&save_path);
        let shutdown = Arc::clone(&shutdown);
        std::thread::spawn(move || {
            match handle_form_connection(
                stream,
                &initial_json,
                &metadata_json,
                &detected_json,
                &save_path,
            ) {
                Ok(true) => {
                    shutdown.store(true, std::sync::atomic::Ordering::SeqCst);
                    // Unblock the accept loop by connecting to ourselves once.
                    let _ = TcpStream::connect_timeout(&address, Duration::from_millis(500));
                }
                Ok(false) => {}
                Err(error) => {
                    eprintln!("config_ui: request failed: {}", error.message());
                }
            }
        });
    }
    Ok(())
}

fn handle_form_connection(
    mut stream: TcpStream,
    initial_json: &str,
    metadata_json: &str,
    detected_json: &str,
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
        let page = form_html(initial_json, metadata_json, detected_json, save_path);
        write_response(&mut stream, "200 OK", "text/html; charset=utf-8", &page)?;
        return Ok(false);
    }
    if first_line.starts_with("GET /api/cli-defaults ") {
        write_response(
            &mut stream,
            "200 OK",
            "application/json; charset=utf-8",
            detected_json,
        )?;
        return Ok(false);
    }
    if first_line.starts_with("GET /api/config ") {
        // Re-read current file so we always return the latest on-disk state.
        let fresh_json = match fs::read_to_string(save_path) {
            Ok(toml_text) => match toml::from_str::<ConfigFile>(&toml_text)
                .ok()
                .and_then(|cf| serde_json::to_string(&cf).ok())
            {
                Some(json) => json,
                None => initial_json.to_string(),
            },
            Err(_) => initial_json.to_string(),
        };
        write_response(
            &mut stream,
            "200 OK",
            "application/json; charset=utf-8",
            &fresh_json,
        )?;
        return Ok(false);
    }
    if first_line.starts_with("POST /api/config ") {
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
        let parsed: ConfigFile = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(error) => {
                write_response(
                    &mut stream,
                    "400 Bad Request",
                    "text/plain; charset=utf-8",
                    &format!("Config JSON is invalid: {}", error),
                )?;
                return Ok(false);
            }
        };
        let toml_text = match toml::to_string_pretty(&parsed) {
            Ok(value) => value,
            Err(error) => {
                write_response(
                    &mut stream,
                    "400 Bad Request",
                    "text/plain; charset=utf-8",
                    &format!("Config could not be serialized to TOML: {}", error),
                )?;
                return Ok(false);
            }
        };
        if let Some(parent) = save_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                OccError::io(
                    "doc_root_not_writable",
                    format!("Failed to create '{}'", output::display_path(parent)),
                    error,
                )
            })?;
        }
        fs::write(save_path, &toml_text).map_err(|error| {
            OccError::io(
                "doc_root_not_writable",
                format!("Failed to write '{}'", output::display_path(save_path)),
                error,
            )
        })?;
        let body = format!("Saved to {}", output::display_path(save_path));
        write_response(&mut stream, "200 OK", "text/plain; charset=utf-8", &body)?;
        return Ok(false);
    }
    if first_line.starts_with("POST /api/toml-preview ") {
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
        let parsed: ConfigFile = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(error) => {
                write_response(
                    &mut stream,
                    "400 Bad Request",
                    "text/plain; charset=utf-8",
                    &format!("Config JSON is invalid: {}", error),
                )?;
                return Ok(false);
            }
        };
        let toml_text = match toml::to_string_pretty(&parsed) {
            Ok(value) => value,
            Err(error) => {
                write_response(
                    &mut stream,
                    "400 Bad Request",
                    "text/plain; charset=utf-8",
                    &format!("Config could not be serialized to TOML: {}", error),
                )?;
                return Ok(false);
            }
        };
        write_response(
            &mut stream,
            "200 OK",
            "text/plain; charset=utf-8",
            &toml_text,
        )?;
        return Ok(false);
    }
    if first_line.starts_with("POST /api/toml-parse ") {
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
        let parsed: ConfigFile = match toml::from_str(&text) {
            Ok(value) => value,
            Err(error) => {
                write_response(
                    &mut stream,
                    "400 Bad Request",
                    "text/plain; charset=utf-8",
                    &format!("Config TOML is invalid: {}", error),
                )?;
                return Ok(false);
            }
        };
        let json_text = match serde_json::to_string(&parsed) {
            Ok(value) => value,
            Err(error) => {
                write_response(
                    &mut stream,
                    "400 Bad Request",
                    "text/plain; charset=utf-8",
                    &format!("Config could not be serialized to JSON: {}", error),
                )?;
                return Ok(false);
            }
        };
        write_response(
            &mut stream,
            "200 OK",
            "application/json; charset=utf-8",
            &json_text,
        )?;
        return Ok(false);
    }
    if first_line.starts_with("POST /api/shutdown ") {
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
            "Server stopped.",
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

fn form_html(
    initial_json: &str,
    metadata_json: &str,
    detected_json: &str,
    save_path: &Path,
) -> String {
    let save_path_display = output::display_path(save_path);
    let save_path_text = escape_html(&save_path_display);
    let save_path_json =
        serde_json::to_string(&save_path_display).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        r#"<!doctype html>
<html lang="zh-CN" data-theme="light">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>One Code CLI Config</title>
<style>
:root {{
  --bg: #0b0f19;
  --bg-gradient: linear-gradient(135deg, #0b0f19 0%, #111827 100%);
  --surface: rgba(17, 24, 39, 0.7);
  --surface-alt: rgba(31, 41, 55, 0.5);
  --border: rgba(255, 255, 255, 0.08);
  --border-strong: rgba(255, 255, 255, 0.16);
  --text: #f3f4f6;
  --text-muted: #9ca3af;
  --text-subtle: #6b7280;
  --primary: #8b5cf6;
  --primary-text: #ffffff;
  --primary-hover: #a78bfa;
  --primary-soft: rgba(139, 92, 246, 0.15);
  --secondary-bg: rgba(31, 41, 55, 0.8);
  --secondary-text: #e5e7eb;
  --secondary-hover: rgba(55, 65, 81, 0.8);
  --danger: #ef4444;
  --danger-text: #ffffff;
  --danger-hover: #f87171;
  --success: #10b981;
  --error: #f43f5e;
  --code-bg: #030712;
  --code-text: #a78bfa;
  --shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1), 0 2px 4px -1px rgba(0, 0, 0, 0.06);
  --shadow-strong: 0 20px 25px -5px rgba(0, 0, 0, 0.3), 0 10px 10px -5px rgba(0, 0, 0, 0.04);
  --tab-active-bg: rgba(139, 92, 246, 0.2);
  --tab-inactive: #9ca3af;
  --focus: 0 0 0 3px rgba(139, 92, 246, 0.3), 0 0 12px rgba(139, 92, 246, 0.2);
  --font-sans: 'Plus Jakarta Sans', -apple-system, BlinkMacSystemFont, "PingFang SC", "Microsoft YaHei", sans-serif;
  --font-mono: 'JetBrains Mono', ui-monospace, monospace;
  --input-bg: rgba(17, 24, 39, 0.4);
  --input-bg-focus: rgba(17, 24, 39, 0.7);
  --toolbar-bg: rgba(11, 15, 25, 0.85);
  --tabs-bg: rgba(31, 41, 55, 0.3);
  --tab-hover-bg: rgba(255, 255, 255, 0.04);
  --toast-bg: rgba(17, 24, 39, 0.95);
}}
html[data-theme="light"] {{
  --bg: #FAF8F5;
  --bg-gradient: linear-gradient(135deg, #FCFAF7 0%, #F5F1E9 100%);
  --surface: rgba(255, 255, 255, 0.95);
  --surface-alt: rgba(245, 241, 233, 0.7);
  --border: rgba(216, 209, 194, 0.5);
  --border-strong: rgba(180, 170, 150, 0.5);
  --text: #2E2B2A;
  --text-muted: #6E6762;
  --text-subtle: #A59E98;
  --primary: #6B5EAE;
  --primary-text: #ffffff;
  --primary-hover: #584A9A;
  --primary-soft: rgba(107, 94, 174, 0.12);
  --secondary-bg: #EFEBE4;
  --secondary-text: #3A3432;
  --secondary-hover: #E2DBD0;
  --danger: #dc2626;
  --danger-text: #ffffff;
  --danger-hover: #b91c1c;
  --success: #059669;
  --error: #e11d48;
  --code-bg: #F5EFE6;
  --code-text: #6B5EAE;
  --shadow: 0 4px 12px -2px rgba(180, 170, 150, 0.12), 0 2px 6px -1px rgba(180, 170, 150, 0.08);
  --shadow-strong: 0 20px 32px -8px rgba(180, 170, 150, 0.25), 0 8px 16px -4px rgba(180, 170, 150, 0.15);
  --tab-active-bg: rgba(107, 94, 174, 0.15);
  --tab-inactive: #7E736E;
  --focus: 0 0 0 3px rgba(107, 94, 174, 0.25);
  --input-bg: #ffffff;
  --input-bg-focus: #ffffff;
  --toolbar-bg: rgba(252, 250, 247, 0.85);
  --tabs-bg: rgba(230, 224, 214, 0.5);
  --tab-hover-bg: rgba(0, 0, 0, 0.03);
  --toast-bg: rgba(255, 255, 255, 0.95);
}}
* {{ box-sizing: border-box; }}
html, body {{ height: 100%; }}
body {{
  margin: 0;
  background: var(--bg-gradient);
  background-attachment: fixed;
  color: var(--text);
  font-family: var(--font-sans);
  font-size: 14px;
  line-height: 1.55;
}}
::-webkit-scrollbar {{
  width: 8px;
  height: 8px;
}}
::-webkit-scrollbar-track {{
  background: transparent;
}}
::-webkit-scrollbar-thumb {{
  background: var(--border-strong);
  border-radius: 4px;
}}
::-webkit-scrollbar-thumb:hover {{
  background: var(--primary);
}}
main {{ max-width: 1280px; margin: 0 auto; padding: 24px 24px 96px; }}
h1 {{ margin: 0 0 4px; font-size: 24px; font-weight: 700; letter-spacing: -0.5px; background: linear-gradient(135deg, var(--text) 30%, var(--primary-hover) 100%); -webkit-background-clip: text; -webkit-text-fill-color: transparent; }}
h2 {{ margin: 0 0 14px; font-size: 18px; font-weight: 600; letter-spacing: -0.3px; }}
p {{ margin: 0 0 8px; color: var(--text-muted); }}
.muted {{ color: var(--text-muted); font-size: 13px; }}
code {{
  background: var(--code-bg); color: var(--code-text);
  padding: 2px 6px; border-radius: 6px;
  font-family: var(--font-mono); font-size: 12px;
  border: 1px solid var(--border);
}}

header {{ display: flex; align-items: center; justify-content: space-between; gap: 16px; margin-bottom: 24px; flex-wrap: wrap; padding-bottom: 16px; border-bottom: 1px solid var(--border); }}
header .info {{ flex: 1 1 auto; min-width: 240px; }}
header .info p {{ font-size: 13px; margin: 4px 0 0; }}
.brand-row {{ display: flex; align-items: center; gap: 12px; }}
.brand-tag {{ display: inline-block; background: var(--primary-soft); border: 1px solid var(--primary); color: var(--primary-hover); font-size: 11px; padding: 2px 10px; border-radius: 999px; font-weight: 700; letter-spacing: .5px; text-transform: uppercase; }}

.toolbar {{
  position: sticky; top: 0; z-index: 10;
  background: var(--toolbar-bg);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  padding: 14px 16px;
  border: 1px solid var(--border);
  border-radius: 12px;
  display: flex; gap: 10px; flex-wrap: wrap; align-items: center;
  margin-bottom: 24px;
  box-shadow: var(--shadow);
}}
.toolbar .spacer {{ flex: 1; }}
.toolbar .status {{ min-height: 22px; color: var(--success); font-size: 13px; font-weight: 500; }}
.toolbar .status.error {{ color: var(--error); }}

button {{
  border: 0; border-radius: 10px; padding: 10px 18px; cursor: pointer;
  background: var(--primary); color: var(--primary-text); font-weight: 600; font-size: 13px;
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  display: inline-flex; align-items: center; justify-content: center; gap: 6px;
}}
button:hover {{
  background: var(--primary-hover);
  transform: translateY(-2px);
  box-shadow: 0 6px 20px rgba(139, 92, 246, 0.35);
}}
button:active {{ transform: translateY(0); }}
button:focus-visible {{ outline: none; box-shadow: var(--focus); }}
button.secondary {{ background: var(--secondary-bg); color: var(--secondary-text); border: 1px solid var(--border); }}
button.secondary:hover {{ background: var(--secondary-hover); box-shadow: 0 4px 12px rgba(0,0,0,0.15); }}
button.danger {{ background: var(--danger); color: var(--danger-text); }}
button.danger:hover {{ background: var(--danger-hover); box-shadow: 0 6px 20px rgba(239, 68, 68, 0.35); }}
button.ghost {{ background: transparent; color: var(--text-muted); padding: 6px 12px; border: 1px solid transparent; }}
button.ghost:hover {{ background: var(--surface-alt); color: var(--text); border-color: var(--border); }}
button.small {{ padding: 6px 12px; font-size: 12px; border-radius: 8px; }}

input[type=text], input[type=number], input[type=password], textarea, select {{
  width: 100%; box-sizing: border-box;
  border: 1px solid var(--border-strong); background: var(--input-bg); color: var(--text);
  padding: 10px 14px; border-radius: 10px;
  font-family: var(--font-mono); font-size: 13px;
  transition: all 0.2s;
}}
input[type=text]:focus, input[type=number]:focus, input[type=password]:focus, textarea:focus, select:focus {{
  outline: none; border-color: var(--primary); box-shadow: var(--focus);
  background: var(--input-bg-focus);
}}
textarea {{ min-height: 80px; resize: vertical; line-height: 1.5; }}

/* Styled Switch instead of checkbox */
.switch {{
  position: relative;
  display: inline-block;
  width: 44px;
  height: 24px;
}}
.switch input {{
  opacity: 0;
  width: 0;
  height: 0;
}}
.slider {{
  position: absolute;
  cursor: pointer;
  top: 0; left: 0; right: 0; bottom: 0;
  background-color: var(--border-strong);
  transition: .3s;
  border-radius: 24px;
}}
.slider:before {{
  position: absolute;
  content: "";
  height: 18px;
  width: 18px;
  left: 3px;
  bottom: 3px;
  background-color: white;
  transition: .3s;
  border-radius: 50%;
}}
input:checked + .slider {{
  background-color: var(--primary);
}}
input:focus + .slider {{
  box-shadow: var(--focus);
}}
input:checked + .slider:before {{
  transform: translateX(20px);
}}

label {{ display: block; font-size: 12px; color: var(--text-muted); margin-bottom: 6px; font-weight: 600; letter-spacing: 0.3px; text-transform: uppercase; }}
.field-hint {{ font-size: 11px; color: var(--text-subtle); margin-top: 6px; }}
.mode-note {{
  margin: 8px 0 12px; padding: 10px 12px; border-radius: 10px;
  background: var(--surface-alt); border: 1px solid var(--border);
  color: var(--text-muted); font-size: 12px;
}}
.mode-note code {{ white-space: normal; overflow-wrap: anywhere; }}

.tabs {{
  display: flex; gap: 8px; margin-bottom: 24px;
  background: var(--tabs-bg); padding: 6px;
  border-radius: 14px; border: 1px solid var(--border);
  overflow-x: auto;
}}
.tab {{
  padding: 10px 20px; cursor: pointer; border-radius: 10px;
  color: var(--tab-inactive); font-weight: 600; font-size: 13px;
  white-space: nowrap; user-select: none; transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
}}
.tab:hover {{ color: var(--text); background: var(--tab-hover-bg); }}
.tab.active {{ background: var(--primary); color: var(--primary-text); box-shadow: 0 6px 16px rgba(139, 92, 246, 0.35); }}

.tab-panel {{ display: none; }}
.tab-panel.active {{ display: block; animation: fade .2s ease-out; }}
@keyframes fade {{ from {{ opacity: 0; transform: translateY(4px); }} to {{ opacity: 1; transform: none; }} }}

.card {{
  background: var(--surface); backdrop-filter: blur(12px); -webkit-backdrop-filter: blur(12px);
  border: 1px solid var(--border);
  border-radius: 16px; padding: 24px; margin-bottom: 20px;
  box-shadow: var(--shadow);
  transition: all 0.25s;
}}
.card:hover {{
  box-shadow: var(--shadow-strong);
  border-color: var(--border-strong);
}}
.card-title {{ display: flex; align-items: center; gap: 10px; margin-bottom: 8px; }}
.card-title h2 {{ margin: 0; font-size: 18px; font-weight: 600; }}
.card-desc {{ margin-bottom: 18px; }}

.grid {{ display: grid; grid-template-columns: repeat(2, 1fr); gap: 16px 20px; }}
.grid .full {{ grid-column: 1 / -1; }}
@media (max-width: 720px) {{ .grid {{ grid-template-columns: 1fr; }} }}

.kv-list {{ display: flex; flex-direction: column; gap: 10px; margin-bottom: 12px; }}
.kv-row {{ display: grid; grid-template-columns: 220px 1fr auto; gap: 10px; align-items: center; }}
@media (max-width: 720px) {{ .kv-row {{ grid-template-columns: 1fr; }} }}

.mapping-table {{ display: flex; flex-direction: column; gap: 10px; }}
.mapping-row {{
  display: grid; grid-template-columns: 220px 1fr; gap: 16px; align-items: center;
  padding: 12px 16px; border-radius: 12px; background: var(--surface-alt);
  border: 1px solid var(--border);
  transition: all 0.2s;
}}
.mapping-row:hover {{
  border-color: var(--border-strong);
  background: var(--surface);
}}
.mapping-row .mapping-label {{ font-size: 13px; }}
.mapping-row .mapping-label strong {{ color: var(--text); font-weight: 600; }}
.mapping-row .mapping-label .muted {{ font-size: 12px; }}
.mapping-row .mapping-label .field-hint {{ margin-top: 4px; }}
@media (max-width: 720px) {{ .mapping-row {{ grid-template-columns: 1fr; }} }}

.agents-layout {{
  display: grid; grid-template-columns: 280px 1fr; gap: 20px;
  align-items: start;
}}
@media (max-width: 900px) {{ .agents-layout {{ grid-template-columns: 1fr; }} }}

.agents-side {{
  background: var(--surface); border: 1px solid var(--border);
  border-radius: 16px; padding: 18px; box-shadow: var(--shadow);
  position: sticky; top: 100px;
}}
.agents-side .agents-list-head {{
  display: flex; align-items: center; gap: 8px; margin-bottom: 14px;
}}
.agents-side .agents-list-head h2 {{ margin: 0; font-size: 15px; font-weight: 700; }}
.agents-side .agents-list-head .count {{ background: var(--primary-soft); color: var(--primary-hover); font-size: 11px; padding: 2px 8px; border-radius: 999px; font-weight: 700; }}
.agents-side .agents-list-head .spacer {{ flex: 1; }}
.agent-list {{ display: flex; flex-direction: column; gap: 6px; max-height: 60vh; overflow-y: auto; }}
.agent-item {{
  padding: 12px 14px; border-radius: 10px; cursor: pointer;
  border: 1px solid transparent;
  display: flex; flex-direction: column; gap: 4px;
  transition: all 0.2s;
}}
.agent-item:hover {{ background: var(--surface-alt); transform: translateX(4px); }}
.agent-item.active {{ background: var(--primary-soft); border-color: var(--primary); box-shadow: inset 3px 0 0 var(--primary); }}
.agent-item .name {{ font-weight: 600; font-size: 13px; }}
.agent-item .cli {{ font-size: 11px; color: var(--text-muted); font-family: var(--font-mono); }}
.agent-item.active .cli {{ color: var(--text-muted); }}
.agent-empty {{ color: var(--text-muted); font-size: 12px; padding: 16px 4px; text-align: center; }}

.agent-pane {{
  background: var(--surface); border: 1px solid var(--border);
  border-radius: 16px; padding: 24px; box-shadow: var(--shadow);
  min-height: 240px;
}}
.agent-pane-empty {{
  display: flex; align-items: center; justify-content: center;
  min-height: 320px; color: var(--text-muted); font-size: 13px;
  flex-direction: column; gap: 14px;
}}
.agent-section-title {{
  font-size: 13px; font-weight: 700; color: var(--text);
  margin: 20px 0 10px; padding-top: 18px;
  border-top: 1px dashed var(--border);
  text-transform: uppercase; letter-spacing: 0.5px;
}}
.agent-section-title:first-of-type {{ border-top: 0; padding-top: 0; }}
.secret-row {{ display: flex; gap: 8px; align-items: center; }}
.secret-row input {{ flex: 1; }}

.metadata {{ font-size: 12px; }}
.metadata p {{ margin: 6px 0; }}
.metadata pre {{
  background: var(--code-bg); color: var(--code-text);
  padding: 12px 14px; border-radius: 8px; margin: 6px 0 14px;
  white-space: pre-wrap; word-break: break-all;
  font-family: var(--font-mono); font-size: 12px;
  border: 1px solid var(--border);
}}

dialog {{
  background: var(--surface); color: var(--text);
  border: 1px solid var(--border-strong); border-radius: 16px;
  padding: 0; max-width: 90vw; width: 800px;
  box-shadow: var(--shadow-strong);
  backdrop-filter: blur(16px);
  -webkit-backdrop-filter: blur(16px);
  animation: dlgShow 0.3s cubic-bezier(0.16, 1, 0.3, 1) forwards;
}}
@keyframes dlgShow {{
  from {{ opacity: 0; transform: scale(0.95) translateY(10px); }}
  to {{ opacity: 1; transform: scale(1) translateY(0); }}
}}
dialog::backdrop {{ background: rgba(11, 15, 25, 0.6); backdrop-filter: blur(4px); }}
dialog .dlg-head {{ display: flex; align-items: center; justify-content: space-between; padding: 14px 20px; border-bottom: 1px solid var(--border); }}
dialog pre {{
  margin: 0; padding: 20px; max-height: 70vh; overflow: auto;
  background: var(--code-bg); color: var(--text);
  font-family: var(--font-mono); font-size: 12px;
  line-height: 1.5;
}}

.toggle-group {{ display: flex; gap: 4px; background: var(--tabs-bg); padding: 4px; border-radius: 10px; border: 1px solid var(--border); }}
.toggle-group button {{ padding: 6px 12px; font-size: 12px; background: transparent; color: var(--text-muted); border-radius: 6px; border: 0; font-weight: 600; }}
.toggle-group button:hover {{ background: var(--tab-hover-bg); transform: none; box-shadow: none; }}
.toggle-group button.active {{ background: var(--tab-active-bg); color: var(--primary-hover); box-shadow: var(--shadow); }}

/* Toast notifications */
.toast-container {{
  position: fixed;
  top: 24px;
  right: 24px;
  z-index: 9999;
  display: flex;
  flex-direction: column;
  gap: 12px;
  pointer-events: none;
}}
.toast {{
  min-width: 280px;
  max-width: 420px;
  padding: 14px 20px;
  border-radius: 12px;
  background: var(--toast-bg);
  backdrop-filter: blur(16px);
  border-left: 4px solid var(--primary);
  border-top: 1px solid var(--border);
  border-right: 1px solid var(--border);
  border-bottom: 1px solid var(--border);
  box-shadow: var(--shadow-strong);
  color: var(--text);
  font-weight: 500;
  display: flex;
  align-items: center;
  gap: 12px;
  pointer-events: auto;
  animation: slideIn 0.3s cubic-bezier(0.16, 1, 0.3, 1) forwards;
}}
.toast.success {{ border-left-color: var(--success); }}
.toast.error {{ border-left-color: var(--error); }}
.toast.info {{ border-left-color: var(--primary); }}
@keyframes slideIn {{
  from {{ opacity: 0; transform: translateY(-20px) scale(0.95); }}
  to {{ opacity: 1; transform: translateY(0) scale(1); }}
}}
@keyframes fadeOut {{
  from {{ opacity: 1; transform: translateY(0) scale(1); }}
  to {{ opacity: 0; transform: translateY(-20px) scale(0.95); }}
}}
</style>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Plus+Jakarta+Sans:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;600&display=swap" rel="stylesheet">
</head>
<body>
<div id="toast-container" class="toast-container"></div>
<main>

<header>
  <div class="info">
    <div class="brand-row">
      <h1>One Code CLI</h1>
      <span class="brand-tag" data-i18n="brand.tag">配置</span>
    </div>
    <p data-i18n="header.subtitle">在表单中编辑 occ 配置；保存后写入到 <code>{save_path}</code>。关闭服务器只会停止 UI，已保存的更改会保留。</p>
  </div>
  <div style="display:flex;gap:8px;align-items:center;flex-wrap:wrap;">
    <div class="toggle-group" role="group" aria-label="Theme">
      <button id="theme-light" type="button" data-i18n="theme.light">浅色</button>
      <button id="theme-dark" type="button" data-i18n="theme.dark">深色</button>
    </div>
    <div class="toggle-group" role="group" aria-label="Language">
      <button id="lang-zh" type="button">中文</button>
      <button id="lang-en" type="button">EN</button>
    </div>
  </div>
</header>

<div class="toolbar">
  <button id="save-btn" data-i18n="action.save">保存到文件</button>
  <button id="reload-btn" class="secondary" data-i18n="action.reload">重新加载</button>
  <button id="preview-btn" class="secondary" data-i18n="action.preview">预览</button>
  <span class="spacer"></span>
  <span class="status" id="status" role="status" aria-live="polite"></span>
  <button id="stop-btn" class="danger" data-i18n="action.stop">停止服务</button>
</div>

<div class="tabs" role="tablist">
  <div class="tab active" data-tab="general" role="tab" tabindex="0" data-i18n="tab.general">常规</div>
  <div class="tab" data-tab="mapping" role="tab" tabindex="0" data-i18n="tab.mapping">CLI 映射</div>
  <div class="tab" data-tab="agents" role="tab" tabindex="0" data-i18n="tab.agents">Agents</div>
  <div class="tab" data-tab="toml-editor" role="tab" tabindex="0" data-i18n="tab.toml_editor">TOML 编辑器</div>
  <div class="tab" data-tab="context" role="tab" tabindex="0" data-i18n="tab.context">上下文</div>
</div>

<section class="tab-panel active" data-panel="general">
  <div class="card">
    <div class="card-title"><h2 data-i18n="general.basic">基础设置</h2></div>
    <p class="card-desc muted" data-i18n="general.basic.desc">控制 occ 自身行为的全局选项。</p>
    <div class="grid">
      <div>
        <label data-i18n="field.version">版本</label>
        <input type="number" id="version" min="1" />
      </div>
      <div>
        <label data-i18n="field.default_agent">默认 agent</label>
        <select id="default-agent"></select>
        <div class="field-hint" data-i18n="field.default_agent.hint">未指定 --agent / --cli 时使用的 agent。</div>
      </div>
      <div>
        <label data-i18n="field.doc_root">运行记录目录</label>
        <input type="text" id="doc-root" data-ph="ph.doc_root" />
      </div>
      <div>
        <label data-i18n="field.timeout">默认超时</label>
        <input type="text" id="default-timeout" data-ph="ph.timeout" />
        <div class="field-hint" data-i18n="field.timeout.hint">支持 none / 60s / 5m / 2h。</div>
      </div>
    </div>
  </div>

  <div class="card">
    <div class="card-title"><h2 data-i18n="general.proxy">代理转发</h2></div>
    <p class="card-desc muted" data-i18n="general.proxy.desc">是否把代理相关环境变量转发给子 CLI。</p>
    <div class="grid">
      <div style="display:flex;align-items:center;gap:12px;">
        <label class="switch">
          <input type="checkbox" id="proxy-enabled" />
          <span class="slider"></span>
        </label>
        <label for="proxy-enabled" style="margin:0;cursor:pointer;font-weight:600;font-size:14px;text-transform:none;" data-i18n="field.proxy_enabled">启用代理转发</label>
      </div>
      <div></div>
      <div class="full">
        <label data-i18n="field.proxy_keys">转发的环境变量（每行一个）</label>
        <textarea id="proxy-env-keys" data-ph="ph.proxy_keys"></textarea>
      </div>
    </div>
  </div>
</section>

<section class="tab-panel" data-panel="mapping">
  <div class="card">
    <div class="card-title"><h2 data-i18n="mapping.defaults">CLI 默认 agent</h2></div>
    <p class="card-desc muted" data-i18n="mapping.defaults.desc">每种 CLI（Claude Code / Codex 等）使用 <code>--cli</code> 时默认调用的 agent。一个 CLI 可以有多个 agent，例如 Claude Code 同时配置 Anthropic 官方接口和 DeepSeek 兼容接口的 agent。</p>
    <div class="mapping-table" id="cli-defaults-list"></div>
  </div>
  <div class="card">
    <div class="card-title"><h2 data-i18n="mapping.aliases">CLI 类型别名</h2></div>
    <p class="card-desc muted" data-i18n="mapping.aliases.desc">给 CLI 类型取短名，只用于 <code>occ run --cli</code> 和 <code>/cli</code>。例如 <code>cc</code> 代表 Claude Code；原名 <code>claude</code>、<code>codex</code> 等不需要重复填写。</p>
    <div class="mapping-table" id="cli-aliases-list"></div>
  </div>
</section>

<section class="tab-panel" data-panel="agents">
  <p class="muted" data-i18n="agents.desc" style="margin-bottom:14px;">同一个 CLI 可以有多个 agent，例如 Claude Code 同时用 Anthropic 官方和 DeepSeek 兼容后端。在 config_dir 和 env 中配置每个 agent 自己的系统目录、API key / base URL / model。</p>
  <div class="agents-layout">
    <aside class="agents-side">
      <div class="agents-list-head">
        <h2 data-i18n="agents.title">Agents</h2>
        <span class="count" id="agent-count">0</span>
        <span class="spacer"></span>
        <button id="add-agent" class="small" data-i18n="action.add_agent">+ 新建</button>
      </div>
      <div class="agent-list" id="agent-list"></div>
    </aside>
    <div class="agent-pane" id="agent-pane">
      <div class="agent-pane-empty">
        <div data-i18n="agents.empty">尚未选择 agent</div>
        <button id="add-agent-empty" class="small" data-i18n="action.add_agent">+ 新建 agent</button>
      </div>
    </div>
  </div>
</section>

<section class="tab-panel" data-panel="toml-editor">
  <div class="card">
    <div class="card-title"><h2 data-i18n="toml_editor.title">原始 TOML 编辑与同步</h2></div>
    <p class="card-desc muted" data-i18n="toml_editor.desc">可以直接在这里查看和编辑底层 TOML 配置内容。支持双向实时同步。</p>
    <textarea id="raw-toml-textarea" spellcheck="false" style="min-height: 480px; font-family: var(--font-mono); line-height: 1.5;"></textarea>
    <div class="actions" style="margin-top: 14px; display: flex; gap: 10px;">
      <button id="sync-to-form-btn" data-i18n="action.sync_to_form">⚡ 同步到表单</button>
      <button id="sync-from-form-btn" class="secondary" data-i18n="action.sync_from_form">🔄 从表单同步</button>
    </div>
  </div>
</section>

<section class="tab-panel" data-panel="context">
  <div class="card metadata">
    <div class="card-title"><h2 data-i18n="context.title">配置上下文</h2></div>
    <p class="card-desc muted" data-i18n="context.desc">当前运行时检测到的路径与配置来源（只读）。</p>
    <div id="metadata-block"></div>
  </div>
</section>

<dialog id="preview-dialog">
  <form method="dialog" style="margin:0;">
    <div class="dlg-head">
      <strong data-i18n="preview.title">预览</strong>
      <button class="secondary small" type="submit" data-i18n="action.close">关闭</button>
    </div>
    <pre id="preview-text"></pre>
  </form>
</dialog>

</main>

<script>
const META = {metadata_json};
const SAVE_PATH = {save_path_json};
const DETECTED = {detected_json};
let CONFIG = {initial_json};
let SELECTED_AGENT_INDEX = null;
let RAW_TOML_DIRTY = false;

const CLI_DEFS = [
  {{
    id: 'claude',
    label: 'Claude Code',
    default_command: 'claude',
    config_env: 'CLAUDE_CONFIG_DIR',
    supports_effort: true,
    env: [
      {{ key: 'ANTHROPIC_API_KEY', label_zh: 'API Key', label_en: 'API Key', secret: true,
         desc_zh: 'Anthropic 官方 API Key，或第三方兼容后端的 token。',
         desc_en: 'Anthropic official API key, or token for an Anthropic-compatible proxy.' }},
      {{ key: 'ANTHROPIC_AUTH_TOKEN', label_zh: 'Auth Token', label_en: 'Auth Token', secret: true,
         desc_zh: '可选，部分代理需要使用 auth token 而不是 API key。',
         desc_en: 'Optional. Some proxies use an auth token instead of an API key.' }},
      {{ key: 'ANTHROPIC_BASE_URL', label_zh: 'Base URL', label_en: 'Base URL',
         desc_zh: '自定义 API 入口，例如使用 DeepSeek/Kimi 兼容 Anthropic 协议时填写。留空走官方。',
         desc_en: 'Custom API endpoint, e.g. when routing through a DeepSeek/Kimi proxy.' }},
      {{ key: 'ANTHROPIC_MODEL', label_zh: '模型覆盖 (ANTHROPIC_MODEL)', label_en: 'Model override (ANTHROPIC_MODEL)',
         desc_zh: '覆盖默认模型，例如 claude-sonnet-4-6。',
         desc_en: 'Override default main model, e.g. claude-sonnet-4-6.' }},
      {{ key: 'ANTHROPIC_SMALL_FAST_MODEL', label_zh: '轻量模型 (ANTHROPIC_SMALL_FAST_MODEL)', label_en: 'Small/fast model (ANTHROPIC_SMALL_FAST_MODEL)',
         desc_zh: '用于工具调用等轻量任务的模型，例如 claude-haiku-4-5。', desc_en: 'Model for lightweight tasks like tool calls, e.g. claude-haiku-4-5.' }},
    ],
  }},
  {{
    id: 'codex',
    label: 'Codex CLI',
    default_command: 'codex',
    config_env: 'CODEX_HOME',
    supports_effort: true,
    env: [
      {{ key: 'OPENAI_API_KEY', label_zh: 'API Key', label_en: 'API Key', secret: true,
         desc_zh: 'OpenAI API Key，或 OpenAI 兼容后端的 token。',
         desc_en: 'OpenAI API key, or token for an OpenAI-compatible backend.' }},
      {{ key: 'OPENAI_BASE_URL', label_zh: 'Base URL', label_en: 'Base URL',
         desc_zh: '自定义 API 入口，例如 DeepSeek、Kimi、ZhipuAI 等 OpenAI 兼容服务。',
         desc_en: 'Custom API endpoint for OpenAI-compatible services.' }},
      {{ key: 'OPENAI_MODEL', label_zh: '模型覆盖 (OPENAI_MODEL)', label_en: 'Model override (OPENAI_MODEL)',
         desc_zh: '覆盖默认模型。',
         desc_en: 'Override default model.' }},
      {{ key: 'OPENAI_ORG_ID', label_zh: '组织 ID (OPENAI_ORG_ID)', label_en: 'Organization ID (OPENAI_ORG_ID)',
         desc_zh: '可选，OpenAI 组织 ID。', desc_en: 'Optional OpenAI organization ID.' }},
      {{ key: 'OPENAI_PROJECT_ID', label_zh: '项目 ID (OPENAI_PROJECT_ID)', label_en: 'Project ID (OPENAI_PROJECT_ID)',
         desc_zh: '可选，OpenAI 项目 ID。', desc_en: 'Optional OpenAI project ID.' }},
    ],
  }},
  {{
    id: 'opencode',
    label: 'opencode',
    default_command: 'opencode',
    config_env: 'OPENCODE_CONFIG_DIR',
    supports_effort: false,
    env: [
      {{ key: 'OPENCODE_API_KEY', label_zh: 'API Key', label_en: 'API Key', secret: true,
         desc_zh: 'opencode 使用的 API Key。', desc_en: 'API key used by opencode.' }},
      {{ key: 'OPENCODE_BASE_URL', label_zh: 'Base URL', label_en: 'Base URL',
         desc_zh: '自定义后端入口。', desc_en: 'Custom backend endpoint.' }},
      {{ key: 'OPENCODE_MODEL', label_zh: '模型覆盖 (OPENCODE_MODEL)', label_en: 'Model override (OPENCODE_MODEL)',
         desc_zh: '覆盖默认模型设置。', desc_en: 'Override the default model.' }},
    ],
  }},
  {{
    id: 'gemini',
    label: 'Gemini CLI',
    default_command: 'gemini',
    config_env: 'HOME / USERPROFILE / APPDATA / LOCALAPPDATA',
    supports_effort: false,
    env: [
      {{ key: 'GEMINI_API_KEY', label_zh: 'API Key', label_en: 'API Key', secret: true,
         desc_zh: 'Google AI Studio 的 Gemini API Key。', desc_en: 'Gemini API key from Google AI Studio.' }},
      {{ key: 'GOOGLE_API_KEY', label_zh: 'Google API Key', label_en: 'Google API Key', secret: true,
         desc_zh: '可选，部分场景使用 Google Cloud 的通用 API Key。', desc_en: 'Optional, generic Google Cloud API key.' }},
      {{ key: 'GOOGLE_CLOUD_PROJECT', label_zh: 'Google Cloud 项目 (GOOGLE_CLOUD_PROJECT)', label_en: 'Google Cloud Project (GOOGLE_CLOUD_PROJECT)',
         desc_zh: '可选，Google Cloud 项目 ID，用于 Vertex AI 等场景。', desc_en: 'Optional Google Cloud project ID for Vertex AI etc.' }},
      {{ key: 'GOOGLE_CLOUD_LOCATION', label_zh: 'Google Cloud 区域 (GOOGLE_CLOUD_LOCATION)', label_en: 'Google Cloud Location (GOOGLE_CLOUD_LOCATION)',
         desc_zh: '可选，Google Cloud 区域，例如 us-central1。', desc_en: 'Optional Google Cloud region, e.g. us-central1.' }},
    ],
  }},
];
const CLI_DEFS_BY_ID = Object.fromEntries(CLI_DEFS.map(d => [d.id, d]));

const I18N = {{
  "zh-CN": {{
    "brand.tag": "配置",
    "header.subtitle": () => "在表单中编辑 occ 配置；保存后写入到 <code>" + escape(SAVE_PATH) + "</code>。关闭服务器只会停止 UI，已保存的更改会保留。",
    "theme.light": "浅色",
    "theme.dark": "深色",
    "action.save": "保存到文件",
    "action.reload": "重新加载",
    "action.preview": "预览",
    "action.stop": "停止服务",
    "action.add_mapping": "+ 添加映射",
    "action.add_alias": "+ 添加别名",
    "action.add_agent": "+ 新建 agent",
    "action.remove": "删除",
    "action.duplicate": "复制",
    "action.close": "关闭",
    "action.show": "显示",
    "action.hide": "隐藏",
    "tab.general": "常规",
    "tab.mapping": "CLI 映射",
    "tab.agents": "Agents",
    "tab.toml_editor": "TOML 编辑器",
    "toml_editor.title": "原始 TOML 编辑与同步",
    "toml_editor.desc": "可以直接在这里查看和编辑底层 TOML 配置内容。支持双向实时同步。",
    "action.sync_to_form": "⚡ 同步到表单",
    "action.sync_from_form": "🔄 从表单同步",
    "tab.context": "上下文",
    "general.basic": "基础设置",
    "general.basic.desc": "控制 occ 自身行为的全局选项。",
    "general.proxy": "代理转发",
    "general.proxy.desc": "是否把代理相关环境变量转发给子 CLI。",
    "field.version": "版本",
    "field.default_agent": "默认 agent",
    "field.default_agent.hint": "未指定 --agent / --cli 时使用的 agent。",
    "field.doc_root": "运行记录目录",
    "field.timeout": "默认超时",
    "field.timeout.hint": "支持 none / 60s / 5m / 2h。",
    "field.proxy_enabled": "启用代理转发",
    "field.proxy_keys": "转发的环境变量（每行一个）",
    "ph.doc_root": "~/.occ",
    "ph.timeout": "none / 60s / 5m",
    "ph.proxy_keys": "HTTP_PROXY",
    "ph.cli_select": "选择 CLI",
    "ph.agent_select": "选择 agent",
    "ph.alias": "别名，每行一个（例如 ds）",
    "ph.cli_alias": "CLI 类型别名，每行一个（例如 cc）",
    "ph.agent_name": "例如：claude-anthropic",
    "ph.command": "可选，留空使用默认",
    "ph.kv_key": "key",
    "ph.kv_value": "value",
    "mapping.defaults": "CLI 默认 agent",
    "mapping.defaults.desc": "每种 CLI（Claude Code / Codex 等）使用 <code>--cli</code> 时默认调用的 agent。一个 CLI 可以有多个 agent，例如 Claude Code 同时配置 Anthropic 官方接口和 DeepSeek 兼容接口的 agent。",
    "mapping.aliases": "CLI 类型别名",
    "mapping.aliases.desc": "给 CLI 类型取短名，只用于 <code>occ run --cli</code> 和 <code>/cli</code>。例如 <code>cc</code> 代表 Claude Code；原名 <code>claude</code>、<code>codex</code> 等不需要重复填写。",
    "mapping.aliases.row_hint": "用于 --cli 和 /cli；原名会自动忽略。",
    "mapping.no_default": "（不指定）",
    "agents.title": "Agents",
    "agents.desc": "同一个 CLI 可以有多个 agent，例如 Claude Code 同时用 Anthropic 官方和 DeepSeek 兼容后端。在 config_dir 和 env 中配置每个 agent 自己的系统目录、API key / base URL / model。",
    "agents.empty": "尚未选择 agent。",
    "agents.untitled": "未命名 agent",
    "context.title": "配置上下文",
    "context.desc": "当前运行时检测到的路径与配置来源（只读）。",
    "preview.title": "预览（保存前）",
    "agent.section.basic": "基础",
    "agent.section.command": "可执行文件",
    "agent.section.system": "CLI 系统目录 / 隔离",
    "agent.section.env": "常用环境变量",
    "agent.section.env_extra": "其它 env（每行 KEY=VALUE）",
    "agent.section.advanced": "高级 / 透传参数",
    "agent.name": "名称 *",
    "agent.cli_type": "CLI 类型 *",
    "agent.aliases": "Agent 别名（每行一个）",
    "agent.aliases.hint": "用于 --agent、--agents 和 /agent，精确选择这个 agent。",
    "agent.command": "命令名称（例如 claude）",
    "agent.model": "主模型名称 (model)",
    "agent.effort": "推理/思考强度 (effort)",
    "agent.path": "可执行文件路径 (覆盖 command)",
    "agent.system_mode": "CLI 系统目录模式",
    "agent.system_mode.default": "使用默认 CLI 系统目录",
    "agent.system_mode.isolated": "使用隔离 config_dir",
    "agent.system_mode.default_note": "默认模式不设置隔离目录，子 CLI 使用它平时自己的登录态和配置目录，环境变量默认继承父进程。",
    "agent.system_mode.isolated_note": "隔离模式会设置 config_dir，并把子进程环境切到 strict；agent.env 会覆盖父环境，env_allowlist 只放行必要父变量。",
    "agent.system_env": "隔离时设置的 CLI 环境变量",
    "action.use_suggested_dir": "使用建议目录",
    "agent.config_dir": "CLI 系统配置目录 (config_dir)",
    "agent.env_mode": "环境变量传递模式 (env_mode)",
    "agent.env_mode.inherit": "完全继承父进程环境 (inherit)",
    "agent.env_mode.strict": "严格沙箱模式，仅继承白名单 (strict)",
    "agent.env_allowlist": "环境变量继承白名单 (env_allowlist，每行一个)",
    "agent.default_timeout": "默认运行超时时间 (default_timeout)",
    "agent.args_strategy": "命令行参数替换策略 (args_strategy)",
    "agent.args_strategy.builtin": "内置默认策略 (builtin)",
    "agent.args_strategy.append": "在后面追加 (append)",
    "agent.args_strategy.override": "完全覆盖默认 (override)",
    "agent.prompt_via": "Prompt 输入传输通路 (prompt_via)",
    "agent.prompt_via.default": "内置默认传输通路",
    "agent.prompt_via.stdin": "标准输入流 (stdin)",
    "agent.prompt_via.arg": "直接命令行参数 (arg)",
    "agent.prompt_via.file": "临时文件输入 (file)",
    "agent.prompt_via.file_indirection": "临时文件间接引用 (file-indirection)",
    "agent.prompt_via.arg_or_file_indirection": "命令行参数或文件间接引用自适应",
    "agent.args": "命令行启动参数列表 (args，完全覆盖，每行一个)",
    "agent.extra_args": "追加启动参数列表 (extra_args，每行一个)",
    "agent.interactive_args": "交互模式参数列表 (interactive_args，每行一个)",
    "agent.non_interactive_args": "非交互模式参数列表 (non_interactive_args，每行一个)",
    "agent.resume_args": "会话恢复参数列表 (resume_args，每行一个)",
    "agent.env_extra": "其它自定义环境变量（每行 KEY=VALUE）",
    "agent.env.detected_from": "检测到的默认配置来源：",
    "agent.env.detected_label": "已检测",
    "action.use_detected": "使用此值",
    "action.use_detected_all": "全部填入检测到的值",
    "agent.no_cli": "未选择 CLI 类型",
    "msg.agent_count": (n) => n + " 个",
    "msg.need_name": "每个 agent 都需要 name。",
    "msg.need_cli": (name) => "agent " + name + " 需要选择 CLI 类型。",
    "msg.duplicate_name": (name) => "已存在同名 agent：" + name,
    "msg.save_failed": "保存失败：",
    "msg.reload_failed": "重新加载失败",
    "msg.reload_failed_e": "重新加载失败：",
    "msg.reloaded": "已重新加载。",
    "msg.stop_confirm": "停止本地配置服务？可以再次运行 `occ settings` 重新启动。",
    "msg.stopped": "服务已停止，可以关闭此页面。",
    "msg.no_meta": "（无上下文信息）",
    "msg.context.cwd": "当前目录",
    "msg.context.doc_root": "运行记录目录",
    "msg.context.recommended": "推荐保存路径",
    "msg.context.loaded": "已加载的配置文件",
    "msg.context.search": "搜索路径",
    "msg.none": "（无）",
    "msg.confirm_remove": (name) => "删除 agent " + name + "？",
    "msg.no_agents_in_cli": "当前 CLI 没有 agent",
  }},
  "en": {{
    "brand.tag": "CONFIG",
    "header.subtitle": () => "Edit your <code>occ</code> config in a form. Saves go to <code>" + escape(SAVE_PATH) + "</code>. Closing this server stops the UI; saved changes persist.",
    "theme.light": "Light",
    "theme.dark": "Dark",
    "action.save": "Save",
    "action.reload": "Reload",
    "action.preview": "Preview",
    "action.stop": "Stop server",
    "action.add_mapping": "+ Add mapping",
    "action.add_alias": "+ Add alias",
    "action.add_agent": "+ New agent",
    "action.remove": "Remove",
    "action.duplicate": "Duplicate",
    "action.close": "Close",
    "action.show": "Show",
    "action.hide": "Hide",
    "tab.general": "General",
    "tab.mapping": "CLI mapping",
    "tab.agents": "Agents",
    "tab.toml_editor": "TOML Editor",
    "toml_editor.title": "Raw TOML Editor",
    "toml_editor.desc": "Directly view and edit the raw underlying TOML configuration. Full bi-directional sync is supported.",
    "action.sync_to_form": "⚡ Sync to Form",
    "action.sync_from_form": "🔄 Sync from Form",
    "tab.context": "Context",
    "general.basic": "Basics",
    "general.basic.desc": "Global options that control occ itself.",
    "general.proxy": "Proxy forwarding",
    "general.proxy.desc": "Whether to forward proxy env vars to child CLIs.",
    "field.version": "Version",
    "field.default_agent": "Default agent",
    "field.default_agent.hint": "Used when neither --agent nor --cli is given.",
    "field.doc_root": "Run record directory",
    "field.timeout": "Default timeout",
    "field.timeout.hint": "Accepts none / 60s / 5m / 2h.",
    "field.proxy_enabled": "Enable proxy forwarding",
    "field.proxy_keys": "Forwarded env vars (one per line)",
    "ph.doc_root": "~/.occ",
    "ph.timeout": "none / 60s / 5m",
    "ph.proxy_keys": "HTTP_PROXY",
    "ph.cli_select": "Select CLI",
    "ph.agent_select": "Select agent",
    "ph.alias": "Aliases, one per line, e.g. ds",
    "ph.cli_alias": "CLI type aliases, one per line, e.g. cc",
    "ph.agent_name": "e.g. claude-anthropic",
    "ph.command": "Optional, blank uses default",
    "ph.kv_key": "key",
    "ph.kv_value": "value",
    "mapping.defaults": "Default agent per CLI",
    "mapping.defaults.desc": "Which agent each CLI calls by default with <code>--cli</code>. A CLI can have many agents, e.g. Claude Code with Anthropic official and Claude Code with a DeepSeek-compatible proxy.",
    "mapping.aliases": "CLI type aliases",
    "mapping.aliases.desc": "Short names for CLI types only, used by <code>occ run --cli</code> and <code>/cli</code>. For example, <code>cc</code> means Claude Code; canonical names like <code>claude</code> and <code>codex</code> do not need to be repeated.",
    "mapping.aliases.row_hint": "Used by --cli and /cli; canonical names are ignored.",
    "mapping.no_default": "(unset)",
    "agents.title": "Agents",
    "agents.desc": "One CLI can have multiple agents (e.g. Claude Code with Anthropic and Claude Code with a DeepSeek-compatible proxy). Use config_dir and env to set per-agent system config, API key / base URL / model.",
    "agents.empty": "No agent selected.",
    "agents.untitled": "Untitled agent",
    "context.title": "Config context",
    "context.desc": "Paths and config sources detected at runtime (read-only).",
    "preview.title": "Preview (before saving)",
    "agent.section.basic": "Basics",
    "agent.section.command": "Executable",
    "agent.section.system": "CLI system root / isolation",
    "agent.section.env": "Common env vars",
    "agent.section.env_extra": "Other env (KEY=VALUE per line)",
    "agent.section.advanced": "Advanced / passthrough",
    "agent.name": "Name *",
    "agent.cli_type": "CLI type *",
    "agent.aliases": "Agent aliases (one per line)",
    "agent.aliases.hint": "Used by --agent, --agents, and /agent to select this exact agent.",
    "agent.command": "command name (e.g. claude)",
    "agent.model": "model (agent-side label)",
    "agent.effort": "effort (reasoning level)",
    "agent.path": "executable path (overrides default command)",
    "agent.system_mode": "CLI system directory mode",
    "agent.system_mode.default": "Use default CLI system directory",
    "agent.system_mode.isolated": "Use isolated config_dir",
    "agent.system_mode.default_note": "Default mode does not set an isolated directory. The child CLI uses its normal login state and config directory, and inherits the parent environment.",
    "agent.system_mode.isolated_note": "Isolated mode sets config_dir and switches the child process to strict env; agent.env overrides parent values and env_allowlist only allows selected parent variables.",
    "agent.system_env": "CLI env set for isolation",
    "action.use_suggested_dir": "Use suggested directory",
    "agent.config_dir": "config_dir (CLI system root)",
    "agent.env_mode": "env_mode (environment inheritance)",
    "agent.env_mode.inherit": "inherit (inherit parent environment)",
    "agent.env_mode.strict": "strict (rebuild from allowlist)",
    "agent.env_allowlist": "env_allowlist (one parent env var per line)",
    "agent.default_timeout": "default_timeout",
    "agent.args_strategy": "args_strategy",
    "agent.args_strategy.builtin": "built-in strategy (builtin)",
    "agent.args_strategy.append": "append arguments (append)",
    "agent.args_strategy.override": "override completely (override)",
    "agent.prompt_via": "prompt_via",
    "agent.prompt_via.default": "(default)",
    "agent.prompt_via.stdin": "Standard Input (stdin)",
    "agent.prompt_via.arg": "Command line argument (arg)",
    "agent.prompt_via.file": "Temporary file (file)",
    "agent.prompt_via.file_indirection": "File indirection (file-indirection)",
    "agent.prompt_via.arg_or_file_indirection": "Arg or File indirection (auto)",
    "agent.args": "args (override, one per line)",
    "agent.extra_args": "extra_args (append, one per line)",
    "agent.interactive_args": "interactive_args (one per line)",
    "agent.non_interactive_args": "non_interactive_args (one per line)",
    "agent.resume_args": "resume_args (one per line)",
    "agent.env_extra": "Env vars not listed above, one KEY=VALUE per line.",
    "agent.env.detected_from": "Detected from:",
    "agent.env.detected_label": "detected",
    "action.use_detected": "Use this",
    "action.use_detected_all": "Use all detected values",
    "agent.no_cli": "No CLI type selected",
    "msg.agent_count": (n) => n + "",
    "msg.need_name": "Every agent needs a name.",
    "msg.need_cli": (name) => "Agent " + name + " needs a CLI type.",
    "msg.duplicate_name": (name) => "Duplicate agent name: " + name,
    "msg.save_failed": "Save failed: ",
    "msg.reload_failed": "Reload failed.",
    "msg.reload_failed_e": "Reload failed: ",
    "msg.reloaded": "Reloaded.",
    "msg.stop_confirm": "Stop the local config server? Restart with `occ settings`.",
    "msg.stopped": "Server stopped. You can close this tab.",
    "msg.no_meta": "(no context)",
    "msg.context.cwd": "cwd",
    "msg.context.doc_root": "doc_root",
    "msg.context.recommended": "recommended config path",
    "msg.context.loaded": "loaded config files",
    "msg.context.search": "search paths",
    "msg.none": "(none)",
    "msg.confirm_remove": (name) => "Remove agent " + name + "?",
    "msg.no_agents_in_cli": "No agent for this CLI",
  }},
}};

let LANG = localStorage.getItem('occ.lang') || 'zh-CN';
let THEME = localStorage.getItem('occ.theme') || (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');

function t(key, ...args) {{
  const dict = I18N[LANG] || I18N['zh-CN'];
  const v = dict[key];
  if (typeof v === 'function') return v(...args);
  if (v == null) return key;
  return v;
}}

function escape(s) {{ return String(s == null ? '' : s).replace(/[&<>"']/g, c => ({{'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}}[c])); }}
function lines(value) {{ return value ? value.split('\n').map(s => s.trim()).filter(Boolean) : []; }}
function joinLines(arr) {{ return Array.isArray(arr) ? arr.join('\n') : ''; }}

function applyI18n() {{
  document.documentElement.lang = LANG === 'en' ? 'en' : 'zh-CN';
  document.querySelectorAll('[data-i18n]').forEach(el => {{
    el.innerHTML = t(el.getAttribute('data-i18n'));
  }});
  document.querySelectorAll('[data-ph]').forEach(el => {{
    el.placeholder = t(el.getAttribute('data-ph'));
  }});
  document.getElementById('lang-zh').classList.toggle('active', LANG === 'zh-CN');
  document.getElementById('lang-en').classList.toggle('active', LANG === 'en');
  refreshAgentList();
  refreshMappingDropdowns();
  renderAgentPane();
  renderMetadata();
  refreshDefaultAgentDropdown();
  updateAgentCount();
}}
function applyTheme() {{
  document.documentElement.setAttribute('data-theme', THEME);
  document.getElementById('theme-light').classList.toggle('active', THEME === 'light');
  document.getElementById('theme-dark').classList.toggle('active', THEME === 'dark');
}}

document.getElementById('lang-zh').addEventListener('click', () => {{ LANG = 'zh-CN'; localStorage.setItem('occ.lang', LANG); applyI18n(); }});
document.getElementById('lang-en').addEventListener('click', () => {{ LANG = 'en'; localStorage.setItem('occ.lang', LANG); applyI18n(); }});
document.getElementById('theme-light').addEventListener('click', () => {{ THEME = 'light'; localStorage.setItem('occ.theme', THEME); applyTheme(); }});
document.getElementById('theme-dark').addEventListener('click', () => {{ THEME = 'dark'; localStorage.setItem('occ.theme', THEME); applyTheme(); }});

document.querySelectorAll('.tab').forEach(tab => {{
  const activate = async () => {{
    if (tab.dataset.tab === 'toml-editor' && !RAW_TOML_DIRTY) {{
      await syncFromForm(true);
    }}
    document.querySelectorAll('.tab').forEach(x => x.classList.remove('active'));
    document.querySelectorAll('.tab-panel').forEach(x => x.classList.remove('active'));
    tab.classList.add('active');
    document.querySelector('[data-panel="' + tab.dataset.tab + '"]').classList.add('active');
  }};
  tab.addEventListener('click', activate);
  tab.addEventListener('keydown', e => {{ if (e.key === 'Enter' || e.key === ' ') {{ e.preventDefault(); activate(); }} }});
}});

document.getElementById('raw-toml-textarea').addEventListener('input', () => {{
  RAW_TOML_DIRTY = true;
}});

const statusEl = document.getElementById('status');
function showToast(text, type = 'success') {{
  const container = document.getElementById('toast-container');
  if (!container) return;
  const toast = document.createElement('div');
  toast.className = `toast ${{type}}`;
  let icon = '⚡';
  if (type === 'success') icon = '✅';
  else if (type === 'error') icon = '❌';
  else if (type === 'info') icon = 'ℹ️';
  toast.innerHTML = `<span style="font-size:16px;">${{icon}}</span><span style="flex:1;">${{escape(text)}}</span>`;
  container.appendChild(toast);
  setTimeout(() => {{
    toast.style.animation = 'fadeOut 0.3s cubic-bezier(0.16, 1, 0.3, 1) forwards';
    setTimeout(() => toast.remove(), 300);
  }}, 4000);
}}
function setStatus(text, isError) {{
  if (!text) return;
  statusEl.textContent = text;
  statusEl.classList.toggle('error', !!isError);
  showToast(text, isError ? 'error' : 'success');
  if (text) setTimeout(() => {{ if (statusEl.textContent === text) statusEl.textContent = ''; }}, 5000);
}}

async function syncFromForm(silent = false) {{
  const cfg = collectConfig();
  try {{
    const r = await fetch('/api/toml-preview', {{
      method: 'POST',
      headers: {{ 'Content-Type': 'application/json' }},
      body: JSON.stringify(cfg)
    }});
    if (!r.ok) {{
      const err = await r.text();
      showToast(err, 'error');
      return;
    }}
    const toml = await r.text();
    document.getElementById('raw-toml-textarea').value = toml;
    RAW_TOML_DIRTY = false;
    if (!silent) {{
      showToast(LANG === 'en' ? 'Synced from form.' : '已从表单同步最新配置。', 'info');
    }}
  }} catch (e) {{
    showToast(e.message, 'error');
  }}
}}

async function syncToForm() {{
  const toml = document.getElementById('raw-toml-textarea').value;
  const parsed = await parseTomlText(toml);
  if (!parsed) return null;
  mergeConfig(parsed);
  RAW_TOML_DIRTY = false;
  showToast(LANG === 'en' ? 'TOML parsed and synced to form.' : 'TOML 已成功解析并同步到表单中。', 'success');
  return parsed;
}}

async function parseTomlText(toml) {{
  try {{
    const r = await fetch('/api/toml-parse', {{
      method: 'POST',
      headers: {{ 'Content-Type': 'text/plain' }},
      body: toml
    }});
    if (!r.ok) {{
      const err = await r.text();
      setStatus(err, true);
      return null;
    }}
    return await r.json();
  }} catch (e) {{
    setStatus(e.message, true);
    return null;
  }}
}}

async function configFromActiveEditor() {{
  const active = document.querySelector('.tab.active');
  if ((active && active.dataset.tab === 'toml-editor') || RAW_TOML_DIRTY) {{
    const parsed = await parseTomlText(document.getElementById('raw-toml-textarea').value);
    if (!parsed) return null;
    parsed.agents = (parsed.agents || []).map(ensureAgentShape);
    CONFIG = parsed;
    RAW_TOML_DIRTY = false;
    return parsed;
  }}
  return collectConfig();
}}

function renderAll() {{
  applyI18n();
  applyTheme();
}}

// ---------------- agents data helpers ----------------

function ensureAgentShape(a) {{
  return Object.assign({{
    name: '',
    aliases: [],
    cli_type: '',
    command: null,
    path: null,
    model: null,
    effort: null,
    default_timeout: null,
    config_dir: null,
    env_mode: 'inherit',
    env_allowlist: [],
    env: {{}},
    args_strategy: 'builtin',
    args: [],
    extra_args: [],
    prompt_via: null,
    resume_args: [],
    interactive_args: [],
    non_interactive_args: [],
  }}, a || {{}});
}}

CONFIG.agents = (CONFIG.agents || []).map(ensureAgentShape);

// ---------------- left agent list ----------------

function refreshAgentList() {{
  const list = document.getElementById('agent-list');
  list.innerHTML = '';
  if (!CONFIG.agents.length) {{
    const empty = document.createElement('div');
    empty.className = 'agent-empty';
    empty.textContent = LANG === 'en' ? 'No agents yet. Click "+ New agent" to add one.' : '还没有 agent，点击"+ 新建 agent"添加一个。';
    list.appendChild(empty);
    return;
  }}
  CONFIG.agents.forEach((agent, idx) => {{
    const item = document.createElement('div');
    item.className = 'agent-item' + (idx === SELECTED_AGENT_INDEX ? ' active' : '');
    const nameEl = document.createElement('div');
    nameEl.className = 'name';
    nameEl.textContent = agent.name || t('agents.untitled');
    const cliEl = document.createElement('div');
    cliEl.className = 'cli';
    const def = CLI_DEFS_BY_ID[agent.cli_type];
    cliEl.textContent = def ? def.label : (agent.cli_type || t('agent.no_cli'));
    item.appendChild(nameEl);
    item.appendChild(cliEl);
    item.addEventListener('click', () => {{ SELECTED_AGENT_INDEX = idx; refreshAgentList(); renderAgentPane(); }});
    list.appendChild(item);
  }});
}}

function updateAgentCount() {{
  document.getElementById('agent-count').textContent = t('msg.agent_count', CONFIG.agents.length);
}}

// ---------------- agent edit pane ----------------

function renderAgentPane() {{
  const pane = document.getElementById('agent-pane');
  pane.innerHTML = '';
  if (SELECTED_AGENT_INDEX == null || !CONFIG.agents[SELECTED_AGENT_INDEX]) {{
    const empty = document.createElement('div');
    empty.className = 'agent-pane-empty';
    const msg = document.createElement('div');
    msg.textContent = t('agents.empty');
    const btn = document.createElement('button');
    btn.className = 'small';
    btn.textContent = t('action.add_agent');
    btn.addEventListener('click', addNewAgent);
    empty.appendChild(msg);
    empty.appendChild(btn);
    pane.appendChild(empty);
    return;
  }}
  const agent = CONFIG.agents[SELECTED_AGENT_INDEX];
  pane.appendChild(buildAgentEditor(agent));
}}

function buildAgentEditor(agent) {{
  const wrap = document.createElement('div');

  // --- header row ---
  const head = document.createElement('div');
  head.style.cssText = 'display:flex;align-items:center;gap:10px;margin-bottom:14px;flex-wrap:wrap;';
  const nameInput = document.createElement('input');
  nameInput.type = 'text';
  nameInput.value = agent.name || '';
  nameInput.placeholder = t('ph.agent_name');
  nameInput.style.cssText = 'flex:1;min-width:200px;font-weight:600;font-size:15px;';
  nameInput.addEventListener('input', () => {{
    const previousSuggestedDir = suggestedConfigDir(agent);
    agent.name = nameInput.value.trim();
    if (agent.config_dir === previousSuggestedDir) agent.config_dir = suggestedConfigDir(agent);
    refreshAgentList();
    refreshMappingDropdowns();
    refreshDefaultAgentDropdown();
  }});
  head.appendChild(nameInput);

  const dupBtn = document.createElement('button');
  dupBtn.className = 'ghost small';
  dupBtn.textContent = t('action.duplicate');
  dupBtn.addEventListener('click', () => {{
    const copy = JSON.parse(JSON.stringify(agent));
    copy.name = uniqueName((agent.name || 'agent') + '-copy');
    CONFIG.agents.splice(SELECTED_AGENT_INDEX + 1, 0, ensureAgentShape(copy));
    SELECTED_AGENT_INDEX += 1;
    refreshAgentList();
    refreshMappingDropdowns();
    refreshDefaultAgentDropdown();
    renderAgentPane();
    updateAgentCount();
  }});
  head.appendChild(dupBtn);

  const delBtn = document.createElement('button');
  delBtn.className = 'danger small';
  delBtn.textContent = t('action.remove');
  delBtn.addEventListener('click', () => {{
    if (!confirm(t('msg.confirm_remove', agent.name || t('agents.untitled')))) return;
    CONFIG.agents.splice(SELECTED_AGENT_INDEX, 1);
    SELECTED_AGENT_INDEX = CONFIG.agents.length ? Math.min(SELECTED_AGENT_INDEX, CONFIG.agents.length - 1) : null;
    refreshAgentList();
    refreshMappingDropdowns();
    refreshDefaultAgentDropdown();
    renderAgentPane();
    updateAgentCount();
  }});
  head.appendChild(delBtn);
  wrap.appendChild(head);

  // --- section: basics ---
  wrap.appendChild(sectionTitle(t('agent.section.basic')));
  const basic = grid();
  basic.appendChild(field(t('agent.cli_type'), buildCliTypeSelect(agent)));
  basic.appendChild(field(t('agent.aliases'), buildTextarea(agent, 'aliases', {{ asLines: true }}), t('agent.aliases.hint')));
  wrap.appendChild(basic);

  // --- section: command ---
  wrap.appendChild(sectionTitle(t('agent.section.command')));
  const cmd = grid();
  cmd.appendChild(field(t('agent.command'), buildText(agent, 'command', {{ placeholder: t('ph.command') }})));
  cmd.appendChild(field(t('agent.path'), buildText(agent, 'path')));
  wrap.appendChild(cmd);

  // --- section: CLI system isolation ---
  const systemHost = document.createElement('div');
  wrap.appendChild(systemHost);
  buildSystemSection(systemHost, agent);

  // --- section: env ---
  const envHost = document.createElement('div');
  wrap.appendChild(envHost);
  buildEnvSection(envHost, agent);

  // --- section: advanced ---
  const advWrap = document.createElement('details');
  advWrap.style.cssText = 'margin-top:18px;padding:0;border:1px dashed var(--border);border-radius:8px;';
  const advSummary = document.createElement('summary');
  advSummary.style.cssText = 'cursor:pointer;padding:10px 14px;font-weight:600;font-size:13px;color:var(--text);';
  advSummary.textContent = t('agent.section.advanced');
  advWrap.appendChild(advSummary);
  const advBody = document.createElement('div');
  advBody.style.cssText = 'padding:14px;border-top:1px dashed var(--border);';
  const adv = grid();
  adv.appendChild(field(t('agent.model'), buildText(agent, 'model')));
  if (supportsEffort(agent)) {{
    adv.appendChild(field(t('agent.effort'), buildText(agent, 'effort')));
  }} else {{
    agent.effort = null;
  }}
  adv.appendChild(field(t('agent.default_timeout'), buildText(agent, 'default_timeout')));
  adv.appendChild(field(t('agent.args_strategy'), buildSelect(agent, 'args_strategy', [
    {{ value: 'builtin', label: t('agent.args_strategy.builtin') }},
    {{ value: 'append', label: t('agent.args_strategy.append') }},
    {{ value: 'override', label: t('agent.args_strategy.override') }},
  ])));
  adv.appendChild(field(t('agent.prompt_via'), buildSelect(agent, 'prompt_via', [
    {{ value: '', label: t('agent.prompt_via.default') }},
    {{ value: 'stdin', label: t('agent.prompt_via.stdin') }},
    {{ value: 'arg', label: t('agent.prompt_via.arg') }},
    {{ value: 'file', label: t('agent.prompt_via.file') }},
    {{ value: 'file-indirection', label: t('agent.prompt_via.file_indirection') }},
    {{ value: 'arg-or-file-indirection', label: t('agent.prompt_via.arg_or_file_indirection') }},
  ])));
  const fullArgs = document.createElement('div');
  fullArgs.style.gridColumn = '1 / -1';
  fullArgs.appendChild(field(t('agent.args'), buildTextarea(agent, 'args', {{ asLines: true }})));
  adv.appendChild(fullArgs);
  const fullExtra = document.createElement('div');
  fullExtra.style.gridColumn = '1 / -1';
  fullExtra.appendChild(field(t('agent.extra_args'), buildTextarea(agent, 'extra_args', {{ asLines: true }})));
  adv.appendChild(fullExtra);
  adv.appendChild(field(t('agent.interactive_args'), buildTextarea(agent, 'interactive_args', {{ asLines: true }})));
  adv.appendChild(field(t('agent.non_interactive_args'), buildTextarea(agent, 'non_interactive_args', {{ asLines: true }})));
  const fullResume = document.createElement('div');
  fullResume.style.gridColumn = '1 / -1';
  fullResume.appendChild(field(t('agent.resume_args'), buildTextarea(agent, 'resume_args', {{ asLines: true }})));
  adv.appendChild(fullResume);
  advBody.appendChild(adv);
  advWrap.appendChild(advBody);
  wrap.appendChild(advWrap);

  return wrap;
}}

function sectionTitle(text) {{
  const el = document.createElement('div');
  el.className = 'agent-section-title';
  el.textContent = text;
  return el;
}}
function grid() {{
  const el = document.createElement('div');
  el.className = 'grid';
  return el;
}}
function field(labelText, control, hintText) {{
  const w = document.createElement('div');
  const lab = document.createElement('label');
  lab.textContent = labelText;
  w.appendChild(lab);
  w.appendChild(control);
  if (hintText) {{
    const hint = document.createElement('div');
    hint.className = 'field-hint';
    hint.textContent = hintText;
    w.appendChild(hint);
  }}
  return w;
}}

function buildText(agent, key, opts) {{
  const input = document.createElement('input');
  input.type = 'text';
  input.value = agent[key] == null ? '' : agent[key];
  if (opts && opts.placeholder) input.placeholder = opts.placeholder;
  input.addEventListener('input', () => {{ agent[key] = input.value || null; }});
  return input;
}}
function buildTextarea(agent, key, opts) {{
  const ta = document.createElement('textarea');
  if (opts && opts.asLines) ta.value = joinLines(agent[key] || []);
  else ta.value = agent[key] == null ? '' : agent[key];
  ta.addEventListener('input', () => {{
    if (opts && opts.asLines) agent[key] = lines(ta.value);
    else agent[key] = ta.value || null;
  }});
  return ta;
}}
function buildSelect(agent, key, options) {{
  const sel = document.createElement('select');
  for (const opt of options) {{
    const o = document.createElement('option');
    o.value = opt.value;
    o.textContent = opt.label;
    sel.appendChild(o);
  }}
  sel.value = agent[key] == null ? '' : agent[key];
  sel.addEventListener('change', () => {{
    agent[key] = sel.value || (key === 'args_strategy' ? 'builtin' : null);
  }});
  return sel;
}}
function buildCliTypeSelect(agent) {{
  const sel = document.createElement('select');
  const blank = document.createElement('option');
  blank.value = '';
  blank.textContent = t('ph.cli_select');
  sel.appendChild(blank);
  for (const def of CLI_DEFS) {{
    const o = document.createElement('option');
    o.value = def.id;
    o.textContent = def.label + ' (' + def.id + ')';
    sel.appendChild(o);
  }}
  sel.value = agent.cli_type || '';
  sel.addEventListener('change', () => {{
    agent.cli_type = sel.value;
    if (!supportsEffort(agent)) agent.effort = null;
    refreshAgentList();
    refreshMappingDropdowns();
    // re-render env section since known keys depend on cli_type
    renderAgentPane();
  }});
  return sel;
}}

function supportsEffort(agent) {{
  const def = CLI_DEFS_BY_ID[agent.cli_type];
  return !!(def && def.supports_effort);
}}

const DEFAULT_ENV_ALLOWLIST = [
  'HTTP_PROXY',
  'HTTPS_PROXY',
  'ALL_PROXY',
  'NO_PROXY',
  'http_proxy',
  'https_proxy',
  'all_proxy',
  'no_proxy',
];

function buildSystemSection(host, agent) {{
  host.innerHTML = '';
  host.appendChild(sectionTitle(t('agent.section.system')));
  const def = CLI_DEFS_BY_ID[agent.cli_type];
  const mode = agent.config_dir ? 'isolated' : 'default';
  const g = grid();

  g.appendChild(field(t('agent.system_mode'), buildSystemModeSelect(agent)));

  const dirWrap = document.createElement('div');
  dirWrap.appendChild(field(t('agent.config_dir'), buildText(agent, 'config_dir', {{ placeholder: suggestedConfigDir(agent) }})));
  const dirActions = document.createElement('div');
  dirActions.style.cssText = 'display:flex;gap:8px;align-items:center;margin-top:8px;flex-wrap:wrap;';
  const suggested = document.createElement('code');
  suggested.textContent = suggestedConfigDir(agent);
  const useSuggested = document.createElement('button');
  useSuggested.type = 'button';
  useSuggested.className = 'ghost small';
  useSuggested.textContent = t('action.use_suggested_dir');
  useSuggested.addEventListener('click', () => {{
    agent.config_dir = suggestedConfigDir(agent);
    agent.env_mode = 'strict';
    mergeEnvAllowlist(agent, strictModeAllowlist());
    renderAgentPane();
  }});
  dirActions.appendChild(suggested);
  dirActions.appendChild(useSuggested);
  dirWrap.appendChild(dirActions);
  g.appendChild(dirWrap);

  g.appendChild(field(t('agent.env_mode'), buildSelect(agent, 'env_mode', [
    {{ value: 'inherit', label: t('agent.env_mode.inherit') }},
    {{ value: 'strict', label: t('agent.env_mode.strict') }},
  ])));
  g.appendChild(field(t('agent.env_allowlist'), buildTextarea(agent, 'env_allowlist', {{ asLines: true }})));
  host.appendChild(g);

  const note = document.createElement('div');
  note.className = 'mode-note';
  const envName = def && def.config_env ? def.config_env : '-';
  note.innerHTML = escape(mode === 'isolated' ? t('agent.system_mode.isolated_note') : t('agent.system_mode.default_note')) +
    '<br>' + escape(t('agent.system_env')) + ': <code>' + escape(envName) + '</code>';
  host.appendChild(note);
}}

function buildSystemModeSelect(agent) {{
  const sel = document.createElement('select');
  for (const opt of [
    {{ value: 'default', label: t('agent.system_mode.default') }},
    {{ value: 'isolated', label: t('agent.system_mode.isolated') }},
  ]) {{
    const o = document.createElement('option');
    o.value = opt.value;
    o.textContent = opt.label;
    sel.appendChild(o);
  }}
  sel.value = agent.config_dir ? 'isolated' : 'default';
  sel.addEventListener('change', () => {{
    if (sel.value === 'isolated') {{
      if (!agent.config_dir) agent.config_dir = suggestedConfigDir(agent);
      agent.env_mode = 'strict';
      mergeEnvAllowlist(agent, strictModeAllowlist());
    }} else {{
      agent.config_dir = null;
      agent.env_mode = 'inherit';
    }}
    renderAgentPane();
  }});
  return sel;
}}

function suggestedConfigDir(agent) {{
  const segment = safePathSegment(agent.name || 'new-agent');
  const base = configDirectoryPath();
  return base ? base + '/agents/' + segment + '/system' : 'agents/' + segment + '/system';
}}

function configDirectoryPath() {{
  const path = String(SAVE_PATH || '').replace(/\\/g, '/');
  const idx = path.lastIndexOf('/');
  if (idx <= 0) return '';
  return path.slice(0, idx);
}}

function safePathSegment(value) {{
  const text = String(value || 'agent').trim().toLowerCase();
  const cleaned = text.replace(/[^a-z0-9._-]+/g, '-').replace(/^-+|-+$/g, '');
  return (!cleaned || cleaned === '.' || cleaned === '..') ? 'agent' : cleaned;
}}

function strictModeAllowlist() {{
  const keys = CONFIG.proxy && Array.isArray(CONFIG.proxy.env_keys) && CONFIG.proxy.env_keys.length
    ? CONFIG.proxy.env_keys
    : DEFAULT_ENV_ALLOWLIST;
  return keys.filter(Boolean);
}}

function mergeEnvAllowlist(agent, keys) {{
  const seen = new Set();
  const next = [];
  for (const key of (agent.env_allowlist || []).concat(keys || [])) {{
    const value = String(key || '').trim();
    if (!value || seen.has(value)) continue;
    seen.add(value);
    next.push(value);
  }}
  agent.env_allowlist = next;
}}

function buildEnvSection(host, agent) {{
  host.innerHTML = '';
  host.appendChild(sectionTitle(t('agent.section.env')));
  const def = CLI_DEFS_BY_ID[agent.cli_type];
  const detected = (DETECTED && agent.cli_type) ? (DETECTED[agent.cli_type] || null) : null;
  if (detected && detected.source_path) {{
    const note = document.createElement('p');
    note.className = 'muted';
    note.style.cssText = 'font-size:12px;margin:0 0 12px;';
    note.innerHTML = t('agent.env.detected_from') + ' <code>' + escape(detected.source_path) + '</code>';
    host.appendChild(note);
  }}
  if (!def) {{
    const hint = document.createElement('p');
    hint.className = 'muted';
    hint.style.cssText = 'font-size:12px;margin-bottom:10px;';
    hint.textContent = t('agent.no_cli');
    host.appendChild(hint);
  }} else {{
    const g = grid();
    for (const meta of def.env) {{
      const labelText = LANG === 'en' ? (meta.label_en || meta.key) : (meta.label_zh || meta.key);
      const desc = LANG === 'en' ? meta.desc_en : meta.desc_zh;
      const wrapper = document.createElement('div');
      const lab = document.createElement('label');
      lab.textContent = labelText + '  (' + meta.key + ')';
      wrapper.appendChild(lab);
      const ctrl = buildEnvField(agent, meta);
      wrapper.appendChild(ctrl);
      if (desc) {{
        const hint = document.createElement('div');
        hint.className = 'field-hint';
        hint.textContent = desc;
        wrapper.appendChild(hint);
      }}
      // detected default hint
      const detectedValue = detected && detected.env ? detected.env[meta.key] : null;
      if (detectedValue) {{
        const detectedHint = document.createElement('div');
        detectedHint.className = 'field-hint';
        detectedHint.style.cssText = 'display:flex;align-items:center;gap:8px;margin-top:4px;color:var(--text-muted);';
        const label = document.createElement('span');
        label.textContent = t('agent.env.detected_label') + ': ';
        const code = document.createElement('code');
        code.textContent = meta.secret ? maskSecret(detectedValue) : detectedValue;
        const useBtn = document.createElement('button');
        useBtn.type = 'button';
        useBtn.className = 'ghost small';
        useBtn.textContent = t('action.use_detected');
        useBtn.addEventListener('click', () => {{
          if (!agent.env) agent.env = {{}};
          agent.env[meta.key] = detectedValue;
          renderAgentPane();
        }});
        detectedHint.appendChild(label);
        detectedHint.appendChild(code);
        detectedHint.appendChild(useBtn);
        wrapper.appendChild(detectedHint);
      }}
      g.appendChild(wrapper);
    }}
    host.appendChild(g);
    if (detected && detected.env && Object.keys(detected.env).length > 0) {{
      const useAll = document.createElement('button');
      useAll.type = 'button';
      useAll.className = 'secondary small';
      useAll.style.marginTop = '10px';
      useAll.textContent = t('action.use_detected_all');
      useAll.addEventListener('click', () => {{
        if (!agent.env) agent.env = {{}};
        for (const meta of def.env) {{
          const v = detected.env[meta.key];
          if (v) agent.env[meta.key] = v;
        }}
        renderAgentPane();
      }});
      host.appendChild(useAll);
    }}
  }}

  // Other env (KEY=VALUE per line) — excludes the structured ones
  host.appendChild(sectionTitle(t('agent.section.env_extra')));
  const knownKeys = new Set(def ? def.env.map(m => m.key) : []);
  const extraText = Object.entries(agent.env || {{}})
    .filter(([k]) => !knownKeys.has(k))
    .map(([k, v]) => k + '=' + v)
    .join('\n');
  const ta = document.createElement('textarea');
  ta.value = extraText;
  ta.style.minHeight = '90px';
  ta.placeholder = 'PATH=/usr/bin\nDEBUG=1';
  ta.addEventListener('input', () => {{
    const known = new Set(def ? def.env.map(m => m.key) : []);
    const next = {{}};
    // keep structured values
    for (const [k, v] of Object.entries(agent.env || {{}})) {{
      if (known.has(k)) next[k] = v;
    }}
    for (const line of lines(ta.value)) {{
      const idx = line.indexOf('=');
      if (idx > 0) next[line.slice(0, idx).trim()] = line.slice(idx + 1).trim();
    }}
    agent.env = next;
  }});
  host.appendChild(ta);
  const hint = document.createElement('div');
  hint.className = 'field-hint';
  hint.textContent = t('agent.env_extra');
  host.appendChild(hint);
}}

function maskSecret(value) {{
  const s = String(value || '');
  if (s.length <= 8) return '••••';
  return s.slice(0, 4) + '••••' + s.slice(-4);
}}

function buildEnvField(agent, meta) {{
  const row = document.createElement('div');
  row.className = 'secret-row';
  const input = document.createElement('input');
  input.type = meta.secret ? 'password' : 'text';
  input.value = (agent.env && agent.env[meta.key]) || '';
  input.addEventListener('input', () => {{
    if (!agent.env) agent.env = {{}};
    if (input.value === '') delete agent.env[meta.key];
    else agent.env[meta.key] = input.value;
  }});
  row.appendChild(input);
  if (meta.secret) {{
    const toggle = document.createElement('button');
    toggle.type = 'button';
    toggle.className = 'secondary small';
    toggle.textContent = t('action.show');
    toggle.addEventListener('click', () => {{
      if (input.type === 'password') {{ input.type = 'text'; toggle.textContent = t('action.hide'); }}
      else {{ input.type = 'password'; toggle.textContent = t('action.show'); }}
    }});
    row.appendChild(toggle);
  }}
  return row;
}}

function uniqueName(base) {{
  const taken = new Set(CONFIG.agents.map(a => a.name));
  if (!taken.has(base)) return base;
  for (let i = 2; i < 9999; i++) {{
    const candidate = base + '-' + i;
    if (!taken.has(candidate)) return candidate;
  }}
  return base + '-' + Date.now();
}}

function addNewAgent() {{
  const name = uniqueName('new-agent');
  const fresh = ensureAgentShape({{
    name,
    cli_type: '',
    env_mode: 'strict',
    env_allowlist: strictModeAllowlist(),
  }});
  fresh.config_dir = suggestedConfigDir(fresh);
  CONFIG.agents.push(fresh);
  SELECTED_AGENT_INDEX = CONFIG.agents.length - 1;
  refreshAgentList();
  refreshMappingDropdowns();
  refreshDefaultAgentDropdown();
  renderAgentPane();
  updateAgentCount();
  document.querySelector('.tab[data-tab="agents"]').click();
  setTimeout(() => {{
    const input = document.querySelector('#agent-pane input[type=text]');
    if (input) input.focus();
  }}, 0);
}}

document.getElementById('add-agent').addEventListener('click', addNewAgent);
document.getElementById('add-agent-empty').addEventListener('click', addNewAgent);

// ---------------- mapping (4 fixed rows, one per CLI) ----------------

function refreshMappingDropdowns() {{
  renderCliDefaults();
  renderCliAliases();
}}

function renderCliDefaults() {{
  const list = document.getElementById('cli-defaults-list');
  const existing = {{}};
  // capture current values before re-render so we keep edits
  list.querySelectorAll('[data-row-cli]').forEach(row => {{
    existing[row.dataset.rowCli] = row.querySelector('[data-agent-select]').value;
  }});
  list.innerHTML = '';
  for (const def of CLI_DEFS) {{
    const cur = existing[def.id] != null ? existing[def.id] : (CONFIG.cli_type_defaults || {{}})[def.id] || '';
    const row = document.createElement('div');
    row.className = 'mapping-row';
    row.dataset.rowCli = def.id;

    const label = document.createElement('div');
    label.className = 'mapping-label';
    const labelStrong = document.createElement('strong');
    labelStrong.textContent = def.label;
    const labelMuted = document.createElement('span');
    labelMuted.className = 'muted';
    labelMuted.textContent = ' (' + def.id + ')';
    label.appendChild(labelStrong);
    label.appendChild(labelMuted);

    const sel = document.createElement('select');
    sel.dataset.agentSelect = '1';
    const blank = document.createElement('option');
    blank.value = '';
    blank.textContent = t('mapping.no_default');
    sel.appendChild(blank);
    const matching = CONFIG.agents.filter(a => a.name && a.cli_type === def.id);
    if (matching.length === 0) {{
      const o = document.createElement('option');
      o.value = '';
      o.disabled = true;
      o.textContent = t('msg.no_agents_in_cli');
      sel.appendChild(o);
    }} else {{
      for (const a of matching) {{
        const o = document.createElement('option');
        o.value = a.name;
        o.textContent = a.name;
        sel.appendChild(o);
      }}
    }}
    sel.value = cur;

    sel.addEventListener('change', () => {{
      if (!CONFIG.cli_type_defaults) CONFIG.cli_type_defaults = {{}};
      const v = sel.value.trim();
      if (v) CONFIG.cli_type_defaults[def.id] = v;
      else delete CONFIG.cli_type_defaults[def.id];
    }});

    row.appendChild(label);
    row.appendChild(sel);
    list.appendChild(row);
  }}
}}

function renderCliAliases() {{
  const list = document.getElementById('cli-aliases-list');
  // current values
  const existing = {{}};
  list.querySelectorAll('[data-row-cli]').forEach(row => {{
    existing[row.dataset.rowCli] = row.querySelector('[data-alias-input]').value;
  }});
  list.innerHTML = '';
  // invert config map: alias -> cli  =>  cli -> aliases[]; identity aliases are redundant.
  const cliToAliases = {{}};
  for (const [alias, cli] of Object.entries(CONFIG.cli_type_aliases || {{}})) {{
    if (!cli) continue;
    if (alias === cli) continue;
    if (!cliToAliases[cli]) cliToAliases[cli] = [];
    cliToAliases[cli].push(alias);
  }}
  for (const def of CLI_DEFS) {{
    const cur = existing[def.id] != null ? existing[def.id] : ((cliToAliases[def.id] || []).join('\n'));
    const row = document.createElement('div');
    row.className = 'mapping-row';
    row.dataset.rowCli = def.id;

    const label = document.createElement('div');
    label.className = 'mapping-label';
    const labelStrong = document.createElement('strong');
    labelStrong.textContent = def.label;
    const labelMuted = document.createElement('span');
    labelMuted.className = 'muted';
    labelMuted.textContent = ' (' + def.id + ')';
    const labelHint = document.createElement('div');
    labelHint.className = 'field-hint';
    labelHint.textContent = t('mapping.aliases.row_hint');
    label.appendChild(labelStrong);
    label.appendChild(labelMuted);
    label.appendChild(labelHint);

    const input = document.createElement('textarea');
    input.value = cur;
    input.placeholder = t('ph.cli_alias');
    input.dataset.aliasInput = '1';
    input.rows = Math.max(2, lines(cur).length || 2);

    input.addEventListener('input', () => {{
      if (!CONFIG.cli_type_aliases) CONFIG.cli_type_aliases = {{}};
      // Remove any old alias for this cli_type
      for (const [k2, v2] of Object.entries(CONFIG.cli_type_aliases)) {{
        if (v2 === def.id) delete CONFIG.cli_type_aliases[k2];
      }}
      for (const alias of lines(input.value)) {{
        if (alias === def.id) continue;
        CONFIG.cli_type_aliases[alias] = def.id;
      }}
    }});

    row.appendChild(label);
    row.appendChild(input);
    list.appendChild(row);
  }}
}}

function readCliDefaults() {{
  const out = {{}};
  document.querySelectorAll('#cli-defaults-list [data-row-cli]').forEach(row => {{
    const cli = row.dataset.rowCli;
    const agent = row.querySelector('[data-agent-select]').value.trim();
    if (cli && agent) out[cli] = agent;
  }});
  return out;
}}

function readCliAliases() {{
  const out = {{}};
  document.querySelectorAll('#cli-aliases-list [data-row-cli]').forEach(row => {{
    const cli = row.dataset.rowCli;
    for (const alias of lines(row.querySelector('[data-alias-input]').value)) {{
      if (cli && alias && alias !== cli) out[alias] = cli;
    }}
  }});
  return out;
}}


// ---------------- default_agent dropdown ----------------

function refreshDefaultAgentDropdown() {{
  const sel = document.getElementById('default-agent');
  const cur = sel.value || CONFIG.default_agent || '';
  sel.innerHTML = '';
  const blank = document.createElement('option');
  blank.value = '';
  blank.textContent = '—';
  sel.appendChild(blank);
  for (const a of CONFIG.agents) {{
    if (!a.name) continue;
    const o = document.createElement('option');
    o.value = a.name;
    const def = CLI_DEFS_BY_ID[a.cli_type];
    o.textContent = a.name + (def ? ' — ' + def.label : '');
    sel.appendChild(o);
  }}
  sel.value = cur;
}}

// ---------------- top-level render ----------------

function renderAll() {{
  document.getElementById('version').value = CONFIG.version != null ? CONFIG.version : 1;
  document.getElementById('doc-root').value = CONFIG.doc_root || '';
  const proxy = CONFIG.proxy || {{ enabled: true, env_keys: [] }};
  document.getElementById('proxy-enabled').checked = !!proxy.enabled;
  document.getElementById('proxy-env-keys').value = joinLines(proxy.env_keys || []);
  document.getElementById('default-timeout').value = (CONFIG.timeouts && CONFIG.timeouts.default_run) || '';
  refreshDefaultAgentDropdown();
  document.getElementById('default-agent').value = CONFIG.default_agent || '';

  renderCliDefaults();
  renderCliAliases();

  if (SELECTED_AGENT_INDEX == null && CONFIG.agents.length) SELECTED_AGENT_INDEX = 0;
  refreshAgentList();
  renderAgentPane();
  updateAgentCount();
  renderMetadata();
}}

function renderMetadata() {{
  const target = document.getElementById('metadata-block');
  if (!META) {{ target.textContent = t('msg.no_meta'); return; }}
  const loaded = (META.loaded_paths || []).join('\n') || t('msg.none');
  const search = (META.search_paths || []).join('\n') || t('msg.none');
  target.innerHTML =
    '<p>' + t('msg.context.cwd') + ': <code>' + escape(META.cwd) + '</code></p>' +
    '<p>' + t('msg.context.doc_root') + ': <code>' + escape(META.doc_root) + '</code></p>' +
    '<p>' + t('msg.context.recommended') + ': <code>' + escape(META.recommended_path) + '</code></p>' +
    '<p>' + t('msg.context.loaded') + ':</p><pre>' + escape(loaded) + '</pre>' +
    '<p>' + t('msg.context.search') + ':</p><pre>' + escape(search) + '</pre>';
}}

function collectConfig() {{
  return {{
    version: parseInt(document.getElementById('version').value, 10) || 1,
    default_agent: document.getElementById('default-agent').value.trim() || null,
    doc_root: document.getElementById('doc-root').value.trim() || null,
    proxy: {{
      enabled: document.getElementById('proxy-enabled').checked,
      env_keys: lines(document.getElementById('proxy-env-keys').value),
    }},
    timeouts: {{ default_run: document.getElementById('default-timeout').value.trim() || null }},
    cli_type_defaults: readCliDefaults(),
    cli_type_aliases: readCliAliases(),
    agents: CONFIG.agents.map(a => ({{
      name: a.name,
      aliases: a.aliases || [],
      cli_type: a.cli_type,
      command: a.command || null,
      path: a.path || null,
      model: a.model || null,
      effort: supportsEffort(a) ? (a.effort || null) : null,
      default_timeout: a.default_timeout || null,
      config_dir: a.config_dir || null,
      env_mode: a.env_mode || 'inherit',
      env_allowlist: a.env_allowlist || [],
      env: a.env || {{}},
      args_strategy: a.args_strategy || 'builtin',
      args: a.args || [],
      extra_args: a.extra_args || [],
      prompt_via: a.prompt_via || null,
      resume_args: a.resume_args || [],
      interactive_args: a.interactive_args || [],
      non_interactive_args: a.non_interactive_args || [],
    }})),
  }};
}}

function validateBeforeSave(cfg) {{
  const seen = new Set();
  for (const agent of cfg.agents) {{
    if (!agent.name) {{ setStatus(t('msg.need_name'), true); return false; }}
    if (seen.has(agent.name)) {{ setStatus(t('msg.duplicate_name', agent.name), true); return false; }}
    seen.add(agent.name);
    if (!agent.cli_type) {{ setStatus(t('msg.need_cli', agent.name), true); return false; }}
  }}
  return true;
}}

document.getElementById('save-btn').addEventListener('click', async () => {{
  const cfg = await configFromActiveEditor();
  if (!cfg) return;
  if (!validateBeforeSave(cfg)) return;
  try {{
    const r = await fetch('/api/config', {{ method: 'POST', headers: {{ 'Content-Type': 'application/json' }}, body: JSON.stringify(cfg) }});
    const text = await r.text();
    if (!r.ok) {{ setStatus(text, true); return; }}
    setStatus(text);
    // Re-read config from the freshly written file so in-memory CONFIG matches disk exactly.
    await doReload();
  }} catch (e) {{ setStatus(t('msg.save_failed') + e.message, true); }}
}});

document.getElementById('reload-btn').addEventListener('click', async () => await doReload());

async function doReload() {{
  try {{
    const r = await fetch('/api/config');
    if (!r.ok) {{ setStatus(t('msg.reload_failed'), true); return; }}
    const fresh = await r.json();
    mergeConfig(fresh);
    setStatus(t('msg.reloaded'));
  }} catch (e) {{ setStatus(t('msg.reload_failed_e') + e.message, true); }}
}}

function mergeConfig(fresh) {{
  CONFIG = fresh;
  CONFIG.agents = (CONFIG.agents || []).map(ensureAgentShape);
  RAW_TOML_DIRTY = false;
  // Keep the same agent selected if its name still exists
  if (SELECTED_AGENT_INDEX != null && SELECTED_AGENT_INDEX >= 0 && SELECTED_AGENT_INDEX < CONFIG.agents.length) {{
    // fine, keep it
  }} else if (CONFIG.agents.length) {{
    SELECTED_AGENT_INDEX = 0;
  }} else {{
    SELECTED_AGENT_INDEX = null;
  }}
  renderAll();
}}

document.getElementById('stop-btn').addEventListener('click', async () => {{
  if (!confirm(t('msg.stop_confirm'))) return;
  try {{ await fetch('/api/shutdown', {{ method: 'POST' }}); }} catch (e) {{}}
  setStatus(t('msg.stopped'));
}});

document.getElementById('preview-btn').addEventListener('click', () => {{
  document.getElementById('preview-text').textContent = JSON.stringify(collectConfig(), null, 2);
  document.getElementById('preview-dialog').showModal();
}});

document.getElementById('default-agent').addEventListener('change', e => {{
  CONFIG.default_agent = e.target.value || null;
}});

document.getElementById('version').addEventListener('input', e => {{
  CONFIG.version = parseInt(e.target.value, 10) || 1;
}});
document.getElementById('doc-root').addEventListener('input', e => {{
  CONFIG.doc_root = e.target.value || null;
}});
document.getElementById('default-timeout').addEventListener('input', e => {{
  if (!CONFIG.timeouts) CONFIG.timeouts = {{}};
  CONFIG.timeouts.default_run = e.target.value || null;
}});
document.getElementById('proxy-enabled').addEventListener('change', e => {{
  if (!CONFIG.proxy) CONFIG.proxy = {{ enabled: true, env_keys: [] }};
  CONFIG.proxy.enabled = e.target.checked;
}});
document.getElementById('proxy-env-keys').addEventListener('input', e => {{
  if (!CONFIG.proxy) CONFIG.proxy = {{ enabled: true, env_keys: [] }};
  CONFIG.proxy.env_keys = lines(e.target.value);
}});

document.getElementById('sync-to-form-btn').addEventListener('click', syncToForm);
document.getElementById('sync-from-form-btn').addEventListener('click', () => syncFromForm(false));

applyTheme();
renderAll();
applyI18n();
</script>
</body>
</html>
"#,
        save_path = save_path_text,
        save_path_json = save_path_json,
        metadata_json = metadata_json,
        detected_json = detected_json,
        initial_json = initial_json,
    )
}
