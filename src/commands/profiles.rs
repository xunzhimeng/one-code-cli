use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::backend::{self, TemplateContext};
use crate::cli::ProfileAddArgs;
use crate::config::{self, ArgsStrategy, EnvMode, Profile};
use crate::error::{OccError, OccResult};
use crate::output::{self, Table};

use super::{current_cwd, load_current};

pub fn profiles_list(config_arg: Option<&PathBuf>) -> OccResult<()> {
    let config = load_current(config_arg)?;
    let mut table = Table::new(&[
        "AGENT", "CLI", "MODEL", "EFFORT", "ENV", "SOURCE", "ALIASES",
    ]);
    for profile in config.profiles {
        let source = if profile.builtin { "builtin" } else { "config" };
        table.add_row(vec![
            profile.name,
            profile.backend,
            profile.model.unwrap_or_else(|| "-".to_string()),
            profile.effort.unwrap_or_else(|| "-".to_string()),
            profile.env.len().to_string(),
            source.to_string(),
            format!("aliases={}", profile.aliases.join(",")),
        ]);
    }
    table.print();
    Ok(())
}

pub fn profiles_show(config_arg: Option<&PathBuf>, name: &str) -> OccResult<()> {
    let config = load_current(config_arg)?;
    let profile = config.profile(name).ok_or_else(|| {
        OccError::new(
            "agent_not_found",
            format!("Agent '{}' was not found.", name),
        )
    })?;
    let text = toml::to_string_pretty(profile).map_err(|error| {
        OccError::new(
            "serialization_failed",
            format!("Failed to serialize agent: {}", error),
        )
    })?;
    println!("{}", output::display_text(&text));
    Ok(())
}

pub fn profiles_test(config_arg: Option<&PathBuf>, name: &str) -> OccResult<()> {
    let cwd = current_cwd()?;
    let config = config::load(config_arg, &cwd)?;
    let profile = config.profile(name).ok_or_else(|| {
        OccError::new(
            "agent_not_found",
            format!("Agent '{}' was not found.", name),
        )
    })?;
    let model = profile.model.clone();
    let doc_root = config.resolved_doc_root(&cwd, None);
    let context = TemplateContext {
        profile: profile.name.clone(),
        backend: profile.backend.clone(),
        model,
        effort: profile.effort.clone(),
        cwd: cwd.clone(),
        prompt: Some("test".to_string()),
        prompt_file: None,
        prompt_indirection_file: Some(doc_root.join("runs").join("run_test").join("prompt.md")),
        config_dir: profile.config_dir.clone(),
        session_id: "sess_test".to_string(),
        backend_session_id: None,
        run_id: "run_test".to_string(),
        doc_root,
    };
    let plan = backend::build_command_plan(profile, &context, false, false, &[])?;
    println!("agent: {}", profile.name);
    println!("cli: {}", profile.backend);
    println!(
        "command: {} {}",
        output::display_path(&plan.executable),
        plan.args.join(" ")
    );
    if !plan.env.is_empty() {
        println!(
            "env_keys: {}",
            plan.env.keys().cloned().collect::<Vec<_>>().join(",")
        );
    }
    Ok(())
}

pub fn profiles_add(config_arg: Option<&PathBuf>, args: ProfileAddArgs) -> OccResult<()> {
    backend::require(&args.backend)?;
    let target = target_config_path(config_arg, args.user, args.project)?;
    let mut file = config::read_config_file(&target)?;
    ensure_new_agent_name(&file, &args.name, &args.aliases)?;
    let env = parse_env_vars(&args.env)?;
    let config_dir = if args.inherit_env && args.config_dir.is_none() {
        None
    } else {
        Some(match &args.config_dir {
            Some(path) => path.clone(),
            None => default_agent_config_dir(&target, &args.name)?,
        })
    };
    if let Some(config_dir) = &config_dir {
        ensure_config_dir(config_dir)?;
    }
    let profile = Profile {
        name: args.name.clone(),
        aliases: clean_aliases(args.aliases),
        backend: args.backend.clone(),
        command: args.command,
        path: args.path,
        model: args.model,
        effort: args.effort,
        default_timeout: None,
        config_dir: config_dir.clone(),
        env_mode: if args.inherit_env {
            EnvMode::Inherit
        } else {
            EnvMode::Strict
        },
        env_allowlist: clean_env_allowlist(args.env_allow),
        env,
        args_strategy: ArgsStrategy::Builtin,
        args: Vec::new(),
        extra_args: Vec::new(),
        prompt_via: None,
        resume_args: Vec::new(),
        interactive_args: Vec::new(),
        non_interactive_args: Vec::new(),
        builtin: false,
    };

    file.profiles.push(profile);
    if args.set_default {
        file.default_profile = Some(args.name.clone());
    }
    if args.set_cli_default {
        file.backend_defaults
            .insert(args.backend.clone(), args.name.clone());
    }
    config::write_config_file(&target, &file)?;
    println!("added: {}", args.name);
    println!("config: {}", output::display_path(&target));
    if let Some(config_dir) = &config_dir {
        println!("config_dir: {}", output::display_path(config_dir));
    } else {
        println!("config_dir: default");
    }
    Ok(())
}

fn target_config_path(
    config_arg: Option<&PathBuf>,
    _user: bool,
    project: bool,
) -> OccResult<PathBuf> {
    if let Some(path) = config_arg {
        return Ok(path.clone());
    }
    if project {
        return Ok(config::default_project_config_path(&current_cwd()?));
    }
    config::default_user_config_path()
}

fn default_agent_config_dir(target: &Path, name: &str) -> OccResult<PathBuf> {
    let cwd = current_cwd()?;
    let target = if target.is_absolute() {
        target.to_path_buf()
    } else {
        cwd.join(target)
    };
    let base = target
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| cwd.clone());
    Ok(base
        .join("agents")
        .join(safe_agent_path_segment(name))
        .join("system"))
}

fn ensure_config_dir(path: &Path) -> OccResult<()> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        current_cwd()?.join(path)
    };
    fs::create_dir_all(&path).map_err(|error| {
        OccError::io(
            "config_dir_not_writable",
            format!(
                "Failed to create agent config_dir '{}'.",
                output::display_path(&path)
            ),
            error,
        )
    })
}

fn safe_agent_path_segment(name: &str) -> String {
    let segment: String = name
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect();
    let segment = segment.trim_matches('-').to_string();
    if segment.is_empty() || segment == "." || segment == ".." {
        "agent".to_string()
    } else {
        segment
    }
}

fn ensure_new_agent_name(
    file: &config::ConfigFile,
    name: &str,
    aliases: &[String],
) -> OccResult<()> {
    let aliases = clean_aliases(aliases.to_vec());
    for profile in &file.profiles {
        if profile.name == name || profile.aliases.iter().any(|alias| alias == name) {
            return Err(OccError::new(
                "profile_alias_conflict",
                format!("Agent '{}' already exists.", name),
            ));
        }
        for alias in &aliases {
            if profile.name == *alias || profile.aliases.iter().any(|existing| existing == alias) {
                return Err(OccError::new(
                    "profile_alias_conflict",
                    format!("Agent alias '{}' already exists.", alias),
                ));
            }
        }
    }
    Ok(())
}

fn parse_env_vars(values: &[String]) -> OccResult<BTreeMap<String, String>> {
    let mut env = BTreeMap::new();
    for value in values {
        let Some((key, raw_value)) = value.split_once('=') else {
            return Err(OccError::new(
                "invalid_argument",
                format!("Env value '{}' must use KEY=VALUE.", value),
            ));
        };
        let key = key.trim();
        if key.is_empty() {
            return Err(OccError::new(
                "invalid_argument",
                format!("Env value '{}' has an empty key.", value),
            ));
        }
        env.insert(key.to_string(), raw_value.to_string());
    }
    Ok(env)
}

fn clean_aliases(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn clean_env_allowlist(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}
