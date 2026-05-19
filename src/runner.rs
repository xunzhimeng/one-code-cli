use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use chrono::Utc;
use serde::Serialize;
use wait_timeout::ChildExt;

use crate::backend::{self, CommandPlan, TemplateContext};
use crate::cli::{CommonArgs, OutputMode, RunArgs, SessionResumeArgs};
use crate::config::{self, EffectiveConfig, EnvMode, Profile, ProxyConfig};
use crate::documents::{self, RunPaths};
use crate::error::{OccError, OccResult};
use crate::i18n;
use crate::ids;
use crate::output::{
    self, print_batch_response, print_run_response, BatchResponse, BatchRunError, ErrorBody,
    RunResponse,
};

use crate::run_record::{self, RunRecord};
use crate::session::{self, SessionRecord};
use colored::Colorize;

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

#[derive(Debug)]
struct ResolvedRunSettings {
    model: Option<String>,
    model_source: String,
    effort: Option<String>,
    effort_source: String,
}

struct StreamReader {
    name: &'static str,
    handle: Option<JoinHandle<std::io::Result<Vec<u8>>>>,
}

struct DryRunOptions<'a> {
    model_source: &'a str,
    effort_source: &'a str,
    timeout: Option<&'a str>,
    stream: bool,
    output: OutputMode,
}

#[derive(Debug, Serialize)]
struct BatchDryRunResponse {
    success: bool,
    batch_id: String,
    runs: Vec<AgentDryRun>,
}

#[derive(Debug, Serialize)]
struct AgentDryRun {
    agent: String,
    context: TemplateContext,
    model_source: String,
    effort_source: String,
    command: DryCommandOwned,
}

#[derive(Debug, Serialize)]
struct DryCommandOwned {
    executable: PathBuf,
    args: Vec<String>,
    cwd: PathBuf,
    env_keys: Vec<String>,
    env_mode: EnvMode,
    env_allowlist: Vec<String>,
    env_removed: Vec<String>,
    prompt_via_stdin: bool,
    prompt_file: Option<PathBuf>,
    prompt_transport: crate::config::PromptVia,
    timeout: Option<String>,
    stream: bool,
}

pub fn run(config_arg: Option<&PathBuf>, args: RunArgs) -> OccResult<()> {
    if !args.agents.is_empty() {
        let output_mode = args.output;
        if args.dry_run {
            let response = execute_batch_dry_run(config_arg, args)?;
            print_batch_dry_run_response(output_mode, &response)?;
        } else {
            let response = execute_batch(config_arg, args)?;
            print_batch_response(output_mode, &response)?;
        }
        return Ok(());
    }
    if let Some(execution) = execute_run(config_arg, args)? {
        print_run_response(execution.output_mode, &execution.body)?;
    }
    Ok(())
}

pub fn resume_session(config_arg: Option<&PathBuf>, args: SessionResumeArgs) -> OccResult<()> {
    let run_args = RunArgs {
        common: CommonArgs {
            profile: None,
            backend: None,
            model: args.model,
            effort: args.effort,
            cwd: args.cwd,
            prompt: args.prompt,
            prompt_file: args.prompt_file,
            stdin: args.stdin,
            session: Some(args.session_id),
            resume: true,
            doc_root: args.doc_root,
            timeout: None,
            dry_run: args.dry_run,
            child_args: args.child_args,
        },
        agents: Vec::new(),
        stream: args.stream,
        interactive: false,
        non_interactive: true,
        output: args.output,
    };
    run(config_arg, run_args)
}

#[derive(Debug)]
pub struct RunExecution {
    pub body: RunResponse,
    pub output_mode: OutputMode,
    pub stdout: String,
    pub stderr: String,
}

pub fn run_once(config_arg: Option<&PathBuf>, args: RunArgs) -> OccResult<Option<RunExecution>> {
    execute_run(config_arg, args)
}

fn execute_run(config_arg: Option<&PathBuf>, args: RunArgs) -> OccResult<Option<RunExecution>> {
    execute_run_with_prompt(config_arg, args, None, None)
}

fn execute_batch(config_arg: Option<&PathBuf>, args: RunArgs) -> OccResult<BatchResponse> {
    validate_batch_args(&args)?;
    let prompt = read_prompt(&args.prompt, args.prompt_file.as_ref(), args.stdin, false)?;
    let batch_id = ids::batch_id();
    let mut handles = Vec::new();

    for agent in &args.agents {
        let run_args = batch_agent_args(&args, agent, OutputMode::Path);
        let config_arg = config_arg.cloned();
        let prompt = prompt.clone();
        let stream_prefix = args.stream.then(|| agent.clone());
        let agent = agent.clone();
        handles.push((
            agent.clone(),
            thread::spawn(move || {
                execute_run_with_prompt(config_arg.as_ref(), run_args, Some(prompt), stream_prefix)
            }),
        ));
    }

    let mut runs = Vec::new();
    let mut errors = Vec::new();
    for (agent, handle) in handles {
        match handle.join() {
            Ok(Ok(Some(execution))) => runs.push(execution.body),
            Ok(Ok(None)) => {}
            Ok(Err(error)) => errors.push(BatchRunError {
                agent,
                code: error.code().to_string(),
                message: error.message().to_string(),
            }),
            Err(_) => errors.push(BatchRunError {
                agent,
                code: "batch_worker_failed".to_string(),
                message: "The agent worker thread panicked.".to_string(),
            }),
        }
    }

    let success = errors.is_empty() && runs.iter().all(|run| run.success);
    Ok(BatchResponse {
        success,
        batch_id,
        runs,
        errors,
    })
}

fn execute_batch_dry_run(
    config_arg: Option<&PathBuf>,
    args: RunArgs,
) -> OccResult<BatchDryRunResponse> {
    validate_batch_args(&args)?;
    let prompt = read_prompt(&args.prompt, args.prompt_file.as_ref(), args.stdin, false)?;
    let batch_id = ids::batch_id();
    let mut runs = Vec::new();

    for agent in &args.agents {
        let run_args = batch_agent_args(&args, agent, OutputMode::Json);
        runs.push(build_agent_dry_run(
            config_arg,
            run_args,
            prompt.clone(),
            args.stream,
        )?);
    }

    Ok(BatchDryRunResponse {
        success: true,
        batch_id,
        runs,
    })
}

fn batch_agent_args(args: &RunArgs, agent: &str, output: OutputMode) -> RunArgs {
    let mut run_args = args.clone();
    run_args.common.profile = Some(agent.to_string());
    run_args.common.backend = None;
    run_args.common.prompt = None;
    run_args.common.prompt_file = None;
    run_args.common.stdin = false;
    run_args.common.session = None;
    run_args.common.resume = false;
    run_args.agents.clear();
    run_args.interactive = false;
    run_args.non_interactive = true;
    run_args.output = output;
    run_args
}

fn validate_batch_args(args: &RunArgs) -> OccResult<()> {
    if args.interactive {
        return Err(OccError::new(
            "invalid_argument",
            "--agents cannot be used with --interactive.",
        ));
    }
    if args.common.backend.is_some() {
        return Err(OccError::new(
            "invalid_argument",
            "--agents selects exact occ agents and cannot be combined with --cli.",
        ));
    }
    if args.common.session.is_some() || args.common.resume {
        return Err(OccError::new(
            "invalid_argument",
            "--agents cannot be combined with --session or --resume.",
        ));
    }
    Ok(())
}

fn build_agent_dry_run(
    config_arg: Option<&PathBuf>,
    args: RunArgs,
    prompt: PromptData,
    stream: bool,
) -> OccResult<AgentDryRun> {
    let cwd = resolve_cwd(args.cwd.as_ref())?;
    let config = config::load(config_arg, &cwd)?;
    let doc_root = config.resolved_doc_root(&cwd, args.doc_root.as_ref());
    let profile = resolve_profile_for_run(&config, &args, None)?;
    let backend_spec = backend::require(&profile.backend)?;
    let resolved = resolve_run_settings(&args, None, &profile, &cwd);
    let run_id = ids::run_id();
    let paths = RunPaths::new(&doc_root, &run_id);
    let backend_session_id = if backend_spec.session_id_args.is_empty() {
        None
    } else {
        Some(ids::backend_session_id())
    };
    let context = TemplateContext {
        profile: profile.name.clone(),
        backend: profile.backend.clone(),
        model: resolved.model.clone(),
        effort: resolved.effort.clone(),
        cwd: cwd.clone(),
        prompt: prompt.text.clone(),
        prompt_file: prompt.file,
        prompt_indirection_file: prompt.text.as_ref().map(|_| paths.prompt_md.clone()),
        config_dir: profile.config_dir.clone(),
        session_id: ids::session_id(),
        backend_session_id,
        run_id,
        doc_root,
    };
    let mut plan = backend::build_command_plan(&profile, &context, false, false, &args.child_args)?;
    apply_proxy_config(&config.proxy, &mut plan);
    let timeout_value = resolve_timeout_value(&args, &profile, &config);

    Ok(AgentDryRun {
        agent: profile.name,
        context,
        model_source: resolved.model_source,
        effort_source: resolved.effort_source,
        command: DryCommandOwned {
            executable: plan.executable,
            args: plan.args,
            cwd: plan.cwd,
            env_keys: plan.env.keys().cloned().collect(),
            env_mode: plan.env_mode,
            env_allowlist: plan.env_allowlist,
            env_removed: plan.env_remove,
            prompt_via_stdin: plan.prompt_stdin.is_some(),
            prompt_file: plan.prompt_file,
            prompt_transport: plan.prompt_transport,
            timeout: timeout_value,
            stream,
        },
    })
}

fn print_batch_dry_run_response(mode: OutputMode, response: &BatchDryRunResponse) -> OccResult<()> {
    match mode {
        OutputMode::Json => {
            let text = serde_json::to_string_pretty(response).map_err(|error| {
                OccError::new(
                    "serialization_failed",
                    format!("Failed to serialize batch dry-run JSON: {}", error),
                )
            })?;
            println!("{}", output::display_text(&text));
        }
        OutputMode::Path => {
            for run in &response.runs {
                println!(
                    "{}\t{}",
                    run.agent,
                    output::display_path(&run.command.executable)
                );
            }
        }
        OutputMode::Text => {
            println!(
                "{} {}",
                "success:".bold(),
                response.success.to_string().green()
            );
            println!("{} {}", "batch_id:".bold(), response.batch_id);
            let mut table = output::Table::new(&["AGENT", "CLI", "MODEL", "COMMAND"]);
            for run in &response.runs {
                table.add_row(vec![
                    run.agent.clone(),
                    run.context.backend.clone(),
                    run.context.model.clone().unwrap_or_else(|| "-".to_string()),
                    format!(
                        "{} {}",
                        output::display_path(&run.command.executable),
                        run.command.args.join(" ")
                    ),
                ]);
            }
            table.print();
        }
    }
    Ok(())
}

fn execute_run_with_prompt(
    config_arg: Option<&PathBuf>,
    args: RunArgs,
    prompt_override: Option<PromptData>,
    stream_prefix: Option<String>,
) -> OccResult<Option<RunExecution>> {
    if args.interactive && args.non_interactive {
        return Err(OccError::new(
            "invalid_argument",
            "--interactive and --non-interactive cannot be used together.",
        ));
    }
    let interactive = resolve_interactive_mode(&args);

    let mut cwd = resolve_cwd(args.cwd.as_ref())?;
    let mut config = config::load(config_arg, &cwd)?;
    let mut doc_root = config.resolved_doc_root(&cwd, args.doc_root.as_ref());
    let prompt = if let Some(prompt) = prompt_override {
        prompt
    } else {
        read_prompt(
            &args.prompt,
            args.prompt_file.as_ref(),
            args.stdin,
            interactive,
        )?
    };

    if interactive && !args.interactive && prompt.text.is_none() {
        eprintln!(
            "Entering interactive mode (no prompt provided). \
             Use --non-interactive with --prompt/--prompt-file/--stdin for scripted runs."
        );
    }

    let mut existing_session = if args.resume {
        if let Some(session_id) = &args.session {
            Some(session::load_by_id(&doc_root, session_id)?)
        } else {
            None
        }
    } else if let Some(session_id) = &args.session {
        Some(session::load_by_id(&doc_root, session_id)?)
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
            format!("Agent '{}' does not support native resume.", profile.name),
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
                    "No latest session was found for agent '{}' and cwd '{}'.",
                    profile.name,
                    output::display_path(&cwd)
                ),
            )
        })?;
        existing_session = Some(session::load_by_id(&doc_root, &latest.session_id)?);
    }

    let resolved = resolve_run_settings(&args, existing_session.as_ref(), &profile, &cwd);
    let run_id = ids::run_id();
    let paths = RunPaths::new(&doc_root, &run_id);
    let now = Utc::now();
    let mut session_record = existing_session.unwrap_or_else(|| {
        SessionRecord::new(
            args.session.clone().unwrap_or_else(ids::session_id),
            profile.name.clone(),
            profile.backend.clone(),
            cwd.clone(),
            resolved.model.clone(),
            resolved.effort.clone(),
            now,
        )
    });
    session_record.profile = profile.name.clone();
    session_record.backend = profile.backend.clone();
    session_record.cwd = cwd.clone();
    session_record.model = resolved.model.clone();
    session_record.effort = resolved.effort.clone();
    if !args.resume
        && session_record.backend_session_id.is_none()
        && !backend_spec.session_id_args.is_empty()
    {
        session_record.backend_session_id = Some(ids::backend_session_id());
    }

    let context = TemplateContext {
        profile: profile.name.clone(),
        backend: profile.backend.clone(),
        model: resolved.model.clone(),
        effort: resolved.effort.clone(),
        cwd: cwd.clone(),
        prompt: prompt.text.clone(),
        prompt_file: prompt.file.clone(),
        prompt_indirection_file: prompt.text.as_ref().map(|_| paths.prompt_md.clone()),
        config_dir: profile.config_dir.clone(),
        session_id: session_record.session_id.clone(),
        backend_session_id: session_record.backend_session_id.clone(),
        run_id: run_id.clone(),
        doc_root: doc_root.clone(),
    };
    let mut plan = backend::build_command_plan(
        &profile,
        &context,
        interactive,
        args.resume,
        &args.child_args,
    )?;
    apply_proxy_config(&config.proxy, &mut plan);
    let timeout_value = resolve_timeout_value(&args, &profile, &config);

    if args.dry_run {
        print_dry_run(
            &profile,
            &context,
            &plan,
            DryRunOptions {
                model_source: &resolved.model_source,
                effort_source: &resolved.effort_source,
                timeout: timeout_value.as_deref(),
                stream: args.stream,
                output: args.output,
            },
        )?;
        return Ok(None);
    }

    ensure_executable(&plan.executable)?;
    fs::create_dir_all(&doc_root).map_err(|error| {
        OccError::io(
            "doc_root_not_writable",
            format!("Failed to create '{}'", output::display_path(&doc_root)),
            error,
        )
    })?;

    paths.create_dirs()?;
    fs::write(&paths.prompt_md, prompt.text.as_deref().unwrap_or(""))
        .map_err(|error| write_prompt_error(&paths.prompt_md, error))?;
    let started_at = Utc::now();
    let timeout = parse_timeout(timeout_value.as_deref())?;
    let child = if interactive {
        execute_interactive(&plan, timeout)?
    } else {
        execute_non_interactive(
            &plan,
            timeout,
            &paths.stdout_log,
            &paths.stderr_log,
            args.stream,
            stream_prefix.as_deref(),
        )?
    };
    let finished_at = Utc::now();
    let timed_out = child.timed_out;
    let success = !timed_out && child.exit_code == Some(0);

    let record = RunRecord {
        run_id: run_id.clone(),
        session_id: session_record.session_id.clone(),
        profile: profile.name.clone(),
        backend: profile.backend.clone(),
        model: resolved.model.clone(),
        model_source: resolved.model_source.clone(),
        effort: resolved.effort.clone(),
        effort_source: resolved.effort_source.clone(),
        cwd: cwd.clone(),
        prompt_source: prompt.source.clone(),
        interactive,
        timeout: timeout_value.clone(),
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
    session::save(&session_record)?;
    session::append_run(&session_record.session_id, &run_id, started_at)?;

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
        model: resolved.model,
        model_source: resolved.model_source,
        effort: resolved.effort,
        effort_source: resolved.effort_source,
        cwd,
        result_path: paths.result_md,
        metadata_path: paths.run_toml,
        exit_code: child.exit_code,
        error,
    };
    Ok(Some(RunExecution {
        body: response,
        output_mode: args.output,
        stdout: child.stdout,
        stderr: child.stderr,
    }))
}

fn resolve_interactive_mode(args: &RunArgs) -> bool {
    if args.interactive {
        return true;
    }
    if args.non_interactive {
        return false;
    }
    args.prompt.is_none() && args.prompt_file.is_none() && !args.stdin
}

fn resolve_profile_for_run(
    config: &EffectiveConfig,
    args: &RunArgs,
    session: Option<&SessionRecord>,
) -> OccResult<Profile> {
    if let Some(session) = session {
        return resolve_profile_for_existing_session(config, args, session);
    }
    config.resolve_profile(args.profile.as_deref(), args.backend.as_deref())
}

fn resolve_profile_for_existing_session(
    config: &EffectiveConfig,
    args: &RunArgs,
    session: &SessionRecord,
) -> OccResult<Profile> {
    let session_profile = config.resolve_profile(Some(&session.profile), None)?;
    if let Some(requested_profile) = args.profile.as_deref() {
        let requested = config.resolve_profile(Some(requested_profile), None)?;
        if requested.name != session_profile.name || requested.backend != session_profile.backend {
            return Err(session_agent_mismatch(
                session,
                format!("agent '{}'", requested.name),
            ));
        }
    }
    if let Some(requested_backend) = args.backend.as_deref() {
        let requested_backend = resolve_backend_alias(config, requested_backend);
        if requested_backend != session.backend {
            return Err(session_agent_mismatch(
                session,
                format!("cli '{}'", requested_backend),
            ));
        }
    }
    Ok(session_profile)
}

fn resolve_backend_alias<'a>(config: &'a EffectiveConfig, backend: &'a str) -> &'a str {
    config
        .backend_aliases
        .get(backend)
        .map(String::as_str)
        .unwrap_or(backend)
}

fn session_agent_mismatch(session: &SessionRecord, requested: String) -> OccError {
    OccError::new(
        "session_agent_mismatch",
        format!(
            "Session '{}' belongs to agent '{}' (cli '{}') and cannot be run with {}.",
            session.session_id, session.profile, session.backend, requested
        ),
    )
}

fn resolve_run_settings(
    args: &RunArgs,
    session: Option<&SessionRecord>,
    profile: &Profile,
    cwd: &Path,
) -> ResolvedRunSettings {
    let (mut model, mut model_source) = resolve_model(args, session, profile);
    let (mut effort, mut effort_source) = resolve_effort(args, session, profile);

    if model.is_none() || effort.is_none() {
        let config_dir = profile
            .config_dir
            .as_ref()
            .map(|path| backend::resolve_config_dir(cwd, path));
        if let Some(defaults) = detected_cli_defaults(&profile.backend, config_dir.as_deref()) {
            if model.is_none() {
                if let Some(default_model) = defaults.model {
                    model = Some(default_model);
                    model_source = "cli-config".to_string();
                }
            }
            if effort.is_none() {
                if let Some(default_effort) = defaults.effort {
                    effort = Some(default_effort);
                    effort_source = "cli-config".to_string();
                }
            }
        }
    }

    ResolvedRunSettings {
        model,
        model_source,
        effort,
        effort_source,
    }
}

fn resolve_model(
    args: &RunArgs,
    session: Option<&SessionRecord>,
    profile: &Profile,
) -> (Option<String>, String) {
    if let Some(model) = &args.model {
        return (Some(model.clone()), "cli-arg".to_string());
    }
    if let Some(model) = session.and_then(|session| session.model.as_ref()) {
        return (Some(model.clone()), "session".to_string());
    }
    if let Some(model) = &profile.model {
        return (Some(model.clone()), "agent".to_string());
    }
    (None, "none".to_string())
}

fn resolve_effort(
    args: &RunArgs,
    session: Option<&SessionRecord>,
    profile: &Profile,
) -> (Option<String>, String) {
    if let Some(effort) = &args.effort {
        return (Some(effort.clone()), "cli-arg".to_string());
    }
    if let Some(effort) = session.and_then(|session| session.effort.as_ref()) {
        return (Some(effort.clone()), "session".to_string());
    }
    if let Some(effort) = &profile.effort {
        return (Some(effort.clone()), "agent".to_string());
    }
    (None, "none".to_string())
}

fn detected_cli_defaults(
    backend: &str,
    config_dir: Option<&Path>,
) -> Option<crate::cli_defaults::DetectedCli> {
    crate::cli_defaults::detect_for_cli(backend, config_dir)
}

fn resolve_cwd(cwd: Option<&PathBuf>) -> OccResult<PathBuf> {
    let path = cwd
        .cloned()
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    if !path.exists() {
        return Err(OccError::new(
            "cwd_not_found",
            format!(
                "Working directory '{}' was not found.",
                output::display_path(&path)
            ),
        ));
    }
    let metadata = fs::metadata(&path).map_err(|error| {
        OccError::io(
            "cwd_not_found",
            format!("Failed to inspect '{}'", output::display_path(&path)),
            error,
        )
    })?;
    if !metadata.is_dir() {
        return Err(OccError::new(
            "cwd_not_found",
            format!(
                "Working directory '{}' is not a directory.",
                output::display_path(&path)
            ),
        ));
    }
    path.canonicalize().map_err(|error| {
        OccError::io(
            "cwd_not_found",
            format!("Failed to canonicalize '{}'", output::display_path(&path)),
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
                format!(
                    "Failed to read prompt file '{}'",
                    output::display_path(path)
                ),
                error,
            )
        })?;
        return Ok(PromptData {
            text: Some(text),
            source: format!("prompt-file:{}", output::display_path(path)),
            file: Some(path.clone()),
        });
    }

    if stdin {
        let (tx, rx) = std::sync::mpsc::channel();
        thread::spawn(move || {
            let mut text = String::new();
            let result = io::stdin().read_to_string(&mut text);
            let _ = tx.send(result.map(|_| text));
        });
        let timeout = Duration::from_secs(30);
        let text = rx
            .recv_timeout(timeout)
            .map_err(|_| {
                OccError::new(
                    "stdin_timeout",
                    format!(
                        "Timed out after {}s waiting for stdin input. \
                     Ensure the pipe sends EOF (e.g. echo prompt | occ run --stdin ...).",
                        timeout.as_secs()
                    ),
                )
            })?
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
    stdout_log: &Path,
    stderr_log: &Path,
    stream: bool,
    stream_prefix: Option<&str>,
) -> OccResult<ChildResult> {
    let stdout_file = open_stream_log(stdout_log)?;
    let stderr_file = open_stream_log(stderr_log)?;
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
            format!(
                "Failed to spawn '{}'",
                output::display_path(&plan.executable)
            ),
            error,
        )
    })?;

    let stdout_reader = spawn_stream_reader(
        "stdout",
        child.stdout.take(),
        stdout_file,
        stream,
        stream_prefix.map(str::to_string),
    );
    let stderr_reader = spawn_stream_reader(
        "stderr",
        child.stderr.take(),
        stderr_file,
        stream,
        stream_prefix.map(str::to_string),
    );

    if let Some(input) = &plan.prompt_stdin {
        if let Some(mut stdin) = child.stdin.take() {
            write_child_stdin(&mut stdin, input)?;
        }
    }

    let (exit_code, timed_out) = if let Some(timeout) = timeout {
        match child.wait_timeout(timeout).map_err(|error| {
            OccError::io(
                "child_process_failed",
                "Failed while waiting for child process",
                error,
            )
        })? {
            Some(status) => (status.code(), false),
            None => {
                terminate_child_tree(&mut child);
                let _ = child.wait();
                (None, true)
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
        (status.code(), false)
    };

    let stdout = join_stream_reader(stdout_reader)?;
    let stderr = join_stream_reader(stderr_reader)?;
    Ok(ChildResult {
        stdout,
        stderr,
        exit_code,
        timed_out,
    })
}

fn write_child_stdin(stdin: &mut impl Write, input: &str) -> OccResult<()> {
    match stdin.write_all(input.as_bytes()) {
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::BrokenPipe => return Ok(()),
        Err(error) => {
            return Err(OccError::io(
                "child_process_failed",
                "Failed to write prompt to child stdin",
                error,
            ));
        }
    }
    match stdin.flush() {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(OccError::io(
            "child_process_failed",
            "Failed to flush prompt to child stdin",
            error,
        )),
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
            format!(
                "Failed to spawn '{}'",
                output::display_path(&plan.executable)
            ),
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
                terminate_child_tree(&mut child);
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

fn open_stream_log(path: &Path) -> OccResult<File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            OccError::io(
                "doc_root_not_writable",
                format!("Failed to create '{}'", output::display_path(parent)),
                error,
            )
        })?;
    }
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(|error| {
            OccError::io(
                "doc_root_not_writable",
                format!("Failed to open '{}'", output::display_path(path)),
                error,
            )
        })
}

fn spawn_stream_reader<R>(
    name: &'static str,
    pipe: Option<R>,
    mut file: File,
    stream: bool,
    stream_prefix: Option<String>,
) -> StreamReader
where
    R: Read + Send + 'static,
{
    let handle = pipe.map(|mut pipe| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let mut chunk = [0_u8; 8192];
            let mut prefixed_mirror = stream_prefix.map(LiveStreamMirror::new);
            loop {
                let count = pipe.read(&mut chunk)?;
                if count == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..count]);
                file.write_all(&chunk[..count])?;
                file.flush()?;
                if stream {
                    if let Some(mirror) = prefixed_mirror.as_mut() {
                        mirror.write_chunk(&chunk[..count])?;
                    } else {
                        mirror_stream_chunk(&chunk[..count]);
                    }
                }
            }
            if let Some(mirror) = prefixed_mirror.as_mut() {
                mirror.finish()?;
            }
            Ok(bytes)
        })
    });
    StreamReader { name, handle }
}

fn join_stream_reader(reader: StreamReader) -> OccResult<String> {
    let Some(handle) = reader.handle else {
        return Ok(String::new());
    };
    let bytes = handle.join().map_err(|_| {
        OccError::new(
            "child_process_failed",
            format!("Failed to join child {} reader.", reader.name),
        )
    })?;
    bytes
        .map(|bytes| decode_child_output(&bytes))
        .map_err(|error| {
            OccError::io(
                "child_process_failed",
                format!("Failed to capture child {}", reader.name),
                error,
            )
        })
}

#[cfg(windows)]
fn terminate_child_tree(child: &mut Child) {
    let pid = child.id().to_string();
    let _ = Command::new("taskkill")
        .args(["/PID", pid.as_str(), "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
}

#[cfg(not(windows))]
fn terminate_child_tree(child: &mut Child) {
    let _ = child.kill();
}

fn decode_child_output(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn mirror_stream_chunk(chunk: &[u8]) {
    let mut stderr = io::stderr().lock();
    let result = write_stream_chunk(&mut stderr, chunk);
    let _ = result;
    let _ = stderr.flush();
}

struct LiveStreamMirror {
    prefix: String,
    pending: Vec<u8>,
}

impl LiveStreamMirror {
    fn new(prefix: String) -> Self {
        Self {
            prefix,
            pending: Vec::new(),
        }
    }

    fn write_chunk(&mut self, chunk: &[u8]) -> io::Result<()> {
        for byte in chunk {
            if *byte == b'\n' {
                self.flush_line()?;
            } else {
                self.pending.push(*byte);
            }
        }
        Ok(())
    }

    fn finish(&mut self) -> io::Result<()> {
        if !self.pending.is_empty() {
            self.flush_line()?;
        }
        Ok(())
    }

    fn flush_line(&mut self) -> io::Result<()> {
        let line = String::from_utf8_lossy(&self.pending);
        if let Some(cleaned) = clean_live_output_line(&line) {
            let mut stderr = io::stderr().lock();
            writeln!(stderr, "[{}] {}", self.prefix, cleaned)?;
            stderr.flush()?;
        }
        self.pending.clear();
        Ok(())
    }
}

fn clean_live_output_line(line: &str) -> Option<String> {
    let cleaned = strip_ansi_sequences(line).replace('\r', "");
    let cleaned = cleaned.trim();
    if cleaned.is_empty() || cleaned.chars().all(char::is_control) {
        None
    } else {
        Some(cleaned.to_string())
    }
}

fn strip_ansi_sequences(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\x1b' {
            output.push(ch);
            continue;
        }
        if !matches!(chars.peek(), Some('[' | ']' | '(' | ')')) {
            continue;
        }
        let introducer = chars.next();
        while let Some(next) = chars.next() {
            if matches!(introducer, Some(']')) && next == '\x07' {
                break;
            }
            if matches!(introducer, Some(']'))
                && next == '\x1b'
                && matches!(chars.peek(), Some('\\'))
            {
                let _ = chars.next();
                break;
            }
            if matches!(introducer, Some('[')) && ('@'..='~').contains(&next) {
                break;
            }
            if matches!(introducer, Some('(' | ')')) {
                break;
            }
        }
    }
    output
}

#[cfg(windows)]
fn write_stream_chunk<W: Write>(writer: &mut W, chunk: &[u8]) -> io::Result<()> {
    writer.write_all(String::from_utf8_lossy(chunk).as_bytes())
}

#[cfg(not(windows))]
fn write_stream_chunk<W: Write>(writer: &mut W, chunk: &[u8]) -> io::Result<()> {
    writer.write_all(chunk)
}

fn write_prompt_error(path: &Path, error: std::io::Error) -> OccError {
    OccError::io(
        "doc_root_not_writable",
        format!("Failed to write '{}'", output::display_path(path)),
        error,
    )
}

fn ensure_executable(executable: &Path) -> OccResult<()> {
    if executable.is_absolute() || executable.components().count() > 1 {
        if executable.exists() {
            return Ok(());
        }
        return Err(OccError::new(
            "executable_not_found",
            format!(
                "Executable '{}' was not found.",
                output::display_path(executable)
            ),
        ));
    }
    which::which(executable).map(|_| ()).map_err(|_| {
        OccError::new(
            "executable_not_found",
            format!(
                "Executable '{}' was not found in PATH.",
                output::display_path(executable)
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
    if plan.env_mode == EnvMode::Strict {
        command.env_clear();
        apply_allowed_parent_env(command, plan);
    } else {
        for key in &plan.env_remove {
            command.env_remove(key);
        }
    }
    apply_utf8_env_defaults(command, plan);
    command.envs(&plan.env);
}

fn apply_allowed_parent_env(command: &mut Command, plan: &CommandPlan) {
    for (key, value) in env::vars_os() {
        let key_text = key.to_string_lossy();
        if parent_env_allowed(plan, &key_text) && !plan_contains_env_key(plan, &key_text) {
            command.env(key, value);
        }
    }
}

fn parent_env_allowed(plan: &CommandPlan, key: &str) -> bool {
    default_strict_env_allowlist()
        .iter()
        .any(|allowed| env_key_eq(allowed, key))
        || plan
            .env_allowlist
            .iter()
            .any(|allowed| env_key_eq(allowed, key))
}

fn apply_utf8_env_defaults(command: &mut Command, plan: &CommandPlan) {
    for &(key, value) in UTF8_ENV_DEFAULTS {
        if plan_contains_env_key(plan, key) {
            continue;
        }
        if plan.env_mode == EnvMode::Strict {
            if !allowed_parent_env_present(plan, key) {
                command.env(key, value);
            }
        } else if env::var_os(key).is_none() {
            command.env(key, value);
        }
    }
}

fn allowed_parent_env_present(plan: &CommandPlan, key: &str) -> bool {
    parent_env_allowed(plan, key) && env::var_os(key).is_some()
}

fn plan_contains_env_key(plan: &CommandPlan, key: &str) -> bool {
    plan.env.keys().any(|value| env_key_eq(value, key))
        || plan.env_remove.iter().any(|value| env_key_eq(value, key))
}

#[cfg(windows)]
fn env_key_eq(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

#[cfg(not(windows))]
fn env_key_eq(left: &str, right: &str) -> bool {
    left == right
}

const UTF8_ENV_DEFAULTS: &[(&str, &str)] = &[
    ("LANG", "C.UTF-8"),
    ("LC_CTYPE", "C.UTF-8"),
    ("PYTHONUTF8", "1"),
    ("PYTHONIOENCODING", "utf-8"),
];

#[cfg(windows)]
fn default_strict_env_allowlist() -> &'static [&'static str] {
    &[
        "PATH",
        "PATHEXT",
        "SystemRoot",
        "WINDIR",
        "COMSPEC",
        "TEMP",
        "TMP",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramW6432",
    ]
}

#[cfg(not(windows))]
fn default_strict_env_allowlist() -> &'static [&'static str] {
    &["PATH", "TERM", "TMPDIR"]
}

fn resolve_timeout_value(
    args: &RunArgs,
    profile: &Profile,
    config: &EffectiveConfig,
) -> Option<String> {
    args.timeout
        .as_deref()
        .or(profile.default_timeout.as_deref())
        .or(config.timeouts.default_run.as_deref())
        .and_then(normalize_timeout_value)
}

fn normalize_timeout_value(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("none") {
        None
    } else {
        Some(value.to_string())
    }
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
    options: DryRunOptions<'_>,
) -> OccResult<()> {
    #[derive(Serialize)]
    struct DryCommand<'a> {
        executable: &'a Path,
        args: &'a [String],
        cwd: &'a Path,
        env_keys: Vec<&'a String>,
        env_mode: EnvMode,
        env_allowlist: &'a [String],
        env_removed: &'a [String],
        prompt_via_stdin: bool,
        prompt_file: Option<&'a PathBuf>,
        prompt_transport: crate::config::PromptVia,
        timeout: Option<&'a str>,
        stream: bool,
    }

    #[derive(Serialize)]
    struct DryRun<'a> {
        success: bool,
        #[serde(rename = "agent")]
        profile: &'a Profile,
        context: &'a TemplateContext,
        model_source: &'a str,
        effort_source: &'a str,
        command: DryCommand<'a>,
    }

    let dry_run = DryRun {
        success: true,
        profile,
        context,
        model_source: options.model_source,
        effort_source: options.effort_source,
        command: DryCommand {
            executable: &plan.executable,
            args: &plan.args,
            cwd: &plan.cwd,
            env_keys: plan.env.keys().collect(),
            env_mode: plan.env_mode,
            env_allowlist: &plan.env_allowlist,
            env_removed: &plan.env_remove,
            prompt_via_stdin: plan.prompt_stdin.is_some(),
            prompt_file: plan.prompt_file.as_ref(),
            prompt_transport: plan.prompt_transport,
            timeout: options.timeout,
            stream: options.stream,
        },
    };

    match options.output {
        OutputMode::Json => {
            let text = serde_json::to_string_pretty(&dry_run).map_err(|error| {
                OccError::new(
                    "serialization_failed",
                    format!("Failed to serialize dry-run JSON: {}", error),
                )
            })?;
            println!("{}", output::display_text(&text));
        }
        _ => {
            let t = i18n::t;
            println!();
            println!("  {}", t("dry.title").bold());
            println!("  {}", "─".repeat(40).dimmed());
            println!(
                "  {:<16} {}",
                t("dry.profile").dimmed(),
                profile.name.cyan()
            );
            println!(
                "  {:<16} {}",
                t("dry.backend").dimmed(),
                profile.backend.cyan()
            );
            println!(
                "  {:<16} {}",
                t("dry.model_source").dimmed(),
                options.model_source
            );
            if let Some(model) = &context.model {
                println!("  {:<16} {}", t("dry.model").dimmed(), model);
            }
            println!(
                "  {:<16} {}",
                t("dry.effort_source").dimmed(),
                options.effort_source
            );
            if let Some(effort) = &context.effort {
                println!("  {:<16} {}", t("dry.effort").dimmed(), effort);
            }
            println!(
                "  {:<16} {}",
                t("dry.cwd").dimmed(),
                output::display_path(&plan.cwd)
            );
            println!(
                "  {:<16} {} {}",
                t("dry.command").dimmed(),
                output::display_path(&plan.executable).cyan(),
                plan.args.join(" ").dimmed()
            );
            if !plan.env.is_empty() {
                println!(
                    "  {:<16} {}",
                    t("dry.env_keys").dimmed(),
                    plan.env.keys().cloned().collect::<Vec<_>>().join(", ")
                );
            }
            if !plan.env_remove.is_empty() {
                println!(
                    "  {:<16} {}",
                    t("dry.env_removed").dimmed(),
                    plan.env_remove.join(", ")
                );
            }
            println!(
                "  {:<16} {:?}",
                t("dry.prompt_transport").dimmed(),
                plan.prompt_transport
            );
            if let Some(prompt_file) = &plan.prompt_file {
                println!(
                    "  {:<16} {}",
                    t("dry.prompt_file").dimmed(),
                    output::display_path(prompt_file)
                );
            }
            println!(
                "  {:<16} {}",
                t("dry.timeout").dimmed(),
                options.timeout.unwrap_or(t("dry.timeout_none"))
            );
            println!(
                "  {:<16} {}",
                t("dry.stream").dimmed(),
                if options.stream {
                    t("dry.stream_on").green().to_string()
                } else {
                    t("dry.stream_off").to_string()
                }
            );
            println!("  {}", "─".repeat(40).dimmed());
            println!();
        }
    }
    Ok(())
}
