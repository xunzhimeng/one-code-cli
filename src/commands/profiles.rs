use std::path::PathBuf;

use crate::backend::{self, TemplateContext};
use crate::config;
use crate::error::{OccError, OccResult};
use crate::output::{self, Table};

use super::{current_cwd, load_current};

pub fn profiles_list(config_arg: Option<&PathBuf>) -> OccResult<()> {
    let config = load_current(config_arg)?;
    let mut table = Table::new(&["AGENT", "CLI", "SOURCE", "ALIASES"]);
    for profile in config.profiles {
        let source = if profile.builtin { "builtin" } else { "config" };
        table.add_row(vec![
            profile.name,
            profile.backend,
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
