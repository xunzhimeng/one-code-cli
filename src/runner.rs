use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use chrono::Utc;
use serde::Serialize;
use wait_timeout::ChildExt;

use crate::backend::{self, CommandPlan, TemplateContext};
use crate::cli::{OutputMode, RunArgs, SessionResumeArgs};
use crate::config::{self, EffectiveConfig, Profile, ProxyConfig};
use crate::documents::{self, RunPaths};
use crate::error::{OccError, OccResult};
use crate::ids;
use crate::output::{print_run_response, ErrorBody, RunResponse};
use crate::run_record::{self, RunRecord};
use crate::session::{self, SessionRecord};

#[derive(Debug, Clone)]
struct PromptData {
    text: Option<String>,
    source: String,
    file: Option<PathBuf>,
}

#[derive(Debug)]
struct ChildResult {
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
    timed_out: bool,
}

pub fn run(config_arg: Option<&PathBuf>, args: RunArgs) -> OccResult<()> {
    execute_run(config_arg, args)
}

pub fn resume_session(config_arg: Option<&PathBuf>, args: SessionResumeArgs) -> OccResult<()> {
    let run_args = RunArgs {
        profile: None,
        backend: None,
        model: args.model,
        cwd: args.cwd,
        prompt: args.prompt,
        prompt_file: args.prompt_file,
        stdin: args.stdin,
        interactive: false,
        non_interactive: true,
        session: Some(args.session_id),
        resume: true,
        doc_root: args.doc_root,
        output: args.output,
        timeout: None,
        dry_run: args.dry_run,
        child_args: args.child_args,
    };
    execute_run(config_arg, run_args)
}

fn execute_run(config_arg: Option<&PathBuf>, args: RunArgs) -> OccResult<()> {
    if args.interactive && args.non_interactive {
        return Err(OccError::new(
            "child_process_failed",
            "--interactive and --non-interactive cannot be used together.",
        ));
    }

    let mut cwd = resolve_cwd(args.cwd.as_ref())?;
    let mut config = config::load(config_arg, &cwd)?;
    let mut doc_root = config.resolved_doc_root(&cwd, args.doc_root.as_ref());
    let prompt = read_prompt(
        &args.prompt,
        args.prompt_file.as_ref(),
        args.stdin,
        args.interactive,
    )?;

    let mut existing_session = if args.resume {
        if let Some(session_id) = &args.session {
            Some(session::load_by_id(&doc_root, session_id)?)
        } else {
            None
        }
    } else if let Some(session_id) = &args.session {
        session::load_by_id(&doc_root, session_id).ok()
    } else {
        None
    };

    if let Some(session) = &existing_session {
        if args.cwd.is_none() {
            cwd = session.cwd.clone();
            config = config::load(config_arg, &cwd)?;
            doc_root = config.resolved_doc_root(&cwd, args.doc_root.as_ref());
        }
    }

    let profile = resolve_profile_for_run(&config, &args, existing_session.as_ref())?;
    let backend_spec = backend::require(&profile.backend)?;
    if args.resume && !backend_spec.supports_resume && profile.resume_args.is_empty() {
        return Err(OccError::new(
            "resume_unsupported",
            format!("Profile '{}' does not support native resume.", profile.name),
        ));
    }

    if args.resume && existing_session.is_none() {
        let latest = session::latest(
            &doc_root,
            Some(&profile.name),
            Some(&profile.backend),
            Some(&cwd),
        )?
        .ok_or_else(|| {
            OccError::new(
                "session_not_found",
                format!(
                    "No latest session was found for profile '{}' and cwd '{}'.",
                    profile.name,
                    cwd.display()
                ),
            )
        })?;
        existing_session = Some(session::load_from_path(&latest.session_path)?);
    }

    let model = args
        .model
        .clone()
        .or_else(|| {
            existing_session
                .as_ref()
                .and_then(|session| session.model.clone())
        })
        .or_else(|| profile.model.clone());
    let run_id = ids::run_id();
    let now = Utc::now();
    let mut session_record = existing_session.unwrap_or_else(|| {
        SessionRecord::new(
            args.session.clone().unwrap_or_else(ids::session_id),
            profile.name.clone(),
            profile.backend.clone(),
            cwd.clone(),
            model.clone(),
            now,
        )
    });
    session_record.profile = profile.name.clone();
    session_record.backend = profile.backend.clone();
    session_record.cwd = cwd.clone();
    session_record.model = model.clone();

    let context = TemplateContext {
        profile: profile.name.clone(),
        backend: profile.backend.clone(),
        model: model.clone(),
        cwd: cwd.clone(),
        prompt: prompt.text.clone(),
        prompt_file: prompt.file.clone(),
        config_dir: profile.config_dir.clone(),
        session_id: session_record.session_id.clone(),
        backend_session_id: session_record.backend_session_id.clone(),
        run_id: run_id.clone(),
        doc_root: doc_root.clone(),
    };
    let mut plan = backend::build_command_plan(
        &profile,
        &context,
        args.interactive,
        args.resume,
        &args.child_args,
    )?;
    apply_proxy_config(&config.proxy, &mut plan);

    if args.dry_run {
        print_dry_run(&profile, &context, &plan, args.output)?;
        return Ok(());
    }

    ensure_executable(&plan.executable)?;
    fs::create_dir_all(&doc_root).map_err(|error| {
        OccError::io(
            "doc_root_not_writable",
            format!("Failed to create '{}'", doc_root.display()),
            error,
        )
    })?;

    let started_at = Utc::now();
    let timeout = parse_timeout(args.timeout.as_deref())?;
    let child = if args.interactive {
        execute_interactive(&plan, timeout)?
    } else {
        execute_non_interactive(&plan, timeout)?
    };
    let finished_at = Utc::now();
    let timed_out = child.timed_out;
    let success = !timed_out && child.exit_code == Some(0);

    let paths = RunPaths::new(&doc_root, &run_id);
    let record = RunRecord {
        run_id: run_id.clone(),
        session_id: session_record.session_id.clone(),
        profile: profile.name.clone(),
        backend: profile.backend.clone(),
        model: model.clone(),
        cwd: cwd.clone(),
        prompt_source: prompt.source.clone(),
        interactive: args.interactive,
        success,
        exit_code: child.exit_code,
        started_at,
        finished_at,
        result_path: paths.result_md.clone(),
        metadata_path: paths.run_toml.clone(),
    };

    documents::write_run_files(
        &paths,
        prompt.text.as_deref(),
        &child.stdout,
        &child.stderr,
        &plan,
        &record,
    )?;
    run_record::append_index(&doc_root, &record)?;

    session_record.latest_run_id = Some(run_id.clone());
    session_record.updated_at = finished_at;
    session::save(&doc_root, &session_record)?;
    session::append_run(&doc_root, &session_record.session_id, &run_id, started_at)?;

    let error = if timed_out {
        Some(ErrorBody {
            code: "timeout".to_string(),
            message: "Child process timed out.".to_string(),
        })
    } else if !success {
        Some(ErrorBody {
            code: "child_process_failed".to_string(),
            message: format!("Child process exited with code {:?}.", child.exit_code),
        })
    } else {
        None
    };

    let response = RunResponse {
        success,
        run_id,
        session_id: session_record.session_id,
        profile: profile.name,
        backend: profile.backend,
        cwd,
        result_path: paths.result_md,
        metadata_path: paths.run_toml,
        exit_code: child.exit_code,
        error,
    };
    print_run_response(args.output, &response)
}

fn resolve_profile_for_run(
    config: &EffectiveConfig,
    args: &RunArgs,
    session: Option<&SessionRecord>,
) -> OccResult<Profile> {
    if args.resume {
        if let Some(session) = session {
            return config.resolve_profile(Some(&session.profile), None);
        }
    }
    config.resolve_profile(args.profile.as_deref(), args.backend.as_deref())
}

fn resolve_cwd(cwd: Option<&PathBuf>) -> OccResult<PathBuf> {
    let path = cwd
        .cloned()
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    if !path.exists() {
        return Err(OccError::new(
            "cwd_not_found",
            format!("Working directory '{}' was not found.", path.display()),
        ));
    }
    let metadata = fs::metadata(&path).map_err(|error| {
        OccError::io(
            "cwd_not_found",
            format!("Failed to inspect '{}'", path.display()),
            error,
        )
    })?;
    if !metadata.is_dir() {
        return Err(OccError::new(
            "cwd_not_found",
            format!("Working directory '{}' is not a directory.", path.display()),
        ));
    }
    path.canonicalize().map_err(|error| {
        OccError::io(
            "cwd_not_found",
            format!("Failed to canonicalize '{}'", path.display()),
            error,
        )
    })
}

fn read_prompt(
    prompt: &Option<String>,
    prompt_file: Option<&PathBuf>,
    stdin: bool,
    interactive: bool,
) -> OccResult<PromptData> {
    let count = prompt.is_some() as usize + prompt_file.is_some() as usize + stdin as usize;
    if count > 1 {
        return Err(OccError::new(
            "invalid_prompt_source",
            "Use only one of --prompt, --prompt-file, or --stdin.",
        ));
    }
    if count == 0 && !interactive {
        return Err(OccError::new(
            "invalid_prompt_source",
            "Non-interactive runs require --prompt, --prompt-file, or --stdin.",
        ));
    }

    if let Some(value) = prompt {
        return Ok(PromptData {
            text: Some(value.clone()),
            source: "prompt".to_string(),
            file: None,
        });
    }

    if let Some(path) = prompt_file {
        let text = fs::read_to_string(path).map_err(|error| {
            OccError::io(
                "invalid_prompt_source",
                format!("Failed to read prompt file '{}'", path.display()),
                error,
            )
        })?;
        return Ok(PromptData {
            text: Some(text),
            source: format!("prompt-file:{}", path.display()),
            file: Some(path.clone()),
        });
    }

    if stdin {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .map_err(|error| {
                OccError::io(
                    "invalid_prompt_source",
                    "Failed to read prompt from stdin",
                    error,
                )
            })?;
        return Ok(PromptData {
            text: Some(text),
            source: "stdin".to_string(),
            file: None,
        });
    }

    Ok(PromptData {
        text: None,
        source: "none".to_string(),
        file: None,
    })
}

fn execute_non_interactive(
    plan: &CommandPlan,
    timeout: Option<Duration>,
) -> OccResult<ChildResult> {
    let mut command = Command::new(&plan.executable);
    command
        .args(&plan.args)
        .current_dir(&plan.cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    apply_command_env(&mut command, plan);
    if plan.prompt_stdin.is_some() {
        command.stdin(Stdio::piped());
    }

    let mut child = command.spawn().map_err(|error| {
        OccError::io(
            "child_process_failed",
            format!("Failed to spawn '{}'", plan.executable.display()),
            error,
        )
    })?;

    if let Some(input) = &plan.prompt_stdin {
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input.as_bytes()).map_err(|error| {
                OccError::io(
                    "child_process_failed",
                    "Failed to write prompt to child stdin",
                    error,
                )
            })?;
        }
    }

    if let Some(timeout) = timeout {
        match child.wait_timeout(timeout).map_err(|error| {
            OccError::io(
                "child_process_failed",
                "Failed while waiting for child process",
                error,
            )
        })? {
            Some(status) => {
                let stdout = read_pipe(child.stdout.take())?;
                let stderr = read_pipe(child.stderr.take())?;
                Ok(ChildResult {
                    stdout,
                    stderr,
                    exit_code: status.code(),
                    timed_out: false,
                })
            }
            None => {
                let _ = child.kill();
                let _ = child.wait();
                let stdout = read_pipe(child.stdout.take()).unwrap_or_default();
                let stderr = read_pipe(child.stderr.take()).unwrap_or_default();
                Ok(ChildResult {
                    stdout,
                    stderr,
                    exit_code: None,
                    timed_out: true,
                })
            }
        }
    } else {
        let output = child.wait_with_output().map_err(|error| {
            OccError::io(
                "child_process_failed",
                "Failed while waiting for child process",
                error,
            )
        })?;
        Ok(ChildResult {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            exit_code: output.status.code(),
            timed_out: false,
        })
    }
}

fn execute_interactive(plan: &CommandPlan, timeout: Option<Duration>) -> OccResult<ChildResult> {
    let mut command = Command::new(&plan.executable);
    command
        .args(&plan.args)
        .current_dir(&plan.cwd)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    apply_command_env(&mut command, plan);
    let mut child = command.spawn().map_err(|error| {
        OccError::io(
            "child_process_failed",
            format!("Failed to spawn '{}'", plan.executable.display()),
            error,
        )
    })?;

    if let Some(timeout) = timeout {
        match child.wait_timeout(timeout).map_err(|error| {
            OccError::io(
                "child_process_failed",
                "Failed while waiting for child process",
                error,
            )
        })? {
            Some(status) => Ok(ChildResult {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: status.code(),
                timed_out: false,
            }),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                Ok(ChildResult {
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: None,
                    timed_out: true,
                })
            }
        }
    } else {
        let status = child.wait().map_err(|error| {
            OccError::io(
                "child_process_failed",
                "Failed while waiting for child process",
                error,
            )
        })?;
        Ok(ChildResult {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: status.code(),
            timed_out: false,
        })
    }
}

fn read_pipe<R: Read>(pipe: Option<R>) -> OccResult<String> {
    let mut text = String::new();
    if let Some(mut pipe) = pipe {
        pipe.read_to_string(&mut text).map_err(|error| {
            OccError::io("child_process_failed", "Failed to read child output", error)
        })?;
    }
    Ok(text)
}

fn ensure_executable(executable: &Path) -> OccResult<()> {
    if executable.is_absolute() || executable.components().count() > 1 {
        if executable.exists() {
            return Ok(());
        }
        return Err(OccError::new(
            "executable_not_found",
            format!("Executable '{}' was not found.", executable.display()),
        ));
    }
    which::which(executable).map(|_| ()).map_err(|_| {
        OccError::new(
            "executable_not_found",
            format!(
                "Executable '{}' was not found in PATH.",
                executable.display()
            ),
        )
    })
}

fn apply_proxy_config(proxy: &ProxyConfig, plan: &mut CommandPlan) {
    if proxy.enabled {
        for key in &proxy.env_keys {
            if !plan.env.contains_key(key) {
                if let Ok(value) = env::var(key) {
                    plan.env.insert(key.clone(), value);
                }
            }
        }
    } else {
        for key in &proxy.env_keys {
            plan.env.remove(key);
            if !plan.env_remove.contains(key) {
                plan.env_remove.push(key.clone());
            }
        }
    }
}

fn apply_command_env(command: &mut Command, plan: &CommandPlan) {
    for key in &plan.env_remove {
        command.env_remove(key);
    }
    command.envs(&plan.env);
}

fn parse_timeout(value: Option<&str>) -> OccResult<Option<Duration>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let parse_num = |text: &str| -> OccResult<u64> {
        text.parse::<u64>()
            .map_err(|_| OccError::new("timeout", format!("Invalid timeout duration '{}'.", value)))
    };
    if let Some(number) = value.strip_suffix("ms") {
        Ok(Some(Duration::from_millis(parse_num(number)?)))
    } else if let Some(number) = value.strip_suffix('s') {
        Ok(Some(Duration::from_secs(parse_num(number)?)))
    } else if let Some(number) = value.strip_suffix('m') {
        Ok(Some(Duration::from_secs(parse_num(number)? * 60)))
    } else {
        Ok(Some(Duration::from_secs(parse_num(value)?)))
    }
}

fn print_dry_run(
    profile: &Profile,
    context: &TemplateContext,
    plan: &CommandPlan,
    output: OutputMode,
) -> OccResult<()> {
    #[derive(Serialize)]
    struct DryCommand<'a> {
        executable: &'a Path,
        args: &'a [String],
        cwd: &'a Path,
        env_keys: Vec<&'a String>,
        env_removed: &'a [String],
        prompt_via_stdin: bool,
        prompt_file: Option<&'a PathBuf>,
    }

    #[derive(Serialize)]
    struct DryRun<'a> {
        success: bool,
        profile: &'a Profile,
        context: &'a TemplateContext,
        command: DryCommand<'a>,
    }

    let dry_run = DryRun {
        success: true,
        profile,
        context,
        command: DryCommand {
            executable: &plan.executable,
            args: &plan.args,
            cwd: &plan.cwd,
            env_keys: plan.env.keys().collect(),
            env_removed: &plan.env_remove,
            prompt_via_stdin: plan.prompt_stdin.is_some(),
            prompt_file: plan.prompt_file.as_ref(),
        },
    };

    match output {
        OutputMode::Json => {
            let text = serde_json::to_string_pretty(&dry_run).map_err(|error| {
                OccError::new(
                    "config_parse_failed",
                    format!("Failed to serialize dry-run JSON: {}", error),
                )
            })?;
            println!("{}", text);
        }
        _ => {
            println!("profile: {}", profile.name);
            println!("backend: {}", profile.backend);
            println!("cwd: {}", plan.cwd.display());
            println!(
                "command: {} {}",
                plan.executable.display(),
                plan.args.join(" ")
            );
            if !plan.env.is_empty() {
                println!(
                    "env_keys: {}",
                    plan.env.keys().cloned().collect::<Vec<_>>().join(",")
                );
            }
            if !plan.env_remove.is_empty() {
                println!("env_removed: {}", plan.env_remove.join(","));
            }
        }
    }
    Ok(())
}
