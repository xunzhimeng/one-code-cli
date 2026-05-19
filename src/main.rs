mod backend;
mod cli;
mod cli_defaults;
mod commands;
mod config;
mod config_ui;
mod documents;
mod error;
mod i18n;
mod ids;
mod output;
mod run_record;
mod runner;
mod session;
mod skills;
mod vibe;

use clap::Parser;

use crate::cli::{
    BackendsCommand, Cli, Commands, ConfigCommand, ProfilesCommand, RunsCommand, SessionsCommand,
    SkillsCommand,
};
use crate::error::OccResult;

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
        Commands::Doctor => commands::doctor(cli.config.as_ref()),
        Commands::Profiles(args) => match args.command {
            Some(ProfilesCommand::Add(args)) => commands::profiles_add(cli.config.as_ref(), *args),
            Some(ProfilesCommand::Show { name }) => {
                commands::profiles_show(cli.config.as_ref(), &name)
            }
            Some(ProfilesCommand::Test { name }) => {
                commands::profiles_test(cli.config.as_ref(), &name)
            }
            Some(ProfilesCommand::List) | None => commands::profiles_list(cli.config.as_ref()),
        },
        Commands::Backends(args) => match args.command {
            Some(BackendsCommand::Show { name }) => {
                commands::backends_show(cli.config.as_ref(), &name)
            }
            Some(BackendsCommand::List) | None => commands::backends_list(cli.config.as_ref()),
        },
        Commands::Config(args) => match args.command {
            Some(ConfigCommand::Init {
                user,
                project,
                force,
            }) => commands::config_init(user, project, force),
            Some(ConfigCommand::Path) => commands::config_path(),
            Some(ConfigCommand::Show { raw }) => commands::config_show(cli.config.as_ref(), raw),
            Some(ConfigCommand::Validate) => commands::config_validate(cli.config.as_ref()),
            Some(ConfigCommand::Ui { output }) => {
                commands::config_ui(cli.config.as_ref(), output, true)
            }
            Some(ConfigCommand::Html {
                save_to,
                port,
                no_open,
            }) => commands::config_html(cli.config.as_ref(), save_to, port, !no_open),
            Some(ConfigCommand::ExportHtml {
                output,
                target,
                open,
            }) => commands::config_export_html(cli.config.as_ref(), output, target, open),
            None => commands::config_show(cli.config.as_ref(), false),
        },
        Commands::Sessions(args) => match args.command {
            Some(SessionsCommand::List { limit }) => {
                commands::sessions_list(cli.config.as_ref(), limit)
            }
            Some(SessionsCommand::Show { session_id }) => {
                commands::sessions_show(cli.config.as_ref(), &session_id)
            }
            Some(SessionsCommand::Resume(args)) => {
                runner::resume_session(cli.config.as_ref(), args)
            }
            Some(SessionsCommand::Latest(args)) => {
                commands::sessions_latest(cli.config.as_ref(), args.profile, args.backend, args.cwd)
            }
            Some(SessionsCommand::Migrate) => commands::sessions_migrate(cli.config.as_ref()),
            None => commands::sessions_list(cli.config.as_ref(), 20),
        },
        Commands::Runs(args) => match args.command {
            Some(RunsCommand::List { limit }) => commands::runs_list(cli.config.as_ref(), limit),
            Some(RunsCommand::Show { run_id }) => commands::runs_show(cli.config.as_ref(), &run_id),
            Some(RunsCommand::Open { run_id, print }) => {
                commands::runs_open(cli.config.as_ref(), &run_id, print)
            }
            None => commands::runs_list(cli.config.as_ref(), 20),
        },
        Commands::Skills(args) => match args.command {
            Some(SkillsCommand::Show { name }) => commands::skills_show(&name),
            Some(SkillsCommand::Export { name, target }) => commands::skills_export(&name, &target),
            Some(SkillsCommand::Install { target }) => commands::skills_install(target),
            Some(SkillsCommand::Doctor { target }) => commands::skills_doctor(target),
            Some(SkillsCommand::List) | None => commands::skills_list(),
        },
        Commands::Settings(args) => commands::config_settings(cli.config.as_ref(), args),
    }
}
