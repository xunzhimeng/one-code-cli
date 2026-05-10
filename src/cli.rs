use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[command(name = "occ", version, about = "One Code CLI")]
pub struct Cli {
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

impl Cli {
    pub fn wants_json_errors(&self) -> bool {
        matches!(
            &self.command,
            Commands::Run(args) if args.output == OutputMode::Json
        ) || matches!(
            &self.command,
            Commands::Sessions(SessionsArgs {
                command: SessionsCommand::Resume(args),
            }) if args.output == OutputMode::Json
        )
    }
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Run(RunArgs),
    Doctor,
    Profiles(ProfilesArgs),
    Backends(BackendsArgs),
    Config(ConfigArgs),
    Sessions(SessionsArgs),
    Runs(RunsArgs),
    Skills(SkillsArgs),
}

#[derive(Debug, Args, Clone)]
pub struct RunArgs {
    #[arg(long)]
    pub profile: Option<String>,

    #[arg(long)]
    pub backend: Option<String>,

    #[arg(long)]
    pub model: Option<String>,

    #[arg(long)]
    pub cwd: Option<PathBuf>,

    #[arg(long)]
    pub prompt: Option<String>,

    #[arg(long)]
    pub prompt_file: Option<PathBuf>,

    #[arg(long)]
    pub stdin: bool,

    #[arg(long)]
    pub interactive: bool,

    #[arg(long)]
    pub non_interactive: bool,

    #[arg(long)]
    pub session: Option<String>,

    #[arg(long)]
    pub resume: bool,

    #[arg(long)]
    pub doc_root: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = OutputMode::Text)]
    pub output: OutputMode,

    #[arg(long)]
    pub timeout: Option<String>,

    #[arg(long)]
    pub dry_run: bool,

    #[arg(last = true)]
    pub child_args: Vec<String>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputMode {
    Text,
    Json,
    Path,
}

#[derive(Debug, Args)]
pub struct ProfilesArgs {
    #[command(subcommand)]
    pub command: ProfilesCommand,
}

#[derive(Debug, Subcommand)]
pub enum ProfilesCommand {
    List,
    Show { name: String },
    Test { name: String },
}

#[derive(Debug, Args)]
pub struct BackendsArgs {
    #[command(subcommand)]
    pub command: BackendsCommand,
}

#[derive(Debug, Subcommand)]
pub enum BackendsCommand {
    List,
    Show { name: String },
}

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    Init {
        #[arg(long)]
        user: bool,

        #[arg(long)]
        project: bool,

        #[arg(long)]
        force: bool,
    },
    Path,
    Show,
    Validate,
    Ui {
        #[arg(long)]
        output: Option<PathBuf>,
    },
    ExportHtml {
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Args)]
pub struct SessionsArgs {
    #[command(subcommand)]
    pub command: SessionsCommand,
}

#[derive(Debug, Subcommand)]
pub enum SessionsCommand {
    List {
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    Show {
        session_id: String,
    },
    Resume(SessionResumeArgs),
    Latest(SessionLatestArgs),
}

#[derive(Debug, Args)]
pub struct SessionResumeArgs {
    pub session_id: String,

    #[arg(long)]
    pub prompt: Option<String>,

    #[arg(long)]
    pub prompt_file: Option<PathBuf>,

    #[arg(long)]
    pub stdin: bool,

    #[arg(long)]
    pub cwd: Option<PathBuf>,

    #[arg(long)]
    pub model: Option<String>,

    #[arg(long)]
    pub doc_root: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = OutputMode::Text)]
    pub output: OutputMode,

    #[arg(long)]
    pub dry_run: bool,

    #[arg(last = true)]
    pub child_args: Vec<String>,
}

#[derive(Debug, Args)]
pub struct SessionLatestArgs {
    #[arg(long)]
    pub profile: Option<String>,

    #[arg(long)]
    pub backend: Option<String>,

    #[arg(long)]
    pub cwd: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct RunsArgs {
    #[command(subcommand)]
    pub command: RunsCommand,
}

#[derive(Debug, Subcommand)]
pub enum RunsCommand {
    List {
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    Show {
        run_id: String,
    },
    Open {
        run_id: String,

        #[arg(long)]
        print: bool,
    },
}

#[derive(Debug, Args)]
pub struct SkillsArgs {
    #[command(subcommand)]
    pub command: SkillsCommand,
}

#[derive(Debug, Subcommand)]
pub enum SkillsCommand {
    List,
    Show {
        name: String,
    },
    Export {
        name: String,

        #[arg(long)]
        target: PathBuf,
    },
    Install {
        #[arg(long)]
        target: PathBuf,
    },
    Doctor {
        #[arg(long)]
        target: Option<PathBuf>,
    },
}
