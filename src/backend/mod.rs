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
    pub non_interactive_args: &'static [&'static str],
    pub interactive_args: &'static [&'static str],
    pub resume_args: &'static [&'static str],
    pub model_args: &'static [&'static str],
}

#[derive(Debug, Clone, Serialize)]
pub struct TemplateContext {
    pub profile: String,
    pub backend: String,
    pub model: Option<String>,
    pub cwd: PathBuf,
    pub prompt: Option<String>,
    pub prompt_file: Option<PathBuf>,
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
            format!("Backend '{}' was not found.", name),
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

    let mut args = match profile.args_strategy {
        ArgsStrategy::Builtin => builtin_args(profile, backend, context, interactive, resume)?,
        ArgsStrategy::Append => {
            let mut args = builtin_args(profile, backend, context, interactive, resume)?;
            args.extend(render_list(&profile.extra_args, context));
            args
        }
        ArgsStrategy::Override => render_list(&profile.args, context),
    };

    args.extend(child_args.iter().cloned());

    let prompt_via = profile.prompt_via.unwrap_or(backend.default_prompt_via);
    let prompt_stdin = match prompt_via {
        PromptVia::Stdin if context.prompt.is_some() && !interactive => context.prompt.clone(),
        _ => None,
    };

    let mut env = BTreeMap::new();
    for (key, value) in &profile.env {
        env.insert(key.clone(), render(value, context));
    }

    Ok(CommandPlan {
        executable,
        args,
        cwd: context.cwd.clone(),
        env,
        env_remove: Vec::new(),
        prompt_stdin,
        prompt_file: context.prompt_file.clone(),
    })
}

pub fn render_list(values: &[String], context: &TemplateContext) -> Vec<String> {
    values.iter().map(|value| render(value, context)).collect()
}

pub fn render(value: &str, context: &TemplateContext) -> String {
    let mut rendered = value.to_string();
    let replacements = [
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
            context
                .backend_session_id
                .as_deref()
                .unwrap_or(context.session_id.as_str()),
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
) -> OccResult<Vec<String>> {
    if interactive && !backend.supports_interactive {
        return Err(OccError::new(
            "child_process_failed",
            format!(
                "Backend '{}' does not support interactive mode.",
                backend.name
            ),
        ));
    }
    if !interactive && !backend.supports_non_interactive {
        return Err(OccError::new(
            "child_process_failed",
            format!(
                "Backend '{}' does not support non-interactive mode.",
                backend.name
            ),
        ));
    }
    if resume && !backend.supports_resume && profile.resume_args.is_empty() {
        return Err(OccError::new(
            "resume_unsupported",
            format!("Profile '{}' does not support native resume.", profile.name),
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
        args.extend(render_list(&resume_args, context));
    }

    if let Some(model) = &context.model {
        if backend.supports_model {
            for arg in backend.model_args {
                args.push(arg.replace("{model}", model));
            }
        }
    }

    match profile.prompt_via.unwrap_or(backend.default_prompt_via) {
        PromptVia::Arg => {
            if let Some(prompt) = &context.prompt {
                args.push(prompt.clone());
            }
        }
        PromptVia::File => {
            if let Some(prompt_file) = &context.prompt_file {
                args.push(path_to_string(prompt_file));
            } else if let Some(prompt) = &context.prompt {
                args.push(prompt.clone());
            }
        }
        PromptVia::Stdin => {}
    }

    Ok(args)
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
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
        default_prompt_via: PromptVia::Arg,
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
        default_prompt_via: PromptVia::Arg,
        non_interactive_args: &["--yolo", "--skip-trust", "-p"],
        interactive_args: &["--yolo", "--skip-trust"],
        resume_args: &[],
        model_args: &["--model", "{model}"],
    },
];
