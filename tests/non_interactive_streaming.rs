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
use std::process::Command;
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
        Some("invalid-stderr") => {
            println!("invalid-stdout-line");
            io::stderr().write_all(&[0xff, b'\n']).unwrap();
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

fn write_model_config(dir: &Path, worker: &Path) -> PathBuf {
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
args = ["large"]
prompt_via = "stdin"
"#,
            toml_string(&doc_root),
            toml_string(worker),
        ),
    )
    .unwrap();
    config
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
fn backends_show_accepts_backend_alias() {
    let dir = temp_dir("backend-alias-show");
    let worker = compile_worker(&dir);
    let config = write_backend_alias_config(&dir, &worker);

    let output = Command::new(env!("CARGO_BIN_EXE_occ"))
        .arg("--config")
        .arg(&config)
        .arg("backends")
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
