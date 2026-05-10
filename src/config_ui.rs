use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;

use crate::config::ConfigFile;
use crate::error::{OccError, OccResult};

pub fn write_html(path: &Path, initial_toml: &str) -> OccResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            OccError::io(
                "doc_root_not_writable",
                format!("Failed to create '{}'", parent.display()),
                error,
            )
        })?;
    }
    fs::write(path, html(initial_toml)).map_err(|error| {
        OccError::io(
            "doc_root_not_writable",
            format!("Failed to write '{}'", path.display()),
            error,
        )
    })
}

pub fn serve(initial_toml: &str, save_path: &Path) -> OccResult<()> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|error| {
        OccError::io(
            "child_process_failed",
            "Failed to start config UI server on 127.0.0.1",
            error,
        )
    })?;
    let address = listener.local_addr().map_err(|error| {
        OccError::io(
            "child_process_failed",
            "Failed to read config UI server address",
            error,
        )
    })?;
    let url = format!("http://{}/", address);
    println!("ui: {}", url);
    println!("config: {}", save_path.display());
    let _ = open::that(&url);
    for stream in listener.incoming() {
        let stream = stream.map_err(|error| {
            OccError::io(
                "child_process_failed",
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

pub fn html(initial_toml: &str) -> String {
    html_with_save_path(initial_toml, None)
}

fn html_with_save_path(initial_toml: &str, save_path: Option<&Path>) -> String {
    let save_button = if save_path.is_some() {
        r#"<button id="save-config">Save to Config File</button>"#
    } else {
        ""
    };
    let close_button = if save_path.is_some() {
        r#"<button id="close-server" class="secondary">Close Server</button>"#
    } else {
        ""
    };
    let save_path_text = save_path
        .map(|path| {
            format!(
                "<p>Saving target: <code>{}</code></p>",
                escape_html(&path.display().to_string())
            )
        })
        .unwrap_or_default();
    let intro_text = if save_path.is_some() {
        "Edit TOML, then save it through the local server, copy it, download it, or save it with browsers that support the File System Access API."
    } else {
        "Edit TOML, then copy it, download it, or save it with browsers that support the File System Access API."
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
<div class="actions">
{}
<button id="copy">Copy TOML</button>
<button id="download" class="secondary">Download TOML</button>
<button id="save" class="secondary">Save As</button>
{}
</div>
<textarea id="toml" spellcheck="false">{}</textarea>
<p id="status" class="status"></p>
</section>
</main>
<script>
const textarea = document.getElementById('toml');
const status = document.getElementById('status');
function setStatus(text) {{ status.textContent = text; setTimeout(() => status.textContent = '', 4000); }}
const saveConfig = document.getElementById('save-config');
if (saveConfig) {{
  saveConfig.addEventListener('click', async () => {{
    const response = await fetch('/config', {{ method: 'POST', headers: {{ 'Content-Type': 'text/plain; charset=utf-8' }}, body: textarea.value }});
    const text = await response.text();
    if (!response.ok) {{ setStatus(text); return; }}
    setStatus(text);
  }});
}}
document.getElementById('copy').addEventListener('click', async () => {{
  await navigator.clipboard.writeText(textarea.value);
  setStatus('Copied TOML to clipboard.');
}});
document.getElementById('download').addEventListener('click', () => {{
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
  const handle = await window.showSaveFilePicker({{ suggestedName: 'config.toml', types: [{{ description: 'TOML', accept: {{ 'application/toml': ['.toml'] }} }}] }});
  const writable = await handle.createWritable();
  await writable.write(textarea.value);
  await writable.close();
  setStatus('Saved TOML.');
}});
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
        save_button,
        close_button,
        escape_html(initial_toml)
    )
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
        let page = html_with_save_path(initial_toml, Some(save_path));
        write_response(&mut stream, "200 OK", "text/html; charset=utf-8", &page)?;
        return Ok(false);
    }
    if first_line.starts_with("POST /config ") {
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
                    format!("Failed to create '{}'", parent.display()),
                    error,
                )
            })?;
        }
        fs::write(save_path, text).map_err(|error| {
            OccError::io(
                "doc_root_not_writable",
                format!("Failed to write '{}'", save_path.display()),
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
            OccError::io(
                "child_process_failed",
                "Failed to read config UI request",
                error,
            )
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
                "config_parse_failed",
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
    stream.write_all(response.as_bytes()).map_err(|error| {
        OccError::io(
            "child_process_failed",
            "Failed to write config UI response",
            error,
        )
    })
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
