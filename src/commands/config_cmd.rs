use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use colored::Colorize;

use crate::backend;
use crate::cli::ConfigTarget;
use crate::config;
use crate::config_ui;
use crate::error::{OccError, OccResult};
use crate::i18n;
use crate::output;

use super::{current_cwd, load_current};

pub fn config_init(user: bool, project: bool, force: bool) -> OccResult<()> {
    let cwd = current_cwd()?;
    let path = if user && !project {
        config::default_user_config_path()?
    } else {
        config::default_project_config_path(&cwd)
    };
    config::write_sample_config(&path, force)?;
    println!("created: {}", output::display_path(&path));
    Ok(())
}

pub fn config_path() -> OccResult<()> {
    let cwd = current_cwd()?;
    println!("search paths:");
    for path in config::search_paths(&cwd) {
        println!("{}", output::display_path(&path));
    }
    Ok(())
}

pub fn config_show(config_arg: Option<&PathBuf>, raw: bool) -> OccResult<()> {
    let config = load_current(config_arg)?;
    if !raw {
        print_config_summary(&config);
        return Ok(());
    }
    let text = toml::to_string_pretty(&config).map_err(|error| {
        OccError::new(
            "serialization_failed",
            format!("Failed to serialize config: {}", error),
        )
    })?;
    println!("{}", output::display_text(&text));
    Ok(())
}

fn print_config_summary(config: &config::EffectiveConfig) {
    let t = i18n::t;

    println!("{}", t("config.summary").bold());
    println!("  {}: {}", t("config.version").dimmed(), config.version);
    println!(
        "  {}: {}",
        t("config.default_profile").dimmed(),
        config
            .default_profile
            .as_deref()
            .map(|s| s.cyan().to_string())
            .unwrap_or_else(|| t("config.default_profile_unset").dimmed().to_string())
    );
    println!(
        "  {}: {}",
        t("config.doc_root").dimmed(),
        output::display_path(&config.doc_root).dimmed()
    );
    let proxy_label = if config.proxy.enabled {
        t("config.proxy_enabled").green().to_string()
    } else {
        t("config.proxy_disabled").yellow().to_string()
    };
    println!("  {}: {}", t("config.proxy").dimmed(), proxy_label);
    println!(
        "  {}: {}",
        t("config.default_timeout").dimmed(),
        config
            .timeouts
            .default_run
            .as_deref()
            .unwrap_or(t("config.timeout_none"))
    );
    println!();

    println!("{}", t("config.loaded_files").bold());
    if config.loaded_paths.is_empty() {
        println!("  {}", t("config.using_defaults").dimmed());
    } else {
        for path in &config.loaded_paths {
            println!("  {} {}", "✓".green(), output::display_path(path));
        }
    }
    println!();

    println!("{}", t("config.search_order").bold());
    for path in &config.search_paths {
        println!("  {}", output::display_path(path).dimmed());
    }
    println!();

    println!("{}", t("config.backend_defaults").bold());
    for (backend, profile) in &config.backend_defaults {
        println!("  {} {} {}", backend.cyan(), "→".dimmed(), profile);
    }
    println!();

    println!("{}", t("config.available_profiles").bold());
    for profile in &config.profiles {
        let source = if profile.builtin {
            t("common.builtin")
        } else {
            t("common.config")
        };
        println!(
            "  {} {} cli={}",
            profile.name.cyan().bold(),
            format!("({})", source).dimmed(),
            profile.backend
        );
    }
    println!();

    println!("{}", t("config.notes").bold());
    println!("  {}", t("config.note_doc_root").dimmed());
    println!("  {}", t("config.note_profile").dimmed());
    println!("  {}", t("config.note_backend_defaults").dimmed());
    println!("  {}", t("config.note_raw").dimmed());
}

pub fn config_validate(config_arg: Option<&PathBuf>) -> OccResult<()> {
    let config = load_current(config_arg)?;
    validate_config_semantics(&config)?;
    println!("ok");
    Ok(())
}

pub fn validate_config_semantics(config: &config::EffectiveConfig) -> OccResult<()> {
    let mut profile_names = BTreeMap::new();
    let backend_names = backend::all()
        .iter()
        .map(|backend| backend.name)
        .collect::<Vec<_>>();
    for profile in &config.profiles {
        backend::require(&profile.backend)?;
        insert_profile_name(&mut profile_names, &profile.name, &profile.name)?;
        for alias in &profile.aliases {
            insert_profile_name(&mut profile_names, alias, &profile.name)?;
            if backend_names.iter().any(|name| name == alias) {
                return Err(OccError::new(
                    "profile_alias_conflict",
                    format!("Agent '{}' shadows a real CLI name.", alias),
                ));
            }
        }
    }
    for (alias, target) in &config.backend_aliases {
        backend::require(target)?;
        if backend_names.iter().any(|name| name == alias) {
            if alias == target {
                continue;
            }
            return Err(OccError::new(
                "backend_alias_conflict",
                format!("CLI alias '{}' shadows a real CLI name.", alias),
            ));
        }
        if let Some(profile) = profile_names.get(alias) {
            return Err(OccError::new(
                "backend_alias_conflict",
                format!(
                    "CLI alias '{}' shadows agent name or alias '{}'.",
                    alias, profile
                ),
            ));
        }
    }
    for (backend, profile) in &config.backend_defaults {
        backend::require(backend)?;
        if config.profile(profile).is_none() {
            return Err(OccError::new(
                "profile_not_found",
                format!("CLI default agent '{}' was not found.", profile),
            ));
        }
    }
    if let Some(default_profile) = &config.default_profile {
        if config.profile(default_profile).is_none() {
            return Err(OccError::new(
                "profile_not_found",
                format!("Default agent '{}' was not found.", default_profile),
            ));
        }
    }
    Ok(())
}

fn insert_profile_name(
    names: &mut BTreeMap<String, String>,
    name: &str,
    profile: &str,
) -> OccResult<()> {
    if let Some(existing) = names.insert(name.to_string(), profile.to_string()) {
        return Err(OccError::new(
            "profile_alias_conflict",
            format!(
                "Agent name or alias '{}' is used by both '{}' and '{}'.",
                name, existing, profile
            ),
        ));
    }
    Ok(())
}

pub fn config_ui(
    config_arg: Option<&PathBuf>,
    output: Option<PathBuf>,
    open_browser: bool,
) -> OccResult<()> {
    let cwd = current_cwd()?;
    let config = config::load(config_arg, &cwd)?;
    let initial = config::editable_config_toml(&config)?;
    if open_browser {
        let save_path = output
            .or_else(|| config.loaded_paths.last().cloned())
            .unwrap_or_else(|| config::default_project_config_path(&cwd));
        config_ui::serve(&initial, &save_path)?;
    } else {
        let path =
            output.unwrap_or_else(|| config.resolved_doc_root(&cwd, None).join("config-ui.html"));
        config_ui::write_html(&path, &initial)?;
        println!("html: {}", crate::output::display_path(&path));
    }
    Ok(())
}

pub fn config_html(
    config_arg: Option<&PathBuf>,
    save_to: Option<PathBuf>,
    port: Option<u16>,
    open_browser: bool,
) -> OccResult<()> {
    let cwd = current_cwd()?;
    let config = config::load(config_arg, &cwd)?;
    let save_path = save_to
        .or_else(|| config.loaded_paths.last().cloned())
        .unwrap_or_else(|| config::default_project_config_path(&cwd));
    let metadata = config_ui::ConfigHtmlMetadata {
        cwd: cwd.clone(),
        target: "loaded".to_string(),
        recommended_path: save_path.clone(),
        loaded_paths: config.loaded_paths.clone(),
        search_paths: config.search_paths.clone(),
        doc_root: config.resolved_doc_root(&cwd, None),
        default_profile: config.default_profile.clone(),
        init_command: "occ config init --user".to_string(),
    };
    let initial_file = editable_config_file_for_path(&save_path)?;
    config_ui::serve_form(&initial_file, &save_path, port, open_browser, metadata)?;
    Ok(())
}

pub fn config_export_html(
    config_arg: Option<&PathBuf>,
    output: Option<PathBuf>,
    target: ConfigTarget,
    open_browser: bool,
) -> OccResult<()> {
    let cwd = current_cwd()?;
    let config = config::load(config_arg, &cwd)?;
    let recommended_path = recommended_config_path(&config, &cwd, target)?;
    let initial = editable_config_toml_for_path(&recommended_path)?;
    let target_name = match target {
        ConfigTarget::User => "user",
        ConfigTarget::Project => "project",
        ConfigTarget::Loaded => "loaded",
    };
    let init_command = match target {
        ConfigTarget::User => "occ config init --user --force",
        ConfigTarget::Project | ConfigTarget::Loaded => "occ config init --project --force",
    };
    let metadata = config_ui::ConfigHtmlMetadata {
        cwd: cwd.clone(),
        target: target_name.to_string(),
        recommended_path,
        loaded_paths: config.loaded_paths.clone(),
        search_paths: config.search_paths.clone(),
        doc_root: config.resolved_doc_root(&cwd, None),
        default_profile: config.default_profile.clone(),
        init_command: init_command.to_string(),
    };
    let path =
        output.unwrap_or_else(|| config.resolved_doc_root(&cwd, None).join("config-ui.html"));
    config_ui::write_static_html(&path, &initial, &metadata)?;
    println!("html: {}", crate::output::display_path(&path));
    println!(
        "recommended_config: {}",
        crate::output::display_path(&metadata.recommended_path)
    );
    if open_browser {
        let _ = open::that(&path);
    }
    Ok(())
}

fn recommended_config_path(
    config: &config::EffectiveConfig,
    cwd: &Path,
    target: ConfigTarget,
) -> OccResult<PathBuf> {
    match target {
        ConfigTarget::User => config::default_user_config_path(),
        ConfigTarget::Project => Ok(config::default_project_config_path(cwd)),
        ConfigTarget::Loaded => Ok(config
            .loaded_paths
            .last()
            .cloned()
            .unwrap_or_else(|| config::default_project_config_path(cwd))),
    }
}

pub fn config_settings(
    config_arg: Option<&PathBuf>,
    args: crate::cli::SettingsArgs,
) -> OccResult<()> {
    if args.output.is_some() {
        if args.server {
            return Err(OccError::new(
                "invalid_argument",
                "--server cannot be combined with --output. Omit --output to open the live settings UI.",
            ));
        }
        config_export_html(config_arg, args.output, args.target, !args.no_open)?;
        return Ok(());
    }

    let cwd = current_cwd()?;
    let config = config::load(config_arg, &cwd)?;
    let save_path = recommended_config_path(&config, &cwd, args.target)?;
    let target_name = match args.target {
        ConfigTarget::User => "user",
        ConfigTarget::Project => "project",
        ConfigTarget::Loaded => "loaded",
    };
    let init_command = match args.target {
        ConfigTarget::User => "occ config init --user --force",
        ConfigTarget::Project | ConfigTarget::Loaded => "occ config init --project --force",
    };
    let metadata = config_ui::ConfigHtmlMetadata {
        cwd: cwd.clone(),
        target: target_name.to_string(),
        recommended_path: save_path.clone(),
        loaded_paths: config.loaded_paths.clone(),
        search_paths: config.search_paths.clone(),
        doc_root: config.resolved_doc_root(&cwd, None),
        default_profile: config.default_profile.clone(),
        init_command: init_command.to_string(),
    };
    let initial_file = editable_config_file_for_path(&save_path)?;
    config_ui::serve_form(
        &initial_file,
        &save_path,
        args.port,
        !args.no_open,
        metadata,
    )?;
    Ok(())
}

fn editable_config_file_for_path(path: &Path) -> OccResult<config::ConfigFile> {
    config::read_config_file(path)
}

fn editable_config_toml_for_path(path: &Path) -> OccResult<String> {
    let file = editable_config_file_for_path(path)?;
    toml::to_string_pretty(&file).map_err(|error| {
        OccError::new(
            "serialization_failed",
            format!("Failed to serialize config TOML: {}", error),
        )
    })
}
