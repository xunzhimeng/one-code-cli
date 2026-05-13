use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::{ArgsStrategy, Profile, PromptVia};
use crate::error::{OccError, OccResult};

#[derive(Debug, Clone, Serialize)]
pub struct BackendSpec {
    pub name: &'static str,
    pub default_command: &'static str,
    pub builtin_profile: &'static str,
    pub supports_model: bool,
    pub supports_interactive: bool,
    pub supports_non_interactive: bool,
    pub supports_resume: bool,
    pub default_prompt_via: PromptVia,
    pub prompt_transports: &'static [PromptVia],
    pub file_indirection_template: Option<&'static str>,
    pub non_interactive_args: &'static [&'static str],
    pub interactive_args: &'static [&'static str],
    pub resume_args: &'static [&'static str],
    pub model_args: &'static [&'static str],
}

#[derive(Debug, Clone, Serialize)]
pub struct TemplateContext {
    #[serde(rename = "agent")]
    pub profile: String,
    #[serde(rename = "cli")]
    pub backend: String,
    pub model: Option<String>,
    pub cwd: PathBuf,
    pub prompt: Option<String>,
    pub prompt_file: Option<PathBuf>,
    pub prompt_indirection_file: Option<PathBuf>,
    pub config_dir: Option<PathBuf>,
    pub session_id: String,
    pub backend_session_id: Option<String>,
    pub run_id: String,
    pub doc_root: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandPlan {
    pub executable: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
    pub env_remove: Vec<String>,
    pub prompt_stdin: Option<String>,
    pub prompt_file: Option<PathBuf>,
    pub prompt_transport: PromptVia,
}

pub fn all() -> &'static [BackendSpec] {
    &BACKENDS
}

pub fn get(name: &str) -> Option<&'static BackendSpec> {
    all().iter().find(|backend| backend.name == name)
}

pub fn require(name: &str) -> OccResult<&'static BackendSpec> {
    get(name).ok_or_else(|| {
        OccError::new(
            "backend_not_found",
            format!("CLI '{}' was not found.", name),
        )
    })
}

pub fn build_command_plan(
    profile: &Profile,
    context: &TemplateContext,
    interactive: bool,
    resume: bool,
    child_args: &[String],
) -> OccResult<CommandPlan> {
    let backend = require(&profile.backend)?;
    let executable = profile.path.clone().unwrap_or_else(|| {
        PathBuf::from(
            profile
                .command
                .as_deref()
                .unwrap_or(backend.default_command),
        )
    });

    let prompt_transport = select_prompt_transport(profile, backend, context)?;
    let transport_prompt_file = prompt_file_for_transport(context, prompt_transport)?;
    let mut transport_context = context.clone();
    if matches!(
        prompt_transport,
        PromptVia::File | PromptVia::FileIndirection
    ) {
        transport_context.prompt_file = transport_prompt_file.clone();
    }

    let mut args = match profile.args_strategy {
        ArgsStrategy::Builtin => builtin_args(
            profile,
            backend,
            &transport_context,
            interactive,
            resume,
            prompt_transport,
        )?,
        ArgsStrategy::Append => {
            let mut args = builtin_args(
                profile,
                backend,
                &transport_context,
                interactive,
                resume,
                prompt_transport,
            )?;
            args.extend(render_list(&profile.extra_args, &transport_context));
            args
        }
        ArgsStrategy::Override => {
            if resume {
                ensure_backend_session_id_for_resume(profile, &transport_context, &profile.args)?;
            }
            render_list(&profile.args, &transport_context)
        }
    };

    args.extend(child_args.iter().cloned());

    let prompt_stdin = match prompt_transport {
        PromptVia::Stdin if context.prompt.is_some() && !interactive => context.prompt.clone(),
        _ => None,
    };

    let mut env = BTreeMap::new();
    for (key, value) in &profile.env {
        env.insert(key.clone(), render(value, &transport_context));
    }

    Ok(CommandPlan {
        executable,
        args,
        cwd: context.cwd.clone(),
        env,
        env_remove: Vec::new(),
        prompt_stdin,
        prompt_file: transport_prompt_file,
        prompt_transport,
    })
}

pub fn render_list(values: &[String], context: &TemplateContext) -> Vec<String> {
    values.iter().map(|value| render(value, context)).collect()
}

pub fn render(value: &str, context: &TemplateContext) -> String {
    let mut rendered = value.to_string();
    let replacements = [
        ("{agent_alias}", context.profile.as_str()),
        ("{cli_type}", context.backend.as_str()),
        ("{profile}", context.profile.as_str()),
        ("{backend}", context.backend.as_str()),
        ("{model}", context.model.as_deref().unwrap_or("")),
        ("{cwd}", &path_to_string(&context.cwd)),
        ("{prompt}", context.prompt.as_deref().unwrap_or("")),
        (
            "{prompt_file}",
            &context
                .prompt_file
                .as_ref()
                .map(|path| path_to_string(path))
                .unwrap_or_default(),
        ),
        (
            "{config_dir}",
            &context
                .config_dir
                .as_ref()
                .map(|path| path_to_string(path))
                .unwrap_or_default(),
        ),
        ("{session_id}", context.session_id.as_str()),
        (
            "{backend_session_id}",
            context.backend_session_id.as_deref().unwrap_or(""),
        ),
        ("{run_id}", context.run_id.as_str()),
        ("{doc_root}", &path_to_string(&context.doc_root)),
    ];

    for (needle, replacement) in replacements {
        rendered = rendered.replace(needle, replacement);
    }
    rendered
}

fn builtin_args(
    profile: &Profile,
    backend: &BackendSpec,
    context: &TemplateContext,
    interactive: bool,
    resume: bool,
    prompt_transport: PromptVia,
) -> OccResult<Vec<String>> {
    if interactive && !backend.supports_interactive {
        return Err(OccError::new(
            "child_process_failed",
            format!(
                "CLI '{}' does not support interactive mode.",
                backend.name
            ),
        ));
    }
    if !interactive && !backend.supports_non_interactive {
        return Err(OccError::new(
            "child_process_failed",
            format!(
                "CLI '{}' does not support non-interactive mode.",
                backend.name
            ),
        ));
    }
    if resume && !backend.supports_resume && profile.resume_args.is_empty() {
        return Err(OccError::new(
            "resume_unsupported",
            format!("Agent '{}' does not support native resume.", profile.name),
        ));
    }

    let mut args = Vec::new();
    let mode_args = if interactive {
        if profile.interactive_args.is_empty() {
            backend
                .interactive_args
                .iter()
                .map(|value| (*value).to_string())
                .collect()
        } else {
            render_list(&profile.interactive_args, context)
        }
    } else if profile.non_interactive_args.is_empty() {
        backend
            .non_interactive_args
            .iter()
            .map(|value| (*value).to_string())
            .collect()
    } else {
        render_list(&profile.non_interactive_args, context)
    };
    args.extend(mode_args);

    if resume {
        let resume_args = if profile.resume_args.is_empty() {
            backend
                .resume_args
                .iter()
                .map(|value| (*value).to_string())
                .collect()
        } else {
            profile.resume_args.clone()
        };
        ensure_backend_session_id_for_resume(profile, context, &resume_args)?;
        args.extend(render_list(&resume_args, context));
    }

    if let Some(model) = &context.model {
        if backend.supports_model {
            for arg in backend.model_args {
                args.push(arg.replace("{model}", model));
            }
        }
    }

    match prompt_transport {
        PromptVia::Arg => {
            if let Some(prompt) = &context.prompt {
                let os_limit = os_arg_byte_limit();
                if prompt.len() > os_limit {
                    // Prompt exceeds OS command-line length limit — try file indirection fallback.
                    if let Some(template) = backend.file_indirection_template {
                        eprintln!(
                            "warning: prompt length ({} bytes) exceeds OS argument limit ({} bytes), \
                             falling back to file indirection.",
                            prompt.len(),
                            os_limit
                        );
                        args.push(render(template, context));
                    } else {
                        return Err(OccError::new(
                            "prompt_too_large",
                            format!(
                                "Prompt length ({} bytes) exceeds OS argument limit ({} bytes) \
                                 and CLI '{}' does not support file indirection.",
                                prompt.len(),
                                os_limit,
                                backend.name
                            ),
                        ));
                    }
                } else {
                    args.push(prompt.clone());
                }
            }
        }
        PromptVia::File => {
            if let Some(prompt_file) = &context.prompt_file {
                args.push(path_to_string(prompt_file));
            }
        }
        PromptVia::FileIndirection => {
            if context.prompt.is_some() {
                let template = backend.file_indirection_template.ok_or_else(|| {
                    OccError::new(
                        "prompt_transport_unsupported",
                        format!(
                            "CLI '{}' does not define a file indirection prompt template.",
                            backend.name
                        ),
                    )
                })?;
                args.push(render(template, context));
            }
        }
        PromptVia::ArgOrFileIndirection => unreachable!(),
        PromptVia::Stdin => {}
    }

    Ok(args)
}

fn select_prompt_transport(
    profile: &Profile,
    backend: &BackendSpec,
    context: &TemplateContext,
) -> OccResult<PromptVia> {
    let requested = profile.prompt_via.unwrap_or(backend.default_prompt_via);
    let selected = match requested {
        PromptVia::ArgOrFileIndirection => {
            if should_use_file_indirection(context) {
                PromptVia::FileIndirection
            } else {
                PromptVia::Arg
            }
        }
        other => other,
    };
    if !backend.prompt_transports.contains(&selected) {
        return Err(OccError::new(
            "prompt_transport_unsupported",
            format!(
                "Backend '{}' does not support prompt transport '{:?}'.",
                backend.name, selected
            ),
        ));
    }
    if selected == PromptVia::FileIndirection && backend.file_indirection_template.is_none() {
        return Err(OccError::new(
            "prompt_transport_unsupported",
            format!(
                "Backend '{}' does not define a file indirection prompt template.",
                backend.name
            ),
        ));
    }
    Ok(selected)
}

fn should_use_file_indirection(context: &TemplateContext) -> bool {
    if context.prompt_file.is_some() {
        return true;
    }
    let Some(prompt) = &context.prompt else {
        return false;
    };
    prompt.contains('\n')
        || prompt.contains('\r')
        || prompt.chars().count() > direct_arg_max_chars()
}

fn direct_arg_max_chars() -> usize {
    if cfg!(windows) {
        1800
    } else {
        8000
    }
}

/// Maximum safe byte length for a single CLI argument on the current OS.
/// Windows: CreateProcess has a 32767 char limit, but cmd.exe is limited to 8191.
/// Unix: ARG_MAX is typically 2097152 but individual arg limits are lower (~131072).
fn os_arg_byte_limit() -> usize {
    if cfg!(windows) {
        8191
    } else {
        131_072
    }
}

fn prompt_file_for_transport(
    context: &TemplateContext,
    transport: PromptVia,
) -> OccResult<Option<PathBuf>> {
    match transport {
        PromptVia::File => {
            if context.prompt_file.is_some() {
                Ok(context.prompt_file.clone())
            } else if context.prompt.is_some() {
                context.prompt_indirection_file.clone().map(Some).ok_or_else(|| {
                    OccError::new(
                        "prompt_transport_unsupported",
                        "A prompt file transport was selected but no prompt file path is available.",
                    )
                })
            } else {
                Ok(None)
            }
        }
        PromptVia::FileIndirection => {
            if context.prompt.is_some() {
                context
                    .prompt_indirection_file
                    .clone()
                    .map(Some)
                    .ok_or_else(|| {
                        OccError::new(
                            "prompt_transport_unsupported",
                            "File indirection was selected but no prompt file path is available.",
                        )
                    })
            } else {
                Ok(None)
            }
        }
        _ => Ok(None),
    }
}

fn ensure_backend_session_id_for_resume(
    profile: &Profile,
    context: &TemplateContext,
    args: &[String],
) -> OccResult<()> {
    if context.backend_session_id.is_none()
        && args
            .iter()
            .any(|value| value.contains("{backend_session_id}"))
    {
        return Err(OccError::new(
            "backend_session_missing",
            format!(
                "Session '{}' does not have a native backend session id for profile '{}'. Run a new task or use a profile with explicit resume_args that do not require backend_session_id.",
                context.session_id, profile.name
            ),
        ));
    }
    Ok(())
}

fn path_to_string(path: &Path) -> String {
    let text = path.to_string_lossy();
    let text = if let Some(path) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{}", path)
    } else if let Some(path) = text.strip_prefix(r"\\?\") {
        path.to_string()
    } else {
        text.into_owned()
    };
    text.replace('\\', "/")
}

static BACKENDS: [BackendSpec; 4] = [
    BackendSpec {
        name: "claude",
        default_command: "claude",
        builtin_profile: "claude",
        supports_model: true,
        supports_interactive: true,
        supports_non_interactive: true,
        supports_resume: true,
        default_prompt_via: PromptVia::Stdin,
        prompt_transports: &[PromptVia::Stdin],
        file_indirection_template: None,
        non_interactive_args: &["--print", "--dangerously-skip-permissions"],
        interactive_args: &["--dangerously-skip-permissions"],
        resume_args: &["--resume", "{backend_session_id}"],
        model_args: &["--model", "{model}"],
    },
    BackendSpec {
        name: "codex",
        default_command: "codex",
        builtin_profile: "codex",
        supports_model: true,
        supports_interactive: true,
        supports_non_interactive: true,
        supports_resume: false,
        default_prompt_via: PromptVia::Stdin,
        prompt_transports: &[PromptVia::Stdin],
        file_indirection_template: None,
        non_interactive_args: &[
            "exec",
            "--dangerously-bypass-approvals-and-sandbox",
            "--skip-git-repo-check",
        ],
        interactive_args: &[
            "--dangerously-bypass-approvals-and-sandbox",
            "--skip-git-repo-check",
        ],
        resume_args: &[],
        model_args: &["--model", "{model}"],
    },
    BackendSpec {
        name: "opencode",
        default_command: "opencode",
        builtin_profile: "opencode",
        supports_model: true,
        supports_interactive: true,
        supports_non_interactive: true,
        supports_resume: false,
        default_prompt_via: PromptVia::ArgOrFileIndirection,
        prompt_transports: &[PromptVia::Arg, PromptVia::FileIndirection],
        file_indirection_template: Some("Run the task described in {prompt_file}."),
        non_interactive_args: &["run", "--dangerously-skip-permissions"],
        interactive_args: &[],
        resume_args: &[],
        model_args: &["--model", "{model}"],
    },
    BackendSpec {
        name: "gemini",
        default_command: "gemini",
        builtin_profile: "gemini",
        supports_model: true,
        supports_interactive: true,
        supports_non_interactive: true,
        supports_resume: false,
        default_prompt_via: PromptVia::ArgOrFileIndirection,
        prompt_transports: &[PromptVia::Arg, PromptVia::FileIndirection],
        file_indirection_template: Some("Read and follow the task in {prompt_file}."),
        non_interactive_args: &["--yolo", "--skip-trust", "-p"],
        interactive_args: &["--yolo", "--skip-trust"],
        resume_args: &[],
        model_args: &["--model", "{model}"],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    fn context(backend_session_id: Option<String>) -> TemplateContext {
        TemplateContext {
            profile: "claude".to_string(),
            backend: "claude".to_string(),
            model: None,
            cwd: PathBuf::from("."),
            prompt: Some("hello".to_string()),
            prompt_file: None,
            prompt_indirection_file: Some(PathBuf::from(".occ/runs/run_test/prompt.md")),
            config_dir: None,
            session_id: "sess_test".to_string(),
            backend_session_id,
            run_id: "run_test".to_string(),
            doc_root: PathBuf::from(".occ"),
        }
    }

    fn profile() -> Profile {
        Profile {
            name: "claude".to_string(),
            aliases: Vec::new(),
            backend: "claude".to_string(),
            command: Some("claude".to_string()),
            path: None,
            model: None,
            default_timeout: None,
            config_dir: None,
            env: BTreeMap::new(),
            args_strategy: ArgsStrategy::Builtin,
            args: Vec::new(),
            extra_args: Vec::new(),
            prompt_via: Some(PromptVia::Stdin),
            resume_args: Vec::new(),
            interactive_args: Vec::new(),
            non_interactive_args: Vec::new(),
            builtin: false,
        }
    }

    #[test]
    fn render_backend_session_id_does_not_fallback_to_occ_session_id() {
        assert_eq!(render("{backend_session_id}", &context(None)), "");
    }

    #[test]
    fn resume_requires_backend_session_id_when_selected_args_need_it() {
        let error = build_command_plan(&profile(), &context(None), false, true, &[]).unwrap_err();
        assert_eq!(error.code(), "backend_session_missing");
    }

    #[test]
    fn custom_resume_args_without_backend_session_id_are_allowed() {
        let mut profile = profile();
        profile.resume_args = vec!["--resume-occ".to_string(), "{session_id}".to_string()];
        let plan = build_command_plan(&profile, &context(None), false, true, &[]).unwrap();
        assert!(plan.args.iter().any(|arg| arg == "sess_test"));
    }

    #[test]
    fn gemini_multiline_prompt_uses_file_indirection() {
        let mut profile = profile();
        profile.name = "gemini".to_string();
        profile.backend = "gemini".to_string();
        profile.command = Some("gemini".to_string());
        profile.prompt_via = None;
        let mut context = context(None);
        context.profile = "gemini".to_string();
        context.backend = "gemini".to_string();
        context.prompt = Some("line one\nline two".to_string());

        let plan = build_command_plan(&profile, &context, false, false, &[]).unwrap();

        assert_eq!(plan.prompt_transport, PromptVia::FileIndirection);
        assert!(plan
            .args
            .iter()
            .any(|arg| arg.starts_with("Read and follow the task in ")));
        assert!(!plan
            .args
            .iter()
            .any(|arg| arg.contains("line one\nline two")));
    }

    #[test]
    fn opencode_short_prompt_uses_direct_arg() {
        let mut profile = profile();
        profile.name = "opencode".to_string();
        profile.backend = "opencode".to_string();
        profile.command = Some("opencode".to_string());
        profile.prompt_via = None;
        let mut context = context(None);
        context.profile = "opencode".to_string();
        context.backend = "opencode".to_string();
        context.prompt = Some("short task".to_string());

        let plan = build_command_plan(&profile, &context, false, false, &[]).unwrap();

        assert_eq!(plan.prompt_transport, PromptVia::Arg);
        assert!(plan.args.iter().any(|arg| arg == "short task"));
    }
}
