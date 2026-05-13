use std::fs;
use std::path::{Path, PathBuf};

use colored::Colorize;

use crate::backend;
use crate::config;
use crate::error::OccResult;
use crate::i18n;
use crate::output;
use crate::session;

use super::current_cwd;

pub fn doctor(config_arg: Option<&PathBuf>) -> OccResult<()> {
    let cwd = current_cwd()?;
    let config = config::load(config_arg, &cwd)?;

    println!("{}", i18n::t("doctor.title").bold());
    println!();

    // ── Current directory ──
    println!("{}", output::section_title(i18n::t("doctor.current_dir")));
    match fs::read_dir(&cwd) {
        Ok(_) => println!(
            "  {} cwd readable: {}",
            "✓".green(),
            output::display_path(&cwd).dimmed()
        ),
        Err(error) => println!(
            "  {} cwd readable: {} ({})",
            "✗".red(),
            output::display_path(&cwd),
            error
        ),
    }
    println!();

    // ── Config search paths ──
    println!("{}", output::section_title(i18n::t("doctor.config_search")));
    for path in &config.search_paths {
        println!("  {}", output::display_path(path).dimmed());
    }
    println!();

    // ── Loaded config files ──
    println!(
        "{}: {}",
        output::section_title(i18n::t("doctor.loaded_config")),
        config.loaded_paths.len()
    );
    for path in &config.loaded_paths {
        println!("  {} config: {}", "✓".green(), output::display_path(path));
    }
    if config.loaded_paths.is_empty() {
        println!("  {} config: using built-in defaults", "✓".green());
    }

    // ── Config semantics ──
    match super::config_cmd::validate_config_semantics(&config) {
        Ok(_) => println!("  {} config_semantics", "✓".green()),
        Err(error) => println!(
            "  {} config_semantics: {}: {}",
            "✗".red(),
            error.code(),
            output::display_text(error.message())
        ),
    }
    println!();

    // ── Storage and sessions ──
    println!("{}", output::section_title(i18n::t("doctor.storage")));
    let doc_root = config.resolved_doc_root(&cwd, None);
    match fs::create_dir_all(&doc_root) {
        Ok(_) => {
            let probe = doc_root.join(".doctor-write-test");
            match fs::write(&probe, b"ok").and_then(|_| fs::remove_file(&probe)) {
                Ok(_) => println!(
                    "  {} doc_root writable: {}",
                    "✓".green(),
                    output::display_path(&doc_root).dimmed()
                ),
                Err(error) => println!(
                    "  {} doc_root writable: {} ({})",
                    "✗".red(),
                    output::display_path(&doc_root),
                    error
                ),
            }
        }
        Err(error) => println!(
            "  {} doc_root: {} ({})",
            "✗".red(),
            output::display_path(&doc_root),
            error
        ),
    }
    match session::check_user_store() {
        Ok(path) => println!(
            "  {} session_store: {}",
            "✓".green(),
            output::display_path(&path).dimmed()
        ),
        Err(error) => println!(
            "  {} session_store: {}: {}",
            "✗".red(),
            error.code(),
            output::display_text(error.message())
        ),
    }
    println!(
        "  proxy_forwarding: {}",
        if config.proxy.enabled {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!("  utf8_env_defaults: enabled_when_missing");
    println!();

    // ── CLIs ──
    println!("{}", output::section_title(i18n::t("doctor.backends")));
    for b in backend::all() {
        let executable_ok = if Path::new(b.default_command).is_absolute() {
            Path::new(b.default_command).exists()
        } else {
            which::which(b.default_command).is_ok()
        };
        let version = if executable_ok {
            query_backend_version(b.default_command).unwrap_or_default()
        } else {
            String::new()
        };
        let status_icon = if executable_ok {
            format!("{}", "✓".green())
        } else {
            format!("{}", "⚠".yellow())
        };
        if version.is_empty() {
            println!(
                "  {} cli {}: command {}",
                status_icon,
                b.name.cyan(),
                b.default_command
            );
        } else {
            println!(
                "  {} cli {}: command {} ({})",
                status_icon,
                b.name.cyan(),
                b.default_command,
                version.dimmed()
            );
        }
    }
    println!();

    // ── Agents ──
    println!("{}", output::section_title(i18n::t("doctor.profiles")));
    for profile in &config.profiles {
        let backend_status = backend::get(&profile.backend);
        let (command, source) = if let Some(path) = &profile.path {
            (output::display_path(path), "path")
        } else if let Some(command) = &profile.command {
            (command.clone(), "agent-command")
        } else if let Some(backend) = backend_status {
            (backend.default_command.to_string(), "builtin-default")
        } else {
            (profile.backend.clone(), "unresolved-cli-type")
        };
        let executable_ok =
            if Path::new(&command).is_absolute() || Path::new(&command).components().count() > 1 {
                Path::new(&command).exists()
            } else {
                which::which(&command).is_ok()
            };
        let resume_status = match backend_status {
            None => "invalid-cli-type",
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
        let t = i18n::t;
        if backend_status.is_none() {
            println!(
                "  {} {} {} {}",
                "✗".red(),
                profile.name.cyan(),
                t("doctor.unknown_backend"),
                profile.backend.red()
            );
            continue;
        }
        let status = if executable_ok {
            format!("{}", "✓".green())
        } else {
            format!("{}", "⚠".yellow())
        };
        println!(
            "  {} {} {}",
            status,
            profile.name.cyan().bold(),
            format!("({})", profile.backend).dimmed()
        );
        println!(
            "      {}: {}   {}: {}   {}: {}",
            t("doctor.executable").dimmed(),
            if executable_ok {
                command.clone()
            } else {
                command.red().to_string()
            },
            t("doctor.source").dimmed(),
            source,
            t("doctor.resume").dimmed(),
            resume_status
        );
    }
    Ok(())
}

/// Query the version of a backend CLI by running `<command> --version`.
/// Returns the first line of output (trimmed), or None on failure.
fn query_backend_version(command: &str) -> Option<String> {
    use std::process::Command;
    let output = Command::new(command).arg("--version").output().ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let text = if stdout.trim().is_empty() {
        stderr
    } else {
        stdout
    };
    text.lines()
        .next()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
}
