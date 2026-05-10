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

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;

use crate::backend::TemplateContext;
use crate::cli::{
    BackendsCommand, Cli, Commands, ConfigCommand, ProfilesCommand, RunsCommand, SessionsCommand,
    SkillsCommand,
};
use crate::error::{OccError, OccResult};

fn main() {
    let cli = Cli::parse();
    let json_errors = cli.wants_json_errors();
    if let Err(error) = dispatch(cli) {
        if json_errors {
            output::print_json_error(&error);
        } else {
            eprintln!("{}: {}", error.code(), error.message());
        }
        std::process::exit(1);
    }
}

fn dispatch(cli: Cli) -> OccResult<()> {
    match cli.command {
        Commands::Run(args) => runner::run(cli.config.as_ref(), args),
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
            ConfigCommand::ExportHtml { output } => config_ui(cli.config.as_ref(), output, false),
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
    println!("cwd: {}", cwd.display());
    println!("loaded_config_files: {}", config.loaded_paths.len());
    for path in &config.loaded_paths {
        println!("  ok config: {}", path.display());
    }
    if config.loaded_paths.is_empty() {
        println!("  ok config: using built-in defaults");
    }
    let doc_root = config.resolved_doc_root(&cwd, None);
    match fs::create_dir_all(&doc_root) {
        Ok(_) => println!("ok doc_root: {}", doc_root.display()),
        Err(error) => println!("error doc_root: {} ({})", doc_root.display(), error),
    }
    for backend in backend::all() {
        println!(
            "backend {}: command {}",
            backend.name, backend.default_command
        );
    }
    for profile in &config.profiles {
        let command = profile
            .path
            .as_ref()
            .map(|path| path.display().to_string())
            .or_else(|| profile.command.clone())
            .unwrap_or_else(|| profile.backend.clone());
        let status = if Path::new(&command).is_absolute() {
            Path::new(&command).exists()
        } else {
            which::which(&command).is_ok()
        };
        println!(
            "{} profile {} ({}) executable {}",
            if status { "ok" } else { "missing" },
            profile.name,
            profile.backend,
            command
        );
    }
    Ok(())
}

fn profiles_list(config_arg: Option<&PathBuf>) -> OccResult<()> {
    let config = load_current(config_arg)?;
    for profile in config.profiles {
        let source = if profile.builtin { "builtin" } else { "config" };
        println!("{}\t{}\t{}", profile.name, profile.backend, source);
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
    println!("{}", text);
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
        plan.executable.display(),
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
        println!(
            "{}\tcommand={}\tbuiltin_profile={}\tdefault_profile={}\tresume={}",
            backend.name,
            backend.default_command,
            backend.builtin_profile,
            default_profile,
            backend.supports_resume
        );
    }
    Ok(())
}

fn backends_show(config_arg: Option<&PathBuf>, name: &str) -> OccResult<()> {
    let config = load_current(config_arg)?;
    let backend = backend::require(name)?;
    let value = serde_json::json!({
        "name": backend.name,
        "default_command": backend.default_command,
        "builtin_profile": backend.builtin_profile,
        "backend_default_profile": config.backend_defaults.get(backend.name),
        "supports_model": backend.supports_model,
        "supports_interactive": backend.supports_interactive,
        "supports_non_interactive": backend.supports_non_interactive,
        "supports_resume": backend.supports_resume,
        "default_prompt_via": backend.default_prompt_via,
        "non_interactive_args": backend.non_interactive_args,
        "interactive_args": backend.interactive_args,
        "resume_args": backend.resume_args,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&value).map_err(|error| {
            OccError::new(
                "config_parse_failed",
                format!("Failed to serialize backend JSON: {}", error),
            )
        })?
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
    println!("created: {}", path.display());
    Ok(())
}

fn config_path() -> OccResult<()> {
    let cwd = current_cwd()?;
    println!("search paths:");
    for path in config::search_paths(&cwd) {
        println!("{}", path.display());
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
    println!("{}", text);
    Ok(())
}

fn config_validate(config_arg: Option<&PathBuf>) -> OccResult<()> {
    let config = load_current(config_arg)?;
    for profile in &config.profiles {
        backend::require(&profile.backend)?;
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
    println!("ok");
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
        println!("html: {}", path.display());
    }
    Ok(())
}

fn sessions_list(config_arg: Option<&PathBuf>, limit: usize) -> OccResult<()> {
    let doc_root = current_doc_root(config_arg)?;
    for entry in session::list(&doc_root, limit)? {
        println!(
            "{}\t{}\t{}\t{}\t{}",
            entry.session_id,
            entry.profile,
            entry.backend,
            entry.cwd.display(),
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
    println!("{}", text);
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
        entry.cwd.display(),
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
            format!("Failed to read '{}'", entry.metadata_path.display()),
            error,
        )
    })?;
    println!("{}", text);
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
    println!("{}", entry.result_path.display());
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
    println!("exported: {}", path.display());
    Ok(())
}

fn skills_install(target: &Path) -> OccResult<()> {
    for path in skills::install(target)? {
        println!("installed: {}", path.display());
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
