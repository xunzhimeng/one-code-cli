use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = env::temp_dir().join(format!("occ-{name}-{nonce}"));
    fs::create_dir_all(&path).unwrap();
    path
}

fn compile_worker(dir: &Path) -> PathBuf {
    let source = dir.join("worker.rs");
    let executable = dir.join(if cfg!(windows) {
        "worker.exe"
    } else {
        "worker"
    });
    fs::write(
        &source,
        r#"
use std::env;
use std::fs;
use std::io::{self, Write};
use std::process::{self, Command};
use std::thread;
use std::time::Duration;

fn main() {
    match env::args().nth(1).as_deref() {
        Some("large") => {
            for index in 0..20_000 {
                println!("stdout-line-{index}-abcdefghijklmnopqrstuvwxyz0123456789");
                eprintln!("stderr-line-{index}-abcdefghijklmnopqrstuvwxyz0123456789");
            }
        }
        Some("small") => {
            println!("small-stdout-line");
            eprintln!("small-stderr-line");
        }
        Some("agent-one") => {
            println!("agent-one-stdout");
            eprintln!("agent-one-stderr");
        }
        Some("agent-two") => {
            println!("agent-two-stdout");
            eprintln!("agent-two-stderr");
        }
        Some("noisy") => {
            eprintln!("\x1b[?25l");
            eprintln!("");
            println!("visible-noisy-stdout");
            eprintln!("visible-noisy-stderr");
        }
        Some("split-ansi") => {
            let filler = "x".repeat(8191);
            eprint!("{filler}\x1b");
            io::stderr().flush().unwrap();
            thread::sleep(Duration::from_millis(20));
            eprintln!("[?25l");
            println!("split-ansi-visible");
        }
        Some("invalid-stderr") => {
            println!("invalid-stdout-line");
            io::stderr().write_all(&[0xff, b'\n']).unwrap();
        }
        Some("env-check") => {
            for key in [
                "OCC_ALLOWED_PARENT",
                "BAD_PARENT_SECRET",
                "ANTHROPIC_BASE_URL",
                "CHILD_ONLY",
                "CLAUDE_CONFIG_DIR",
            ] {
                match env::var(key) {
                    Ok(value) => println!("{key}={value}"),
                    Err(_) => println!("{key}=<missing>"),
                }
            }
            if env::var("OCC_ALLOWED_PARENT").as_deref() != Ok("allowed-value") {
                process::exit(3);
            }
            if env::var_os("BAD_PARENT_SECRET").is_some() {
                process::exit(4);
            }
            if env::var_os("ANTHROPIC_BASE_URL").is_some() {
                process::exit(5);
            }
            if env::var("CHILD_ONLY").as_deref() != Ok("child-value") {
                process::exit(6);
            }
            if env::var_os("CLAUDE_CONFIG_DIR").is_none() {
                process::exit(7);
            }
        }
        Some("timeout") => {
            println!("partial-stdout-before-timeout");
            eprintln!("partial-stderr-before-timeout");
            io::stdout().flush().unwrap();
            io::stderr().flush().unwrap();
            thread::sleep(Duration::from_secs(30));
        }
        Some("spawn-child-timeout") => {
            let marker = env::args().nth(2).unwrap();
            let current = env::current_exe().unwrap();
            Command::new(current)
                .arg("delayed-marker")
                .arg(marker)
                .spawn()
                .unwrap();
            println!("spawned-child-before-timeout");
            io::stdout().flush().unwrap();
            thread::sleep(Duration::from_secs(30));
        }
        Some("delayed-marker") => {
            let marker = env::args().nth(2).unwrap();
            thread::sleep(Duration::from_secs(3));
            fs::write(marker, "child-survived").unwrap();
        }
        _ => {}
    }
}
"#,
    )
    .unwrap();

    let rustc = env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
    let status = Command::new(rustc)
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .status()
        .unwrap();
    assert!(status.success(), "failed to compile mock worker");
    executable
}

fn write_config(dir: &Path, worker: &Path, mode: &str) -> PathBuf {
    let config = dir.join(format!("config-{mode}.toml"));
    let doc_root = dir.join(format!("docs-{mode}"));
    fs::write(
        &config,
        format!(
            r#"version = 1
doc_root = "{}"

[[agents]]
name = "mock"
cli_type = "claude"
path = "{}"
args_strategy = "override"
args = ["{}"]
prompt_via = "stdin"
"#,
            toml_string(&doc_root),
            toml_string(worker),
            mode
        ),
    )
    .unwrap();
    config
}

fn write_session_binding_config(dir: &Path, worker: &Path) -> PathBuf {
    let config = dir.join("config-session-binding.toml");
    let doc_root = dir.join("docs-session-binding");
    fs::write(
        &config,
        format!(
            r#"version = 1
doc_root = "{}"
default_agent = "beta"

[[agents]]
name = "alpha"
cli_type = "claude"
path = "{}"
model = "alpha-model"
args_strategy = "override"
args = ["small"]
prompt_via = "stdin"

[[agents]]
name = "beta"
cli_type = "claude"
path = "{}"
model = "beta-model"
args_strategy = "override"
args = ["small"]
prompt_via = "stdin"
"#,
            toml_string(&doc_root),
            toml_string(worker),
            toml_string(worker),
        ),
    )
    .unwrap();
    config
}

fn write_multi_agent_config(dir: &Path, worker: &Path) -> PathBuf {
    let config = dir.join("config-multi-agent.toml");
    let doc_root = dir.join("docs-multi-agent");
    fs::write(
        &config,
        format!(
            r#"version = 1
doc_root = "{}"

[[agents]]
name = "one"
cli_type = "claude"
path = "{}"
model = "model-one"
env = {{ ANTHROPIC_BASE_URL = "https://one.example.test" }}
args_strategy = "override"
args = ["agent-one"]
prompt_via = "stdin"

[[agents]]
name = "two"
cli_type = "claude"
path = "{}"
model = "model-two"
env = {{ ANTHROPIC_BASE_URL = "https://two.example.test" }}
args_strategy = "override"
args = ["agent-two"]
prompt_via = "stdin"

[[agents]]
name = "noisy"
cli_type = "claude"
path = "{}"
args_strategy = "override"
args = ["noisy"]
prompt_via = "stdin"

[[agents]]
name = "split-ansi"
cli_type = "claude"
path = "{}"
args_strategy = "override"
args = ["split-ansi"]
prompt_via = "stdin"
"#,
            toml_string(&doc_root),
            toml_string(worker),
            toml_string(worker),
            toml_string(worker),
            toml_string(worker),
        ),
    )
    .unwrap();
    config
}

fn write_config_without_doc_root(dir: &Path, worker: &Path, mode: &str) -> PathBuf {
    let config = dir.join(format!("config-no-doc-root-{mode}.toml"));
    fs::write(
        &config,
        format!(
            r#"version = 1

[[agents]]
name = "mock"
cli_type = "claude"
path = "{}"
args_strategy = "builtin"
prompt_via = "stdin"
interactive_args = ["interactive-mode"]
non_interactive_args = ["{}"]
"#,
            toml_string(worker),
            mode
        ),
    )
    .unwrap();
    config
}

fn write_tree_timeout_config(dir: &Path, worker: &Path, marker: &Path) -> PathBuf {
    let config = dir.join("config-tree-timeout.toml");
    let doc_root = dir.join("docs-tree-timeout");
    fs::write(
        &config,
        format!(
            r#"version = 1
doc_root = "{}"

[[agents]]
name = "mock"
cli_type = "claude"
path = "{}"
args_strategy = "override"
args = ["spawn-child-timeout", "{}"]
prompt_via = "stdin"
"#,
            toml_string(&doc_root),
            toml_string(worker),
            toml_string(marker),
        ),
    )
    .unwrap();
    config
}

fn write_timeout_config(
    dir: &Path,
    worker: &Path,
    mode: &str,
    global_timeout: &str,
    profile_timeout: Option<&str>,
) -> PathBuf {
    let config = dir.join("config-timeout.toml");
    let doc_root = dir.join("docs-timeout");
    let profile_timeout = profile_timeout
        .map(|value| format!("default_timeout = \"{}\"\n", value))
        .unwrap_or_default();
    fs::write(
        &config,
        format!(
            r#"version = 1
doc_root = "{}"

[timeouts]
default_run = "{}"

[[agents]]
name = "mock"
cli_type = "claude"
path = "{}"
args_strategy = "override"
args = ["{}"]
prompt_via = "stdin"
{}"#,
            toml_string(&doc_root),
            global_timeout,
            toml_string(worker),
            mode,
            profile_timeout,
        ),
    )
    .unwrap();
    config
}

fn write_gemini_config(dir: &Path) -> PathBuf {
    let config = dir.join("config-gemini.toml");
    let doc_root = dir.join("docs-gemini");
    fs::write(
        &config,
        format!(
            r#"version = 1
doc_root = "{}"

[cli_type_defaults]
gemini = "mock-gemini"

[[agents]]
name = "mock-gemini"
cli_type = "gemini"
command = "gemini"
args_strategy = "builtin"
"#,
            toml_string(&doc_root)
        ),
    )
    .unwrap();
    config
}

fn write_model_config_with_mode(dir: &Path, worker: &Path, mode: &str) -> PathBuf {
    let config = dir.join("config-model.toml");
    let doc_root = dir.join("docs-model");
    fs::write(
        &config,
        format!(
            r#"version = 1
doc_root = "{}"

[[agents]]
name = "mock"
cli_type = "claude"
path = "{}"
model = "profile-model"
args_strategy = "override"
args = ["{}"]
prompt_via = "stdin"
"#,
            toml_string(&doc_root),
            toml_string(worker),
            mode,
        ),
    )
    .unwrap();
    config
}

fn write_model_config(dir: &Path, worker: &Path) -> PathBuf {
    write_model_config_with_mode(dir, worker, "large")
}

fn write_model_effort_override_codex_config(dir: &Path, worker: &Path, mode: &str) -> PathBuf {
    let config = dir.join("config-model-effort-codex.toml");
    let doc_root = dir.join("docs-model-effort-codex");
    fs::write(
        &config,
        format!(
            r#"version = 1
doc_root = "{}"

[[agents]]
name = "mock"
cli_type = "codex"
path = "{}"
model = "profile-model"
effort = "high"
args_strategy = "override"
args = ["{}"]
prompt_via = "stdin"
"#,
            toml_string(&doc_root),
            toml_string(worker),
            mode,
        ),
    )
    .unwrap();
    config
}

fn write_model_effort_builtin_codex_config(dir: &Path) -> PathBuf {
    let config = dir.join("config-model-effort-codex-builtin.toml");
    let doc_root = dir.join("docs-model-effort-codex-builtin");
    fs::write(
        &config,
        format!(
            r#"version = 1
doc_root = "{}"

[[agents]]
name = "mock"
cli_type = "codex"
model = "profile-model"
effort = "high"
args_strategy = "builtin"
"#,
            toml_string(&doc_root),
        ),
    )
    .unwrap();
    config
}

fn write_isolated_cli_config(dir: &Path, worker: &Path) -> PathBuf {
    let config = dir.join("config-isolated-clis.toml");
    let doc_root = dir.join("docs-isolated-clis");
    fs::write(
        &config,
        format!(
            r#"version = 1
doc_root = "{}"

[[agents]]
name = "claude-iso"
cli_type = "claude"
path = "{}"
config_dir = "{}"
args_strategy = "override"
args = ["small"]
prompt_via = "stdin"

[[agents]]
name = "codex-iso"
cli_type = "codex"
path = "{}"
config_dir = "{}"
args_strategy = "override"
args = ["small"]
prompt_via = "stdin"

[[agents]]
name = "opencode-iso"
cli_type = "opencode"
path = "{}"
config_dir = "{}"
args_strategy = "override"
args = ["small"]
prompt_via = "arg"

[[agents]]
name = "gemini-iso"
cli_type = "gemini"
path = "{}"
config_dir = "{}"
args_strategy = "override"
args = ["small"]
prompt_via = "arg"
"#,
            toml_string(&doc_root),
            toml_string(worker),
            toml_string(&dir.join("systems").join("claude")),
            toml_string(worker),
            toml_string(&dir.join("systems").join("codex")),
            toml_string(worker),
            toml_string(&dir.join("systems").join("opencode")),
            toml_string(worker),
            toml_string(&dir.join("systems").join("gemini")),
        ),
    )
    .unwrap();
    config
}

fn write_strict_env_config(dir: &Path, worker: &Path) -> PathBuf {
    let config = dir.join("config-strict-env.toml");
    let doc_root = dir.join("docs-strict-env");
    fs::write(
        &config,
        format!(
            r#"version = 1
doc_root = "{}"

[[agents]]
name = "strict-env"
cli_type = "claude"
path = "{}"
config_dir = "{}"
env_mode = "strict"
env_allowlist = ["OCC_ALLOWED_PARENT"]
env = {{ CHILD_ONLY = "child-value" }}
args_strategy = "override"
args = ["env-check"]
prompt_via = "stdin"
"#,
            toml_string(&doc_root),
            toml_string(worker),
            toml_string(&dir.join("systems").join("strict-claude")),
        ),
    )
    .unwrap();
    config
}

fn run_occ_dry_agent(config: &Path, cwd: &Path, agent: &str) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("--config")
        .arg(config)
        .arg("run")
        .arg("--agent")
        .arg(agent)
        .arg("--cwd")
        .arg(cwd)
        .arg("--prompt")
        .arg("hello")
        .arg("--output")
        .arg("json")
        .arg("--dry-run")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn run_occ_dry_with_env(
    config: &Path,
    cwd: &Path,
    extra: &[&str],
    envs: &[(&str, &Path)],
) -> serde_json::Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_occ"));
    command
        .arg("--config")
        .arg(config)
        .arg("run")
        .arg("--agent")
        .arg("mock")
        .arg("--cwd")
        .arg(cwd)
        .arg("--prompt")
        .arg("hello")
        .arg("--output")
        .arg("json")
        .arg("--dry-run");
    for (key, value) in envs {
        command.env(key, value);
    }
    if let Some((_, home)) = envs.iter().find(|(key, _)| *key == "USERPROFILE") {
        let home_text = home.to_string_lossy().to_string();
        if home_text.len() >= 2 && home_text.as_bytes()[1] == b':' {
            command.env("HOMEDRIVE", &home_text[..2]);
            command.env("HOMEPATH", &home_text[2..]);
        }
        command.env("APPDATA", home.join("AppData").join("Roaming"));
        command.env("LOCALAPPDATA", home.join("AppData").join("Local"));
    }
    for arg in extra {
        command.arg(arg);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn apply_home_env(command: &mut Command, home: &Path) {
    command.env("USERPROFILE", home);
    command.env("HOME", home);
    let home_text = home.to_string_lossy().to_string();
    if home_text.len() >= 2 && home_text.as_bytes()[1] == b':' {
        command.env("HOMEDRIVE", &home_text[..2]);
        command.env("HOMEPATH", &home_text[2..]);
    }
    command.env("APPDATA", home.join("AppData").join("Roaming"));
    command.env("LOCALAPPDATA", home.join("AppData").join("Local"));
}

fn write_alias_config(dir: &Path, worker: &Path) -> PathBuf {
    let config = dir.join("config-alias.toml");
    let doc_root = dir.join("docs-alias");
    fs::write(
        &config,
        format!(
            r#"version = 1
doc_root = "{}"

[[agents]]
name = "mock"
aliases = ["m"]
cli_type = "claude"
path = "{}"
args_strategy = "override"
args = ["quick"]
prompt_via = "stdin"
"#,
            toml_string(&doc_root),
            toml_string(worker),
        ),
    )
    .unwrap();
    config
}

fn write_conflicting_alias_config(dir: &Path, worker: &Path) -> PathBuf {
    let config = dir.join("config-alias-conflict.toml");
    let doc_root = dir.join("docs-alias-conflict");
    fs::write(
        &config,
        format!(
            r#"version = 1
doc_root = "{}"

[[agents]]
name = "one"
aliases = ["dup"]
cli_type = "claude"
path = "{}"

[[agents]]
name = "two"
aliases = ["dup"]
cli_type = "claude"
path = "{}"
"#,
            toml_string(&doc_root),
            toml_string(worker),
            toml_string(worker),
        ),
    )
    .unwrap();
    config
}

fn write_backend_alias_config(dir: &Path, worker: &Path) -> PathBuf {
    let config = dir.join("config-backend-alias.toml");
    let doc_root = dir.join("docs-backend-alias");
    fs::write(
        &config,
        format!(
            r#"version = 1
doc_root = "{}"

[cli_type_aliases]
c = "claude"

[[agents]]
name = "mock"
cli_type = "claude"
path = "{}"
args_strategy = "override"
args = ["quick"]
prompt_via = "stdin"
"#,
            toml_string(&doc_root),
            toml_string(worker),
        ),
    )
    .unwrap();
    config
}

fn write_new_names_config(dir: &Path, worker: &Path) -> PathBuf {
    let config = dir.join("config-new-names.toml");
    let doc_root = dir.join("docs-new-names");
    fs::write(
        &config,
        format!(
            r#"version = 1
doc_root = "{}"
default_agent = "mock"

[cli_type_aliases]
c = "claude"

[[agents]]
name = "mock"
cli_type = "claude"
path = "{}"
args_strategy = "override"
args = ["quick"]
prompt_via = "stdin"
"#,
            toml_string(&doc_root),
            toml_string(worker),
        ),
    )
    .unwrap();
    config
}

fn write_export_config(dir: &Path) -> PathBuf {
    let config = dir.join("config-export.toml");
    let doc_root = dir.join("docs-export");
    fs::write(
        &config,
        format!(
            r#"version = 1
doc_root = "{}"
"#,
            toml_string(&doc_root),
        ),
    )
    .unwrap();
    config
}

fn write_conflicting_backend_alias_config(dir: &Path) -> PathBuf {
    let config = dir.join("config-backend-alias-conflict.toml");
    let doc_root = dir.join("docs-backend-alias-conflict");
    fs::write(
        &config,
        format!(
            r#"version = 1
doc_root = "{}"

[cli_type_aliases]
claude = "codex"
"#,
            toml_string(&doc_root),
        ),
    )
    .unwrap();
    config
}

fn write_identity_backend_alias_config(dir: &Path) -> PathBuf {
    let config = dir.join("config-backend-alias-identity.toml");
    let doc_root = dir.join("docs-backend-alias-identity");
    fs::write(
        &config,
        format!(
            r#"version = 1
doc_root = "{}"

[cli_type_aliases]
codex = "codex"
gemini = "gemini"
"#,
            toml_string(&doc_root),
        ),
    )
    .unwrap();
    config
}

fn write_profile_alias_shadows_backend_config(dir: &Path, worker: &Path) -> PathBuf {
    let config = dir.join("config-profile-alias-shadows-backend.toml");
    let doc_root = dir.join("docs-profile-alias-shadows-backend");
    fs::write(
        &config,
        format!(
            r#"version = 1
doc_root = "{}"

[[agents]]
name = "mock"
aliases = ["claude"]
cli_type = "claude"
path = "{}"
"#,
            toml_string(&doc_root),
            toml_string(worker),
        ),
    )
    .unwrap();
    config
}

fn write_backend_alias_shadows_profile_config(dir: &Path, worker: &Path) -> PathBuf {
    let config = dir.join("config-backend-alias-shadows-profile.toml");
    let doc_root = dir.join("docs-backend-alias-shadows-profile");
    fs::write(
        &config,
        format!(
            r#"version = 1
doc_root = "{}"

[cli_type_aliases]
mock = "claude"

[[agents]]
name = "mock"
cli_type = "claude"
path = "{}"
"#,
            toml_string(&doc_root),
            toml_string(worker),
        ),
    )
    .unwrap();
    config
}

fn toml_string(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

fn run_occ(config: &Path, cwd: &Path, timeout: &str) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("--config")
        .arg(config)
        .arg("run")
        .arg("--agent")
        .arg("mock")
        .arg("--cwd")
        .arg(cwd)
        .arg("--prompt")
        .arg("hello")
        .arg("--output")
        .arg("json")
        .arg("--timeout")
        .arg(timeout)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn run_occ_with_default_timeout(config: &Path, cwd: &Path) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("--config")
        .arg(config)
        .arg("run")
        .arg("--agent")
        .arg("mock")
        .arg("--cwd")
        .arg(cwd)
        .arg("--prompt")
        .arg("hello")
        .arg("--output")
        .arg("json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn run_occ_dry(config: &Path, cwd: &Path, extra: &[&str]) -> serde_json::Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_occ"));
    command
        .arg("--config")
        .arg(config)
        .arg("run")
        .arg("--agent")
        .arg("mock")
        .arg("--cwd")
        .arg(cwd)
        .arg("--prompt")
        .arg("hello")
        .arg("--output")
        .arg("json")
        .arg("--dry-run");
    for arg in extra {
        command.arg(arg);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn run_occ_dry_without_prompt(config: &Path, cwd: &Path) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("--config")
        .arg(config)
        .arg("run")
        .arg("--agent")
        .arg("mock")
        .arg("--cwd")
        .arg(cwd)
        .arg("--output")
        .arg("json")
        .arg("--dry-run")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn run_occ_profile(config: &Path, cwd: &Path, profile: &str) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("--config")
        .arg(config)
        .arg("run")
        .arg("--agent")
        .arg(profile)
        .arg("--cwd")
        .arg(cwd)
        .arg("--prompt")
        .arg("hello")
        .arg("--output")
        .arg("json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn run_occ_backend(config: &Path, cwd: &Path, backend: &str) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("--config")
        .arg(config)
        .arg("run")
        .arg("--cli")
        .arg(backend)
        .arg("--cwd")
        .arg(cwd)
        .arg("--prompt")
        .arg("hello")
        .arg("--output")
        .arg("json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn run_occ_agents(config: &Path, cwd: &Path, agents: &str, stream: bool) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_occ"));
    command
        .arg("--config")
        .arg(config)
        .arg("run")
        .arg("--agents")
        .arg(agents)
        .arg("--cwd")
        .arg(cwd)
        .arg("--prompt")
        .arg("hello")
        .arg("--output")
        .arg("json");
    if stream {
        command.arg("--stream");
    }
    command.output().unwrap()
}

fn run_occ_agents_dry(config: &Path, cwd: &Path, agents: &str) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_occ"));
    command
        .arg("--config")
        .arg(config)
        .arg("run")
        .arg("--agents")
        .arg(agents)
        .arg("--cwd")
        .arg(cwd)
        .arg("--prompt")
        .arg("hello")
        .arg("--output")
        .arg("json")
        .arg("--dry-run");
    command.output().unwrap()
}

fn run_occ_expect_failure(config: &Path, args: &[&str]) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_occ"));
    command.arg("--config").arg(config);
    for arg in args {
        command.arg(arg);
    }
    let output = command.output().unwrap();
    assert!(
        !output.status.success(),
        "occ unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn run_occ_text(config: &Path, args: &[&str]) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_occ"));
    command.arg("--config").arg(config);
    for arg in args {
        command.arg(arg);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn run_occ_text_with_stdin(config: &Path, args: &[&str], stdin: &str) -> String {
    use std::io::Write;
    use std::process::Stdio;

    let mut command = Command::new(env!("CARGO_BIN_EXE_occ"));
    command.arg("--config").arg(config);
    for arg in args {
        command.arg(arg);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn non_interactive_large_output_is_streamed_to_logs() {
    let dir = temp_dir("large-output");
    let worker = compile_worker(&dir);
    let config = write_config(&dir, &worker, "large");

    let response = run_occ(&config, &dir, "10s");
    assert_eq!(response["success"], true);
    let result_path = PathBuf::from(response["result_path"].as_str().unwrap());
    let run_dir = result_path.parent().unwrap();
    let stdout = fs::read_to_string(run_dir.join("stdout.log")).unwrap();
    let stderr = fs::read_to_string(run_dir.join("stderr.log")).unwrap();

    assert!(stdout.contains("stdout-line-19999"));
    assert!(stderr.contains("stderr-line-19999"));
    assert!(stdout.len() > 64 * 1024);
    assert!(stderr.len() > 64 * 1024);
}

#[test]
fn multi_agent_run_executes_each_agent_and_reports_batch_json() {
    let dir = temp_dir("multi-agent-run");
    let worker = compile_worker(&dir);
    let config = write_multi_agent_config(&dir, &worker);

    let output = run_occ_agents(&config, &dir, "one,two", false);
    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["success"], true);
    assert!(response["batch_id"]
        .as_str()
        .is_some_and(|value| value.starts_with("batch_")));
    let runs = response["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0]["agent"], "one");
    assert_eq!(runs[0]["model"], "model-one");
    assert_eq!(runs[1]["agent"], "two");
    assert_eq!(runs[1]["model"], "model-two");

    let first_result = PathBuf::from(runs[0]["result_path"].as_str().unwrap());
    let second_result = PathBuf::from(runs[1]["result_path"].as_str().unwrap());
    assert!(fs::read_to_string(first_result)
        .unwrap()
        .contains("agent-one-stdout"));
    assert!(fs::read_to_string(second_result)
        .unwrap()
        .contains("agent-two-stdout"));
}

#[test]
fn multi_agent_stream_prefixes_live_output_and_filters_noise() {
    let dir = temp_dir("multi-agent-stream");
    let worker = compile_worker(&dir);
    let config = write_multi_agent_config(&dir, &worker);

    let output = run_occ_agents(&config, &dir, "one,noisy", true);
    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("[one] agent-one-stdout"));
    assert!(stderr.contains("[one] agent-one-stderr"));
    assert!(stderr.contains("[noisy] visible-noisy-stdout"));
    assert!(stderr.contains("[noisy] visible-noisy-stderr"));
    assert!(!stderr.contains("[?25l"));
}

#[test]
fn multi_agent_stream_filters_ansi_sequences_split_across_chunks() {
    let dir = temp_dir("multi-agent-split-ansi");
    let worker = compile_worker(&dir);
    let config = write_multi_agent_config(&dir, &worker);

    let output = run_occ_agents(&config, &dir, "split-ansi", true);
    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("[split-ansi] split-ansi-visible"));
    assert!(!stderr.contains("[?25l"));
}

#[test]
fn multi_agent_dry_run_reports_each_agent_plan_without_creating_runs() {
    let dir = temp_dir("multi-agent-dry-run");
    let worker = compile_worker(&dir);
    let config = write_multi_agent_config(&dir, &worker);

    let output = run_occ_agents_dry(&config, &dir, "one,two");
    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["success"], true);
    assert!(response["batch_id"]
        .as_str()
        .is_some_and(|value| value.starts_with("batch_")));
    let runs = response["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0]["agent"], "one");
    assert_eq!(runs[0]["context"]["model"], "model-one");
    assert!(runs[0]["command"]["args"]
        .as_array()
        .unwrap()
        .iter()
        .any(|arg| arg == "agent-one"));
    assert_eq!(runs[1]["agent"], "two");
    assert_eq!(runs[1]["context"]["model"], "model-two");
    assert!(runs[1]["command"]["args"]
        .as_array()
        .unwrap()
        .iter()
        .any(|arg| arg == "agent-two"));
    assert!(!dir.join("docs-multi-agent").join("runs").exists());
}

#[test]
fn help_describes_top_level_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("run"));
    assert!(stdout.contains("Run one delegated task"));
    assert!(stdout.contains("vibe"));
    assert!(stdout.contains("Chat with a selected coding CLI"));
}

#[test]
fn help_uses_user_facing_agent_and_cli_terms() {
    let run_help = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("run")
        .arg("--help")
        .output()
        .unwrap();
    assert!(
        run_help.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run_help.stdout),
        String::from_utf8_lossy(&run_help.stderr)
    );
    let stdout = String::from_utf8_lossy(&run_help.stdout);
    assert!(stdout.contains("--agent <AGENT>"));
    assert!(stdout.contains("--cli <CLI>"));
    assert!(!stdout.contains("<PROFILE>"));
    assert!(!stdout.contains("<BACKEND>"));

    let add_help = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("agents")
        .arg("add")
        .arg("--help")
        .output()
        .unwrap();
    assert!(
        add_help.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&add_help.stdout),
        String::from_utf8_lossy(&add_help.stderr)
    );
    let stdout = String::from_utf8_lossy(&add_help.stdout);
    assert!(stdout.contains("--cli <CLI>"));
    assert!(!stdout.contains("<BACKEND>"));
}

#[test]
fn legacy_agent_and_cli_command_names_are_rejected() {
    for command in [
        "targets",
        "profiles",
        "agent-aliases",
        "backends",
        "cli-types",
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_occ"))
            .arg(command)
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "legacy command unexpectedly succeeded: {command}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn legacy_agent_and_cli_option_aliases_are_rejected() {
    for args in [
        vec![
            "run",
            "--profile",
            "claude",
            "--prompt",
            "hello",
            "--dry-run",
        ],
        vec![
            "run",
            "--target",
            "claude",
            "--prompt",
            "hello",
            "--dry-run",
        ],
        vec![
            "run",
            "--agent-alias",
            "claude",
            "--prompt",
            "hello",
            "--dry-run",
        ],
        vec![
            "run",
            "--backend",
            "claude",
            "--prompt",
            "hello",
            "--dry-run",
        ],
        vec![
            "run",
            "--cli-type",
            "claude",
            "--prompt",
            "hello",
            "--dry-run",
        ],
        vec!["agents", "add", "legacy", "--backend", "claude"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_occ"))
            .args(&args)
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "legacy args unexpectedly succeeded: {:?}\nstdout:\n{}\nstderr:\n{}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn list_container_commands_default_to_list() {
    let dir = temp_dir("container-default-list");
    let worker = compile_worker(&dir);
    let config = write_alias_config(&dir, &worker);

    let agent_aliases = run_occ_text(&config, &["agents"]);
    assert!(agent_aliases.contains("mock"));

    let cli_types = run_occ_text(&config, &["clis"]);
    assert!(cli_types.contains("claude"));

    let runs = run_occ_text(&config, &["runs"]);
    assert!(!runs.contains("required arguments"));
}

#[test]
fn list_table_output_does_not_style_headers() {
    let dir = temp_dir("list-table-plain-header");
    let worker = compile_worker(&dir);
    let config = write_alias_config(&dir, &worker);
    let output = Command::new(env!("CARGO_BIN_EXE_occ"))
        .env("CLICOLOR_FORCE", "1")
        .arg("--config")
        .arg(&config)
        .arg("clis")
        .arg("list")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("CLI"));
    assert!(stdout.contains("claude"));
    assert!(!stdout.contains('\u{1b}'));
}

#[test]
fn skills_list_table_fits_default_terminal_width() {
    let output = Command::new(env!("CARGO_BIN_EXE_occ"))
        .env("COLUMNS", "80")
        .arg("skills")
        .arg("list")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("using-one-code-cli"));
    assert!(stdout.lines().all(|line| line.chars().count() <= 80));
}

#[test]
fn skills_install_removes_obsolete_bundled_files() {
    let dir = temp_dir("skills-install-obsolete");
    let target = dir.join("skills");
    let examples = target.join("using-one-code-cli").join("examples");
    fs::create_dir_all(&examples).unwrap();
    fs::write(examples.join("run-with-profile.md"), "old profile").unwrap();
    fs::write(examples.join("run-with-backend.md"), "old backend").unwrap();
    fs::write(examples.join("custom-note.md"), "keep").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("skills")
        .arg("install")
        .arg("--target")
        .arg(&target)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(examples.join("run-with-agent.md").exists());
    assert!(examples.join("run-with-cli.md").exists());
    assert!(!examples.join("run-with-profile.md").exists());
    assert!(!examples.join("run-with-backend.md").exists());
    assert!(examples.join("custom-note.md").exists());
}

#[test]
fn skills_install_exports_model_aware_using_one_code_cli_docs() {
    let dir = temp_dir("skills-install-model-aware");
    let target = dir.join("skills");

    let output = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("skills")
        .arg("install")
        .arg("--target")
        .arg(&target)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let skill_dir = target.join("using-one-code-cli");
    let skill_toml = fs::read_to_string(skill_dir.join("skill.toml")).unwrap();
    let skill_md = fs::read_to_string(skill_dir.join("SKILL.md")).unwrap();
    let run_with_agent =
        fs::read_to_string(skill_dir.join("examples").join("run-with-agent.md")).unwrap();

    assert!(skill_toml.contains("model = \"Optional model override for the delegated run.\""));
    assert!(skill_toml
        .contains("effort = \"Optional reasoning effort override for the delegated run.\""));
    assert!(skill_toml.contains("model = \"Resolved model used for the run when available.\""));
    assert!(skill_toml.contains("model_source = \"Where the resolved model came from.\""));
    assert!(skill_toml
        .contains("effort = \"Resolved reasoning effort used for the run when available.\""));
    assert!(skill_toml.contains("effort_source = \"Where the resolved effort came from.\""));
    assert!(skill_md.contains("model override"));
    assert!(skill_md.contains("model_source"));
    assert!(skill_md.contains("effort override"));
    assert!(skill_md.contains("effort_source"));
    assert!(run_with_agent.contains("--model <model>"));
    assert!(run_with_agent.contains("--effort <level>"));
}

#[test]
fn skills_install_defaults_to_agents_skills_under_home() {
    let dir = temp_dir("skills-install-default-target");
    let home = dir.join("home");
    fs::create_dir_all(&home).unwrap();

    let output = {
        let mut command = Command::new(env!("CARGO_BIN_EXE_occ"));
        apply_home_env(&mut command, &home);
        command.arg("skills").arg("install").output().unwrap()
    };

    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let installed = home
        .join(".agents")
        .join("skills")
        .join("using-one-code-cli");
    assert!(installed.join("SKILL.md").exists());
    assert!(installed.join("skill.toml").exists());
}

#[test]
fn config_show_uses_language_preference_and_explains_fields() {
    let dir = temp_dir("localized-config-show");
    let worker = compile_worker(&dir);
    let config = write_config(&dir, &worker, "small");

    let zh = Command::new(env!("CARGO_BIN_EXE_occ"))
        .env("OCC_LANG", "zh-CN")
        .arg("--config")
        .arg(&config)
        .arg("config")
        .arg("show")
        .output()
        .unwrap();
    assert!(zh.status.success());
    let zh_stdout = String::from_utf8_lossy(&zh.stdout);
    assert!(zh_stdout.contains("配置概览"));
    assert!(zh_stdout.contains("运行记录目录"));

    let en = Command::new(env!("CARGO_BIN_EXE_occ"))
        .env("OCC_LANG", "en-US")
        .arg("--config")
        .arg(&config)
        .arg("config")
        .arg("show")
        .output()
        .unwrap();
    assert!(en.status.success());
    let en_stdout = String::from_utf8_lossy(&en.stdout);
    assert!(en_stdout.contains("Configuration summary"));
    assert!(en_stdout.contains("run artifact directory"));
}

#[test]
fn settings_export_uses_target_config_instead_of_flattened_effective_config() {
    let dir = temp_dir("settings-target-config");
    let config = dir.join("config-settings-target.toml");
    fs::write(
        &config,
        r#"version = 1
default_agent = "project-agent"

[[agents]]
name = "project-agent"
cli_type = "codex"
args_strategy = "builtin"
"#,
    )
    .unwrap();
    let output_html = dir.join("settings.html");

    let output = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("--config")
        .arg(&config)
        .arg("settings")
        .arg("--target")
        .arg("loaded")
        .arg("--output")
        .arg(&output_html)
        .arg("--no-open")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let html = fs::read_to_string(&output_html).unwrap();
    assert!(html.contains("project-agent"));
    assert!(!html.contains("name = &quot;codex&quot;"));
}

#[test]
fn settings_without_output_serves_form_ui_by_default() {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpStream;
    use std::process::Stdio;

    fn http_request(url: &str, method: &str, path: &str) -> String {
        let address = url
            .trim()
            .strip_prefix("http://")
            .unwrap()
            .trim_end_matches('/');
        let mut stream = TcpStream::connect(address).unwrap();
        write!(
            stream,
            "{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
            method, path, address
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    }

    let dir = temp_dir("settings-default-form-ui");
    let config = dir.join("config-settings-default-form.toml");
    fs::write(
        &config,
        r#"version = 1

[[agents]]
name = "project-agent"
cli_type = "codex"
args_strategy = "builtin"
"#,
    )
    .unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("--config")
        .arg(&config)
        .arg("settings")
        .arg("--target")
        .arg("loaded")
        .arg("--no-open")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut url = String::new();
    for _ in 0..4 {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if let Some(value) = line.strip_prefix("ui: ") {
            url = value.trim().to_string();
            break;
        }
    }
    assert!(!url.is_empty(), "settings server did not print a UI URL");

    let page = http_request(&url, "GET", "/");
    assert!(page.contains("raw-toml-textarea"));
    assert!(page.contains("agent-list"));
    assert!(page.contains("project-agent"));
    assert!(!page.contains("textarea id=\"toml\""));

    let _ = http_request(&url, "POST", "/api/shutdown");
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn agents_add_writes_isolated_agent_config() {
    let dir = temp_dir("agents-add");
    let config = dir.join("config-add.toml");
    fs::write(&config, "version = 1\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("--config")
        .arg(&config)
        .arg("agents")
        .arg("add")
        .arg("deepseek-cc")
        .arg("--cli")
        .arg("claude")
        .arg("--model")
        .arg("deepseek-chat")
        .arg("--env-allow")
        .arg("HTTPS_PROXY")
        .arg("--env")
        .arg("ANTHROPIC_BASE_URL=https://api.deepseek.com/anthropic")
        .arg("--set-cli-default")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("added: deepseek-cc"));

    let raw = fs::read_to_string(&config).unwrap();
    assert!(raw.contains("name = \"deepseek-cc\""));
    assert!(raw.contains("cli_type = \"claude\""));
    assert!(raw.contains("config_dir"));
    assert!(raw.contains("model = \"deepseek-chat\""));
    assert!(raw.contains("env_mode = \"strict\""));
    assert!(raw.contains("env_allowlist = [\"HTTPS_PROXY\"]"));
    assert!(raw.contains("ANTHROPIC_BASE_URL"));
    assert!(raw.contains("claude = \"deepseek-cc\""));

    let shown = run_occ_text(&config, &["agents", "show", "deepseek-cc"]);
    assert!(shown.contains("model = \"deepseek-chat\""));
    assert!(shown.contains("ANTHROPIC_BASE_URL"));
}

#[test]
fn agents_add_can_use_default_inherited_cli_environment() {
    let dir = temp_dir("agents-add-inherit-env");
    let config = dir.join("config-add-inherit.toml");
    fs::write(&config, "version = 1\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("--config")
        .arg(&config)
        .arg("agents")
        .arg("add")
        .arg("default-claude")
        .arg("--cli")
        .arg("claude")
        .arg("--inherit-env")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let raw = fs::read_to_string(&config).unwrap();
    assert!(raw.contains("name = \"default-claude\""));
    assert!(!raw.contains("env_mode = \"strict\""));
    assert!(!raw.contains("config_dir"));

    let output = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("--config")
        .arg(&config)
        .arg("run")
        .arg("--agent")
        .arg("default-claude")
        .arg("--cwd")
        .arg(&dir)
        .arg("--prompt")
        .arg("hello")
        .arg("--output")
        .arg("json")
        .arg("--dry-run")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let env_keys = response["command"]["env_keys"].as_array().unwrap();
    assert!(!env_keys.iter().any(|key| key == "CLAUDE_CONFIG_DIR"));
}

#[test]
fn agents_add_uses_stable_safe_config_dir_segment() {
    let dir = temp_dir("agents-add-safe-segment");
    let config = dir.join("config-add-safe-segment.toml");
    fs::write(&config, "version = 1\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("--config")
        .arg(&config)
        .arg("agents")
        .arg("add")
        .arg("DeepSeek CC")
        .arg("--cli")
        .arg("claude")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let raw = fs::read_to_string(&config).unwrap().replace('\\', "/");
    assert!(raw.contains("/agents/deepseek-cc/system"));
}

#[test]
fn claude_dry_run_assigns_native_session_id_for_future_resume() {
    let dir = temp_dir("claude-native-session-id");
    let config = dir.join("config-claude-native-session.toml");
    fs::write(
        &config,
        r#"version = 1

[[agents]]
name = "mock"
cli_type = "claude"
args_strategy = "builtin"
"#,
    )
    .unwrap();

    let response = run_occ_dry(&config, &dir, &[]);
    let backend_session_id = response["context"]["backend_session_id"]
        .as_str()
        .expect("backend_session_id should be assigned");
    assert!(backend_session_id.contains('-'));
    let args = response["command"]["args"].as_array().unwrap();
    assert!(args.iter().any(|arg| arg == "--session-id"));
    assert!(args.iter().any(|arg| arg == backend_session_id));
}

#[test]
fn run_with_missing_session_id_fails_instead_of_creating_new_session() {
    let dir = temp_dir("missing-session-id");
    let worker = compile_worker(&dir);
    let config = write_config(&dir, &worker, "small");

    let error = run_occ_expect_failure(
        &config,
        &[
            "run",
            "--agent",
            "mock",
            "--cwd",
            dir.to_str().unwrap(),
            "--session",
            "sess_missing",
            "--prompt",
            "hello",
        ],
    );
    assert!(error.contains("session_not_found"));
}

#[test]
fn run_with_session_id_keeps_session_agent_without_resume_flag() {
    let dir = temp_dir("session-id-keeps-agent");
    let worker = compile_worker(&dir);
    let config = write_session_binding_config(&dir, &worker);

    let first = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("--config")
        .arg(&config)
        .arg("run")
        .arg("--agent")
        .arg("alpha")
        .arg("--cwd")
        .arg(&dir)
        .arg("--prompt")
        .arg("hello")
        .arg("--output")
        .arg("json")
        .output()
        .unwrap();
    assert!(
        first.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&first.stderr)
    );
    let first_json: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(first_json["agent"], "alpha");
    let session_id = first_json["session_id"].as_str().unwrap();

    let second = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("--config")
        .arg(&config)
        .arg("run")
        .arg("--session")
        .arg(session_id)
        .arg("--cwd")
        .arg(&dir)
        .arg("--prompt")
        .arg("follow up")
        .arg("--output")
        .arg("json")
        .output()
        .unwrap();
    assert!(
        second.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&second.stdout),
        String::from_utf8_lossy(&second.stderr)
    );
    let second_json: serde_json::Value = serde_json::from_slice(&second.stdout).unwrap();
    assert_eq!(second_json["agent"], "alpha");
    assert_eq!(second_json["model"], "alpha-model");
    assert_eq!(second_json["model_source"], "session");

    let mismatch = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("--config")
        .arg(&config)
        .arg("run")
        .arg("--session")
        .arg(session_id)
        .arg("--agent")
        .arg("beta")
        .arg("--cwd")
        .arg(&dir)
        .arg("--prompt")
        .arg("wrong agent")
        .output()
        .unwrap();
    assert!(
        !mismatch.status.success(),
        "occ unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&mismatch.stdout),
        String::from_utf8_lossy(&mismatch.stderr)
    );
    assert!(String::from_utf8_lossy(&mismatch.stderr).contains("session_agent_mismatch"));
}

#[test]
fn legacy_config_agent_cli_names_are_not_migrated() {
    let dir = temp_dir("legacy-config-names");
    let config = dir.join("config-legacy.toml");
    fs::write(
        &config,
        r#"version = 1

[[agents]]
name = "legacy"
backend = "claude"
"#,
    )
    .unwrap();

    let error = run_occ_expect_failure(&config, &["agents", "list"]);
    assert!(error.contains("config_parse_failed"));
}

#[test]
fn config_dir_is_mapped_to_cli_specific_isolation_env() {
    let dir = temp_dir("config-dir-isolation-env");
    let worker = compile_worker(&dir);
    let codex_system = dir.join("systems").join("codex");
    fs::create_dir_all(&codex_system).unwrap();
    fs::write(
        codex_system.join("config.toml"),
        "model = \"isolated-codex-model\"\nmodel_reasoning_effort = \"xhigh\"\n",
    )
    .unwrap();
    let config = write_isolated_cli_config(&dir, &worker);

    let claude = run_occ_dry_agent(&config, &dir, "claude-iso");
    let claude_env = claude["command"]["env_keys"].as_array().unwrap();
    assert!(claude_env.iter().any(|key| key == "CLAUDE_CONFIG_DIR"));

    let codex = run_occ_dry_agent(&config, &dir, "codex-iso");
    assert_eq!(codex["context"]["model"], "isolated-codex-model");
    assert_eq!(codex["context"]["effort"], "xhigh");
    assert_eq!(codex["model_source"], "cli-config");
    assert_eq!(codex["effort_source"], "cli-config");
    let codex_env = codex["command"]["env_keys"].as_array().unwrap();
    assert!(codex_env.iter().any(|key| key == "CODEX_HOME"));

    let opencode = run_occ_dry_agent(&config, &dir, "opencode-iso");
    let opencode_env = opencode["command"]["env_keys"].as_array().unwrap();
    assert!(opencode_env.iter().any(|key| key == "OPENCODE_CONFIG_DIR"));

    let gemini = run_occ_dry_agent(&config, &dir, "gemini-iso");
    let gemini_env = gemini["command"]["env_keys"].as_array().unwrap();
    assert!(gemini_env.iter().any(|key| key == "HOME"));
    if cfg!(windows) {
        assert!(gemini_env.iter().any(|key| key == "USERPROFILE"));
        assert!(gemini_env.iter().any(|key| key == "APPDATA"));
        assert!(gemini_env.iter().any(|key| key == "LOCALAPPDATA"));
    }
}

#[test]
fn strict_env_mode_rebuilds_child_env_from_agent_allowlist() {
    let dir = temp_dir("strict-env-mode");
    let worker = compile_worker(&dir);
    let config = write_strict_env_config(&dir, &worker);

    let output = Command::new(env!("CARGO_BIN_EXE_occ"))
        .env("OCC_ALLOWED_PARENT", "allowed-value")
        .env("BAD_PARENT_SECRET", "must-not-leak")
        .env("ANTHROPIC_BASE_URL", "https://must-not-leak.example")
        .arg("--config")
        .arg(&config)
        .arg("run")
        .arg("--agent")
        .arg("strict-env")
        .arg("--cwd")
        .arg(&dir)
        .arg("--prompt")
        .arg("hello")
        .arg("--output")
        .arg("json")
        .arg("--timeout")
        .arg("10s")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["success"], true);
    let result = fs::read_to_string(response["result_path"].as_str().unwrap()).unwrap();
    assert!(result.contains("OCC_ALLOWED_PARENT=allowed-value"));
    assert!(result.contains("BAD_PARENT_SECRET=<missing>"));
    assert!(result.contains("ANTHROPIC_BASE_URL=<missing>"));
    assert!(result.contains("CHILD_ONLY=child-value"));
    assert!(result.contains("CLAUDE_CONFIG_DIR="));
}

#[test]
fn strict_env_mode_still_forwards_enabled_proxy_keys() {
    let dir = temp_dir("strict-env-proxy");
    let worker = compile_worker(&dir);
    let config = write_strict_env_config(&dir, &worker);

    let output = Command::new(env!("CARGO_BIN_EXE_occ"))
        .env("HTTPS_PROXY", "http://127.0.0.1:8317")
        .arg("--config")
        .arg(&config)
        .arg("run")
        .arg("--agent")
        .arg("strict-env")
        .arg("--cwd")
        .arg(&dir)
        .arg("--prompt")
        .arg("hello")
        .arg("--output")
        .arg("json")
        .arg("--dry-run")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let env_keys = response["command"]["env_keys"].as_array().unwrap();
    assert!(env_keys.iter().any(|key| key == "HTTPS_PROXY"));
}

#[test]
fn vibe_slash_commands_can_switch_backend_model_and_report_status() {
    let dir = temp_dir("vibe-slash-commands");
    let worker = compile_worker(&dir);
    let config = write_alias_config(&dir, &worker);

    let output = run_occ_text_with_stdin(
        &config,
        &["vibe"],
        "/help\n/cli codex\n/model test-model\n/status\n/exit\n",
    );

    assert!(output.contains("cli: codex"));
    assert!(output.contains("model: test-model"));
    assert!(output.contains("/cli <name>"));
    assert!(!output.contains("/target"));
    assert!(!output.contains("/profile"));
    assert!(!output.contains("/backend"));
    assert!(!output.contains("/cli-type"));
}

#[test]
fn vibe_run_summary_reports_effective_model_and_source() {
    let dir = temp_dir("vibe-run-model-summary");
    let worker = compile_worker(&dir);
    let config = write_model_effort_override_codex_config(&dir, &worker, "small");

    let output = run_occ_text_with_stdin(&config, &["vibe", "--agent", "mock"], "hello\n/exit\n");

    assert!(output.contains("model: profile-model"));
    assert!(output.contains("model_source: agent"));
    assert!(output.contains("effort: high"));
    assert!(output.contains("effort_source: agent"));
}

#[test]
fn vibe_slash_commands_can_switch_effort_and_report_status() {
    let dir = temp_dir("vibe-slash-effort");
    let worker = compile_worker(&dir);
    let config = write_alias_config(&dir, &worker);

    let output =
        run_occ_text_with_stdin(&config, &["vibe"], "/help\n/effort xhigh\n/status\n/exit\n");

    assert!(output.contains("effort: xhigh"));
    assert!(output.contains("/effort <level>"));
}

#[test]
fn default_doc_root_is_user_occ_when_config_omits_doc_root() {
    let dir = temp_dir("default-doc-root");
    let worker = compile_worker(&dir);
    let config = write_config_without_doc_root(&dir, &worker, "small");

    let response = run_occ_dry(&config, &dir, &[]);
    let doc_root = PathBuf::from(response["context"]["doc_root"].as_str().unwrap());

    assert!(doc_root.is_absolute());
    assert_eq!(doc_root.file_name().unwrap(), ".occ");
    assert_ne!(doc_root, dir.join(".occ"));
}

#[test]
fn run_without_prompt_defaults_to_interactive_mode() {
    let dir = temp_dir("default-interactive");
    let worker = compile_worker(&dir);
    let config = write_config_without_doc_root(&dir, &worker, "small");

    let response = run_occ_dry_without_prompt(&config, &dir);

    assert_eq!(response["success"], true);
    assert_eq!(response["command"]["args"][0], "interactive-mode");
    assert_eq!(response["command"]["prompt_via_stdin"], false);
}

#[test]
fn stream_flag_mirrors_non_interactive_child_output_to_parent_stderr() {
    let dir = temp_dir("stream-flag");
    let worker = compile_worker(&dir);
    let config = write_config(&dir, &worker, "small");
    let output = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("--config")
        .arg(&config)
        .arg("run")
        .arg("--agent")
        .arg("mock")
        .arg("--cwd")
        .arg(&dir)
        .arg("--prompt")
        .arg("hello")
        .arg("--output")
        .arg("json")
        .arg("--stream")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("small-stdout-line"));
    assert!(stderr.contains("small-stderr-line"));
}

#[test]
fn stream_flag_survives_non_utf8_child_stderr() {
    let dir = temp_dir("stream-non-utf8");
    let worker = compile_worker(&dir);
    let config = write_config(&dir, &worker, "invalid-stderr");
    let output = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("--config")
        .arg(&config)
        .arg("run")
        .arg("--agent")
        .arg("mock")
        .arg("--cwd")
        .arg(&dir)
        .arg("--prompt")
        .arg("hello")
        .arg("--output")
        .arg("json")
        .arg("--stream")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["success"], true);
    let stderr_log = response["metadata_path"]
        .as_str()
        .map(Path::new)
        .unwrap()
        .with_file_name("stderr.log");
    assert!(fs::read(stderr_log).unwrap().contains(&0xff));
}

#[test]
fn timeout_keeps_partial_streamed_logs() {
    let dir = temp_dir("timeout-output");
    let worker = compile_worker(&dir);
    let config = write_config(&dir, &worker, "timeout");

    let response = run_occ(&config, &dir, "1s");
    assert_eq!(response["success"], false);
    assert_eq!(response["error"]["code"], "timeout");
    let result_path = PathBuf::from(response["result_path"].as_str().unwrap());
    let run_dir = result_path.parent().unwrap();
    let stdout = fs::read_to_string(run_dir.join("stdout.log")).unwrap();
    let stderr = fs::read_to_string(run_dir.join("stderr.log")).unwrap();

    assert!(stdout.contains("partial-stdout-before-timeout"));
    assert!(stderr.contains("partial-stderr-before-timeout"));
}

#[cfg(windows)]
#[test]
fn timeout_terminates_spawned_child_process_tree() {
    let dir = temp_dir("timeout-process-tree");
    let worker = compile_worker(&dir);
    let marker = dir.join("child-survived.txt");
    let config = write_tree_timeout_config(&dir, &worker, &marker);

    let response = run_occ(&config, &dir, "1s");
    assert_eq!(response["success"], false);
    assert_eq!(response["error"]["code"], "timeout");
    thread::sleep(Duration::from_secs(4));

    assert!(!marker.exists());
}

#[test]
fn dry_run_shows_gemini_file_indirection_for_multiline_prompt() {
    let dir = temp_dir("dry-run-gemini");
    let config = write_gemini_config(&dir);

    let output = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("--config")
        .arg(&config)
        .arg("run")
        .arg("--cli")
        .arg("gemini")
        .arg("--cwd")
        .arg(&dir)
        .arg("--prompt")
        .arg("line one\nline two")
        .arg("--output")
        .arg("json")
        .arg("--dry-run")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["success"], true);
    assert_eq!(response["command"]["prompt_transport"], "file-indirection");
    assert!(response["command"]["args"]
        .as_array()
        .unwrap()
        .iter()
        .any(|arg| arg
            .as_str()
            .unwrap()
            .starts_with("Read and follow the task in ")));
    assert!(!response["command"]["args"]
        .as_array()
        .unwrap()
        .iter()
        .any(|arg| arg.as_str().unwrap().contains("line one\nline two")));
    assert!(!dir.join("docs-gemini").join("runs").exists());
}

#[test]
fn timeout_defaults_follow_cli_profile_global_precedence() {
    let dir = temp_dir("timeout-precedence");
    let worker = compile_worker(&dir);

    let global_config = write_timeout_config(&dir, &worker, "large", "3s", None);
    let global = run_occ_dry(&global_config, &dir, &[]);
    assert_eq!(global["command"]["timeout"], "3s");

    let profile_config = write_timeout_config(&dir, &worker, "large", "3s", Some("2s"));
    let profile = run_occ_dry(&profile_config, &dir, &[]);
    assert_eq!(profile["command"]["timeout"], "2s");

    let cli = run_occ_dry(&profile_config, &dir, &["--timeout", "1s"]);
    assert_eq!(cli["command"]["timeout"], "1s");

    let none = run_occ_dry(&profile_config, &dir, &["--timeout", "none"]);
    assert!(none["command"]["timeout"].is_null());
}

#[test]
fn selected_timeout_is_written_to_run_metadata() {
    let dir = temp_dir("timeout-metadata");
    let worker = compile_worker(&dir);
    let config = write_timeout_config(&dir, &worker, "quick", "5s", Some("4s"));

    let response = run_occ_with_default_timeout(&config, &dir);
    assert_eq!(response["success"], true);
    let result_path = PathBuf::from(response["result_path"].as_str().unwrap());
    let run_dir = result_path.parent().unwrap();
    let command_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(run_dir.join("command.json")).unwrap()).unwrap();
    let run_toml = fs::read_to_string(run_dir.join("run.toml")).unwrap();

    assert_eq!(command_json["timeout"], "4s");
    assert!(run_toml.contains("timeout = \"4s\""));
}

#[test]
fn model_source_is_reported_for_profile_and_cli_model() {
    let dir = temp_dir("model-source");
    let worker = compile_worker(&dir);
    let config = write_model_config(&dir, &worker);

    let profile = run_occ_dry(&config, &dir, &[]);
    assert_eq!(profile["context"]["model"], "profile-model");
    assert_eq!(profile["model_source"], "agent");

    let cli = run_occ_dry(&config, &dir, &["--model", "cli-model"]);
    assert_eq!(cli["context"]["model"], "cli-model");
    assert_eq!(cli["model_source"], "cli-arg");

    let response = run_occ_with_default_timeout(&config, &dir);
    assert_eq!(response["model"], "profile-model");
    assert_eq!(response["model_source"], "agent");
}

#[test]
fn codex_dry_run_reports_effort_source_and_args() {
    let dir = temp_dir("codex-effort-dry");
    let config = write_model_effort_builtin_codex_config(&dir);

    let profile = run_occ_dry(&config, &dir, &[]);
    assert_eq!(profile["context"]["model"], "profile-model");
    assert_eq!(profile["context"]["effort"], "high");
    assert_eq!(profile["model_source"], "agent");
    assert_eq!(profile["effort_source"], "agent");

    let cli = run_occ_dry(
        &config,
        &dir,
        &["--model", "cli-model", "--effort", "xhigh"],
    );
    assert_eq!(cli["context"]["model"], "cli-model");
    assert_eq!(cli["context"]["effort"], "xhigh");
    assert_eq!(cli["model_source"], "cli-arg");
    assert_eq!(cli["effort_source"], "cli-arg");
    assert!(cli["command"]["args"]
        .as_array()
        .unwrap()
        .iter()
        .any(|arg| arg.as_str().unwrap() == "--model"));
    assert!(cli["command"]["args"]
        .as_array()
        .unwrap()
        .iter()
        .any(|arg| arg.as_str().unwrap() == "cli-model"));
    assert!(cli["command"]["args"]
        .as_array()
        .unwrap()
        .iter()
        .any(|arg| arg.as_str().unwrap() == "model_reasoning_effort=\"xhigh\""));
}

#[test]
fn codex_cli_config_defaults_are_reported_for_model_and_effort() {
    let dir = temp_dir("codex-cli-defaults");
    let home = dir.join("home");
    let codex_home = home.join(".codex");
    fs::create_dir_all(&codex_home).unwrap();
    fs::write(
        codex_home.join("config.toml"),
        "model = \"gpt-5.4\"\nmodel_reasoning_effort = \"xhigh\"\n",
    )
    .unwrap();

    let config = write_model_effort_builtin_codex_config(&dir);
    let response = run_occ_dry_with_env(&config, &dir, &[], &[("CODEX_HOME", &codex_home)]);

    assert_eq!(response["context"]["model"], "profile-model");
    assert_eq!(response["context"]["effort"], "high");

    let worker = compile_worker(&dir);
    let config = dir.join("config-cli-defaults.toml");
    fs::write(
        &config,
        format!(
            r#"version = 1
doc_root = "{}"

[[agents]]
name = "mock"
cli_type = "codex"
path = "{}"
args_strategy = "override"
args = ["small"]
prompt_via = "stdin"
"#,
            toml_string(&dir.join("docs-cli-defaults")),
            toml_string(&worker),
        ),
    )
    .unwrap();

    let response = run_occ_dry_with_env(&config, &dir, &[], &[("CODEX_HOME", &codex_home)]);
    assert_eq!(response["context"]["model"], "gpt-5.4");
    assert_eq!(response["context"]["effort"], "xhigh");
    assert_eq!(response["model_source"], "cli-config");
    assert_eq!(response["effort_source"], "cli-config");
}

#[test]
fn runs_list_shows_model_column_and_values() {
    let dir = temp_dir("runs-list-model");
    let worker = compile_worker(&dir);
    let config = write_model_effort_override_codex_config(&dir, &worker, "small");

    let response = run_occ_with_default_timeout(&config, &dir);
    assert_eq!(response["model"], "profile-model");
    assert_eq!(response["effort"], "high");

    let runs = run_occ_text(&config, &["runs", "list"]);
    assert!(runs.contains("MODEL"));
    assert!(runs.contains("EFFORT"));
    assert!(runs.contains("profile-model"));
    assert!(runs.contains("high"));
}

#[test]
fn profile_alias_can_select_profile() {
    let dir = temp_dir("profile-alias");
    let worker = compile_worker(&dir);
    let config = write_alias_config(&dir, &worker);

    let response = run_occ_profile(&config, &dir, "m");

    assert_eq!(response["success"], true);
    assert_eq!(response["agent"], "mock");
}

#[test]
fn config_validate_rejects_conflicting_profile_alias() {
    let dir = temp_dir("profile-alias-conflict");
    let worker = compile_worker(&dir);
    let config = write_conflicting_alias_config(&dir, &worker);

    let error = run_occ_expect_failure(&config, &["config", "validate"]);

    assert!(error.contains("profile_alias_conflict"));
}

#[test]
fn backend_alias_can_select_backend() {
    let dir = temp_dir("backend-alias");
    let worker = compile_worker(&dir);
    let config = write_backend_alias_config(&dir, &worker);

    let response = run_occ_backend(&config, &dir, "c");

    assert_eq!(response["success"], true);
    assert_eq!(response["agent"], "mock");
    assert_eq!(response["cli"], "claude");
}

#[test]
fn new_config_names_can_select_agent_alias_and_cli_type_alias() {
    let dir = temp_dir("new-config-names");
    let worker = compile_worker(&dir);
    let config = write_new_names_config(&dir, &worker);

    let agent_alias_response = run_occ_profile(&config, &dir, "mock");
    assert_eq!(agent_alias_response["success"], true);
    assert_eq!(agent_alias_response["agent"], "mock");

    let cli_response = run_occ_backend(&config, &dir, "c");
    assert_eq!(cli_response["success"], true);
    assert_eq!(cli_response["cli"], "claude");

    let raw = run_occ_text(&config, &["config", "show", "--raw"]);
    assert!(raw.contains("default_agent"));
    assert!(raw.contains("[[agents]]"));
    assert!(raw.contains("cli_type = \"claude\""));
}

#[test]
fn clis_show_accepts_cli_alias() {
    let dir = temp_dir("backend-alias-show");
    let worker = compile_worker(&dir);
    let config = write_backend_alias_config(&dir, &worker);

    let output = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("--config")
        .arg(&config)
        .arg("clis")
        .arg("show")
        .arg("c")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(response["name"], "claude");
    assert!(response["aliases"]
        .as_array()
        .unwrap()
        .iter()
        .any(|alias| alias == "c"));
}

#[test]
fn config_validate_rejects_backend_alias_shadowing_backend_name() {
    let dir = temp_dir("backend-alias-conflict");
    let config = write_conflicting_backend_alias_config(&dir);

    let error = run_occ_expect_failure(&config, &["config", "validate"]);

    assert!(error.contains("backend_alias_conflict"));
}

#[test]
fn config_validate_allows_identity_backend_aliases() {
    let dir = temp_dir("backend-alias-identity");
    let config = write_identity_backend_alias_config(&dir);

    let output = run_occ_text(&config, &["config", "validate"]);

    assert!(output.contains("ok"));
}

#[test]
fn config_validate_rejects_profile_alias_shadowing_backend_name() {
    let dir = temp_dir("profile-alias-shadows-backend");
    let worker = compile_worker(&dir);
    let config = write_profile_alias_shadows_backend_config(&dir, &worker);

    let error = run_occ_expect_failure(&config, &["config", "validate"]);

    assert!(error.contains("profile_alias_conflict"));
}

#[test]
fn config_validate_rejects_backend_alias_shadowing_profile_name() {
    let dir = temp_dir("backend-alias-shadows-profile");
    let worker = compile_worker(&dir);
    let config = write_backend_alias_shadows_profile_config(&dir, &worker);

    let error = run_occ_expect_failure(&config, &["config", "validate"]);

    assert!(error.contains("backend_alias_conflict"));
}

#[test]
fn doctor_reports_alias_semantic_errors_without_failing() {
    let dir = temp_dir("doctor-alias-conflict");
    let worker = compile_worker(&dir);
    let config = write_profile_alias_shadows_backend_config(&dir, &worker);

    let output = run_occ_text(&config, &["doctor"]);

    assert!(output.contains("config_semantics") && output.contains("profile_alias_conflict"));
}

#[test]
fn profiles_list_shows_aliases() {
    let dir = temp_dir("profiles-list-aliases");
    let worker = compile_worker(&dir);
    let config = write_alias_config(&dir, &worker);

    let output = run_occ_text(&config, &["agents", "list"]);

    assert!(
        output.contains("mock")
            && output.contains("claude")
            && output.contains("config")
            && output.contains("aliases=m")
    );
}

#[test]
fn export_html_includes_target_metadata_and_static_file_controls() {
    let dir = temp_dir("export-html");
    let config = write_export_config(&dir);
    let html_path = dir.join("occ-config.html");

    let output = Command::new(env!("CARGO_BIN_EXE_occ"))
        .current_dir(&dir)
        .arg("--config")
        .arg(&config)
        .arg("config")
        .arg("export-html")
        .arg("--target")
        .arg("project")
        .arg("--output")
        .arg(&html_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "occ failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let html = fs::read_to_string(&html_path).unwrap();
    let project_config = dir.join(".occ").join("config.toml");

    assert!(html.contains("const metadata = {"));
    assert!(html.contains(r#""target":"project""#));
    assert!(html.contains(&toml_string(&project_config)));
    assert!(html.contains("Open Config File"));
    assert!(html.contains("Save to Opened File"));
    assert!(html.contains("Browsers cannot silently write local config files"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("recommended_config:"));
}
