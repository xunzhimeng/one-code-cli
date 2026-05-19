use std::path::PathBuf;

use crate::backend;
use crate::error::{OccError, OccResult};
use crate::output::{self, Table};

use super::load_current;

pub fn backends_list(config_arg: Option<&PathBuf>) -> OccResult<()> {
    let config = load_current(config_arg)?;
    let mut table = Table::new(&[
        "CLI",
        "COMMAND",
        "BUILTIN_AGENT",
        "DEFAULT_AGENT",
        "AGENTS",
        "ALIASES",
        "RESUME",
    ]);
    for backend in backend::all() {
        let default_profile = config
            .backend_defaults
            .get(backend.name)
            .map(String::as_str)
            .unwrap_or("");
        let aliases = config
            .backend_aliases
            .iter()
            .filter_map(|(alias, target)| (target == backend.name).then_some(alias.as_str()))
            .collect::<Vec<_>>()
            .join(",");
        let agent_count = config.profiles_for_backend(backend.name).count();
        table.add_row(vec![
            backend.name.to_string(),
            backend.default_command.to_string(),
            backend.builtin_profile.to_string(),
            default_profile.to_string(),
            agent_count.to_string(),
            aliases,
            backend.supports_resume.to_string(),
        ]);
    }
    table.print();
    Ok(())
}

pub fn backends_show(config_arg: Option<&PathBuf>, name: &str) -> OccResult<()> {
    let config = load_current(config_arg)?;
    let backend_name = config
        .backend_aliases
        .get(name)
        .map(String::as_str)
        .unwrap_or(name);
    let backend = backend::require(backend_name)?;
    let value = serde_json::json!({
        "name": backend.name,
        "default_command": backend.default_command,
        "builtin_agent": backend.builtin_profile,
        "builtin_profile": backend.builtin_profile,
        "cli_default_agent": config.backend_defaults.get(backend.name),
        "aliases": config
            .backend_aliases
            .iter()
            .filter_map(|(alias, target)| (target == backend.name).then_some(alias))
            .collect::<Vec<_>>(),
        "supports_model": backend.supports_model,
        "supports_effort": backend.supports_effort,
        "supports_interactive": backend.supports_interactive,
        "supports_non_interactive": backend.supports_non_interactive,
        "supports_resume": backend.supports_resume,
        "default_prompt_via": backend.default_prompt_via,
        "prompt_transports": backend.prompt_transports,
        "prompt_arg": backend.prompt_arg,
        "file_indirection_template": backend.file_indirection_template,
        "non_interactive_args": backend.non_interactive_args,
        "interactive_args": backend.interactive_args,
        "session_id_args": backend.session_id_args,
        "resume_args": backend.resume_args,
    });
    println!(
        "{}",
        output::display_text(&serde_json::to_string_pretty(&value).map_err(|error| {
            OccError::new(
                "serialization_failed",
                format!("Failed to serialize CLI JSON: {}", error),
            )
        })?)
    );
    Ok(())
}
