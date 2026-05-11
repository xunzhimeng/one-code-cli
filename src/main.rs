mod backend;
mod cli;
mod config;
mod config_ui;
mod documents;
mod error;
mod ids;
mod output;
mod run_record;
mod runner;
mod session;
mod skills;
mod vibe;

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;

use crate::backend::TemplateContext;
use crate::cli::{
    BackendsCommand, Cli, Commands, ConfigCommand, ConfigTarget, ProfilesCommand, RunsCommand,
    SessionsCommand, SkillsCommand,
};
use crate::error::{OccError, OccResult};

fn main() {
    let cli = Cli::parse();
    let json_errors = cli.wants_json_errors();
    if let Err(error) = dispatch(cli) {
        if json_errors {
            output::print_json_error(&error);
        } else {
            eprintln!(
                "{}: {}",
                error.code(),
                output::display_text(error.message())
            );
        }
        std::process::exit(1);
    }
}

fn dispatch(cli: Cli) -> OccResult<()> {
    match cli.command {
        Commands::Run(args) => runner::run(cli.config.as_ref(), args),
        Commands::Vibe(args) => vibe::start(cli.config.as_ref(), args),
        Commands::Doctor => doctor(cli.config.as_ref()),
        Commands::Profiles(args) => match args.command {
            ProfilesCommand::List => profiles_list(cli.config.as_ref()),
            ProfilesCommand::Show { name } => profiles_show(cli.config.as_ref(), &name),
            ProfilesCommand::Test { name } => profiles_test(cli.config.as_ref(), &name),
        },
        Commands::Backends(args) => match args.command {
            BackendsCommand::List => backends_list(cli.config.as_ref()),
            BackendsCommand::Show { name } => backends_show(cli.config.as_ref(), &name),
        },
        Commands::Config(args) => match args.command {
            ConfigCommand::Init {
                user,
                project,
                force,
            } => config_init(user, project, force),
            ConfigCommand::Path => config_path(),
            ConfigCommand::Show => config_show(cli.config.as_ref()),
            ConfigCommand::Validate => config_validate(cli.config.as_ref()),
            ConfigCommand::Ui { output } => config_ui(cli.config.as_ref(), output, true),
            ConfigCommand::ExportHtml {
                output,
                target,
                open,
            } => config_export_html(cli.config.as_ref(), output, target, open),
        },
        Commands::Sessions(args) => match args.command {
            SessionsCommand::List { limit } => sessions_list(cli.config.as_ref(), limit),
            SessionsCommand::Show { session_id } => sessions_show(cli.config.as_ref(), &session_id),
            SessionsCommand::Resume(args) => runner::resume_session(cli.config.as_ref(), args),
            SessionsCommand::Latest(args) => {
                sessions_latest(cli.config.as_ref(), args.profile, args.backend, args.cwd)
            }
        },
        Commands::Runs(args) => match args.command {
            RunsCommand::List { limit } => runs_list(cli.config.as_ref(), limit),
            RunsCommand::Show { run_id } => runs_show(cli.config.as_ref(), &run_id),
            RunsCommand::Open { run_id, print } => runs_open(cli.config.as_ref(), &run_id, print),
        },
        Commands::Skills(args) => match args.command {
            SkillsCommand::List => skills_list(),
            SkillsCommand::Show { name } => skills_show(&name),
            SkillsCommand::Export { name, target } => skills_export(&name, &target),
            SkillsCommand::Install { target } => skills_install(&target),
            SkillsCommand::Doctor { target } => skills_doctor(target),
        },
    }
}

fn current_cwd() -> OccResult<PathBuf> {
    env::current_dir()
        .map_err(|error| OccError::io("cwd_not_found", "Failed to read current directory", error))
}

fn load_current(config_arg: Option<&PathBuf>) -> OccResult<config::EffectiveConfig> {
    let cwd = current_cwd()?;
    config::load(config_arg, &cwd)
}

fn current_doc_root(config_arg: Option<&PathBuf>) -> OccResult<PathBuf> {
    let cwd = current_cwd()?;
    let config = config::load(config_arg, &cwd)?;
    Ok(config.resolved_doc_root(&cwd, None))
}

fn doctor(config_arg: Option<&PathBuf>) -> OccResult<()> {
    let cwd = current_cwd()?;
    let config = config::load(config_arg, &cwd)?;
    println!("One Code CLI doctor");
    match fs::read_dir(&cwd) {
        Ok(_) => println!("ok cwd readable: {}", output::display_path(&cwd)),
        Err(error) => println!(
            "error cwd readable: {} ({})",
            output::display_path(&cwd),
            error
        ),
    }
    println!("config_search_paths:");
    for path in &config.search_paths {
        println!("  {}", output::display_path(path));
    }
    println!("loaded_config_files: {}", config.loaded_paths.len());
    for path in &config.loaded_paths {
        println!("  ok config: {}", output::display_path(path));
    }
    if config.loaded_paths.is_empty() {
        println!("  ok config: using built-in defaults");
    }
    match validate_config_semantics(&config) {
        Ok(_) => println!("ok config_semantics"),
        Err(error) => println!(
            "error config_semantics: {}: {}",
            error.code(),
            output::display_text(error.message())
        ),
    }
    let doc_root = config.resolved_doc_root(&cwd, None);
    match fs::create_dir_all(&doc_root) {
        Ok(_) => {
            let probe = doc_root.join(".doctor-write-test");
            match fs::write(&probe, b"ok").and_then(|_| fs::remove_file(&probe)) {
                Ok(_) => println!("ok doc_root writable: {}", output::display_path(&doc_root)),
                Err(error) => println!(
                    "error doc_root writable: {} ({})",
                    output::display_path(&doc_root),
                    error
                ),
            }
        }
        Err(error) => println!(
            "error doc_root: {} ({})",
            output::display_path(&doc_root),
            error
        ),
    }
    match session::check_user_store() {
        Ok(path) => println!("ok session_store: {}", output::display_path(&path)),
        Err(error) => println!(
            "error session_store: {}: {}",
            error.code(),
            output::display_text(error.message())
        ),
    }
    println!(
        "proxy_forwarding: {}",
        if config.proxy.enabled {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!("utf8_env_defaults: enabled_when_missing");
    for backend in backend::all() {
        println!(
            "backend {}: command {}",
            backend.name, backend.default_command
        );
    }
    for profile in &config.profiles {
        let backend_status = backend::get(&profile.backend);
        let (command, source) = if let Some(path) = &profile.path {
            (output::display_path(path), "path")
        } else if let Some(command) = &profile.command {
            (command.clone(), "profile-command")
        } else if let Some(backend) = backend_status {
            (backend.default_command.to_string(), "builtin-default")
        } else {
            (profile.backend.clone(), "unresolved-backend")
        };
        let executable_ok =
            if Path::new(&command).is_absolute() || Path::new(&command).components().count() > 1 {
                Path::new(&command).exists()
            } else {
                which::which(&command).is_ok()
            };
        let resume_status = match backend_status {
            None => "invalid-backend",
            Some(backend) if profile.resume_args.is_empty() && !backend.supports_resume => {
                "unsupported"
            }
            Some(backend)
                if profile.resume_args.is_empty()
                    && backend
                        .resume_args
                        .iter()
                        .any(|value| value.contains("{backend_session_id}")) =>
            {
                "requires-backend-session"
            }
            Some(_) if profile.resume_args.is_empty() => "builtin-available",
            Some(_) => "configured",
        };
        if backend_status.is_none() {
            println!(
                "error profile {} references unknown backend {}",
                profile.name, profile.backend
            );
        }
        println!(
            "{} profile {} ({}) executable {} source {} resume {}",
            if executable_ok { "ok" } else { "missing" },
            profile.name,
            profile.backend,
            command,
            source,
            resume_status
        );
    }
    Ok(())
}

fn profiles_list(config_arg: Option<&PathBuf>) -> OccResult<()> {
    let config = load_current(config_arg)?;
    for profile in config.profiles {
        let source = if profile.builtin { "builtin" } else { "config" };
        println!(
            "{}\t{}\t{}\taliases={}",
            profile.name,
            profile.backend,
            source,
            profile.aliases.join(",")
        );
    }
    Ok(())
}

fn profiles_show(config_arg: Option<&PathBuf>, name: &str) -> OccResult<()> {
    let config = load_current(config_arg)?;
    let profile = config.profile(name).ok_or_else(|| {
        OccError::new(
            "profile_not_found",
            format!("Profile '{}' was not found.", name),
        )
    })?;
    let text = toml::to_string_pretty(profile).map_err(|error| {
        OccError::new(
            "config_parse_failed",
            format!("Failed to serialize profile: {}", error),
        )
    })?;
    println!("{}", output::display_text(&text));
    Ok(())
}

fn profiles_test(config_arg: Option<&PathBuf>, name: &str) -> OccResult<()> {
    let cwd = current_cwd()?;
    let config = config::load(config_arg, &cwd)?;
    let profile = config.profile(name).ok_or_else(|| {
        OccError::new(
            "profile_not_found",
            format!("Profile '{}' was not found.", name),
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
    println!("profile: {}", profile.name);
    println!("backend: {}", profile.backend);
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

fn backends_list(config_arg: Option<&PathBuf>) -> OccResult<()> {
    let config = load_current(config_arg)?;
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
        println!(
            "{}\tcommand={}\tbuiltin_profile={}\tdefault_profile={}\taliases={}\tresume={}",
            backend.name,
            backend.default_command,
            backend.builtin_profile,
            default_profile,
            aliases,
            backend.supports_resume
        );
    }
    Ok(())
}

fn backends_show(config_arg: Option<&PathBuf>, name: &str) -> OccResult<()> {
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
        "builtin_profile": backend.builtin_profile,
        "backend_default_profile": config.backend_defaults.get(backend.name),
        "aliases": config
            .backend_aliases
            .iter()
            .filter_map(|(alias, target)| (target == backend.name).then_some(alias))
            .collect::<Vec<_>>(),
        "supports_model": backend.supports_model,
        "supports_interactive": backend.supports_interactive,
        "supports_non_interactive": backend.supports_non_interactive,
        "supports_resume": backend.supports_resume,
        "default_prompt_via": backend.default_prompt_via,
        "prompt_transports": backend.prompt_transports,
        "file_indirection_template": backend.file_indirection_template,
        "non_interactive_args": backend.non_interactive_args,
        "interactive_args": backend.interactive_args,
        "resume_args": backend.resume_args,
    });
    println!(
        "{}",
        output::display_text(&serde_json::to_string_pretty(&value).map_err(|error| {
            OccError::new(
                "config_parse_failed",
                format!("Failed to serialize backend JSON: {}", error),
            )
        })?)
    );
    Ok(())
}

fn config_init(user: bool, project: bool, force: bool) -> OccResult<()> {
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

fn config_path() -> OccResult<()> {
    let cwd = current_cwd()?;
    println!("search paths:");
    for path in config::search_paths(&cwd) {
        println!("{}", output::display_path(&path));
    }
    Ok(())
}

fn config_show(config_arg: Option<&PathBuf>) -> OccResult<()> {
    let config = load_current(config_arg)?;
    let text = toml::to_string_pretty(&config).map_err(|error| {
        OccError::new(
            "config_parse_failed",
            format!("Failed to serialize config: {}", error),
        )
    })?;
    println!("{}", output::display_text(&text));
    Ok(())
}

fn config_validate(config_arg: Option<&PathBuf>) -> OccResult<()> {
    let config = load_current(config_arg)?;
    validate_config_semantics(&config)?;
    println!("ok");
    Ok(())
}

fn validate_config_semantics(config: &config::EffectiveConfig) -> OccResult<()> {
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
                    format!("Profile alias '{}' shadows a real backend name.", alias),
                ));
            }
        }
    }
    for (alias, target) in &config.backend_aliases {
        if backend_names.iter().any(|name| name == alias) {
            return Err(OccError::new(
                "backend_alias_conflict",
                format!("Backend alias '{}' shadows a real backend name.", alias),
            ));
        }
        if let Some(profile) = profile_names.get(alias) {
            return Err(OccError::new(
                "backend_alias_conflict",
                format!(
                    "Backend alias '{}' shadows profile name or alias '{}'.",
                    alias, profile
                ),
            ));
        }
        backend::require(target)?;
    }
    for (backend, profile) in &config.backend_defaults {
        backend::require(backend)?;
        if config.profile(profile).is_none() {
            return Err(OccError::new(
                "profile_not_found",
                format!("Backend default profile '{}' was not found.", profile),
            ));
        }
    }
    if let Some(default_profile) = &config.default_profile {
        if config.profile(default_profile).is_none() {
            return Err(OccError::new(
                "profile_not_found",
                format!("Default profile '{}' was not found.", default_profile),
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
                "Profile name or alias '{}' is used by both '{}' and '{}'.",
                name, existing, profile
            ),
        ));
    }
    Ok(())
}

fn config_ui(
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
        println!("html: {}", output::display_path(&path));
    }
    Ok(())
}

fn config_export_html(
    config_arg: Option<&PathBuf>,
    output: Option<PathBuf>,
    target: ConfigTarget,
    open_browser: bool,
) -> OccResult<()> {
    let cwd = current_cwd()?;
    let config = config::load(config_arg, &cwd)?;
    let initial = config::editable_config_toml(&config)?;
    let recommended_path = recommended_config_path(&config, &cwd, target)?;
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
    println!("html: {}", output::display_path(&path));
    println!(
        "recommended_config: {}",
        output::display_path(&metadata.recommended_path)
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

fn sessions_list(config_arg: Option<&PathBuf>, limit: usize) -> OccResult<()> {
    let doc_root = current_doc_root(config_arg)?;
    for entry in session::list(&doc_root, limit)? {
        println!(
            "{}\t{}\t{}\t{}\t{}",
            entry.session_id,
            entry.profile,
            entry.backend,
            output::display_path(&entry.cwd),
            entry.updated_at
        );
    }
    Ok(())
}

fn sessions_show(config_arg: Option<&PathBuf>, session_id: &str) -> OccResult<()> {
    let doc_root = current_doc_root(config_arg)?;
    let session = session::load_by_id(&doc_root, session_id)?;
    let text = toml::to_string_pretty(&session).map_err(|error| {
        OccError::new(
            "config_parse_failed",
            format!("Failed to serialize session: {}", error),
        )
    })?;
    println!("{}", output::display_text(&text));
    Ok(())
}

fn sessions_latest(
    config_arg: Option<&PathBuf>,
    profile: Option<String>,
    backend: Option<String>,
    cwd: Option<PathBuf>,
) -> OccResult<()> {
    let base_cwd = current_cwd()?;
    let doc_root = current_doc_root(config_arg)?;
    let cwd = cwd
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                base_cwd.join(path)
            }
        })
        .map(|path| path.canonicalize().unwrap_or(path));
    let entry = session::latest(
        &doc_root,
        profile.as_deref(),
        backend.as_deref(),
        cwd.as_deref(),
    )?
    .ok_or_else(|| OccError::new("session_not_found", "No matching session was found."))?;
    println!(
        "{}\t{}\t{}\t{}\t{}",
        entry.session_id,
        entry.profile,
        entry.backend,
        output::display_path(&entry.cwd),
        entry.updated_at
    );
    Ok(())
}

fn runs_list(config_arg: Option<&PathBuf>, limit: usize) -> OccResult<()> {
    let doc_root = current_doc_root(config_arg)?;
    for entry in run_record::list(&doc_root, limit)? {
        println!(
            "{}\t{}\t{}\t{}\t{}",
            entry.run_id, entry.session_id, entry.profile, entry.success, entry.created_at
        );
    }
    Ok(())
}

fn runs_show(config_arg: Option<&PathBuf>, run_id: &str) -> OccResult<()> {
    let doc_root = current_doc_root(config_arg)?;
    let entry = run_record::find(&doc_root, run_id)?.ok_or_else(|| {
        OccError::new(
            "config_not_found",
            format!("Run '{}' was not found.", run_id),
        )
    })?;
    let text = fs::read_to_string(&entry.metadata_path).map_err(|error| {
        OccError::io(
            "config_parse_failed",
            format!(
                "Failed to read '{}'",
                output::display_path(&entry.metadata_path)
            ),
            error,
        )
    })?;
    println!("{}", output::display_text(&text));
    Ok(())
}

fn runs_open(config_arg: Option<&PathBuf>, run_id: &str, print: bool) -> OccResult<()> {
    let doc_root = current_doc_root(config_arg)?;
    let entry = run_record::find(&doc_root, run_id)?.ok_or_else(|| {
        OccError::new(
            "config_not_found",
            format!("Run '{}' was not found.", run_id),
        )
    })?;
    println!("{}", output::display_path(&entry.result_path));
    if !print {
        let _ = open::that(&entry.result_path);
    }
    Ok(())
}

fn skills_list() -> OccResult<()> {
    for skill in skills::all() {
        println!("{}\t{}", skill.name, skill.description);
    }
    Ok(())
}

fn skills_show(name: &str) -> OccResult<()> {
    let skill = skills::require(name)?;
    println!("{}", skill.skill_md);
    Ok(())
}

fn skills_export(name: &str, target: &Path) -> OccResult<()> {
    let path = skills::export(name, target)?;
    println!("exported: {}", output::display_path(&path));
    Ok(())
}

fn skills_install(target: &Path) -> OccResult<()> {
    for path in skills::install(target)? {
        println!("installed: {}", output::display_path(&path));
    }
    Ok(())
}

fn skills_doctor(target: Option<PathBuf>) -> OccResult<()> {
    let target = match target {
        Some(path) => path,
        None => directories::BaseDirs::new()
            .map(|base_dirs| base_dirs.home_dir().join(".agents").join("skills"))
            .ok_or_else(|| {
                OccError::new(
                    "config_not_found",
                    "Unable to locate the user home directory.",
                )
            })?,
    };
    for line in skills::doctor(&target)? {
        println!("{}", line);
    }
    Ok(())
}
