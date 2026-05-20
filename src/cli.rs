use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[command(
    name = "occ",
    version,
    about = "One Code CLI - unified coding-agent CLI dispatcher",
    after_help = "Language: set OCC_LANG=zh-CN or OCC_LANG=en-US to prefer Chinese or English explanatory output."
)]
pub struct Cli {
    #[arg(long, help = "Path to a specific config file")]
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
                command: Some(SessionsCommand::Resume(args)),
            }) if args.output == OutputMode::Json
        )
    }
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(
        about = "Run one delegated task, or fan out to multiple agents / 运行一次委派任务，或并行调用多个 agent"
    )]
    Run(RunArgs),
    #[command(alias = "chat")]
    #[command(about = "Chat with a selected coding CLI / 与指定 CLI 连续对话")]
    Vibe(VibeArgs),
    #[command(
        about = "Check configuration, paths, agent aliases, and executables / 检查配置、路径、agent alias 和可执行文件"
    )]
    Doctor,
    #[command(name = "agents")]
    #[command(about = "List, show, and test agents / 查看、展示和测试 agent")]
    Profiles(ProfilesArgs),
    #[command(name = "clis")]
    #[command(about = "List and inspect supported CLIs / 查看支持的 CLI")]
    Backends(BackendsArgs),
    #[command(about = "Manage and explain occ configuration / 管理并解释 occ 配置")]
    Config(ConfigArgs),
    #[command(about = "List, inspect, and resume sessions / 查看、检查和恢复会话")]
    Sessions(SessionsArgs),
    #[command(about = "List, inspect, and open run artifacts / 查看、检查和打开运行记录")]
    Runs(RunsArgs),
    #[command(about = "Install and inspect bundled skills / 安装和查看内置 skills")]
    Skills(SkillsArgs),
    #[command(about = "Open interactive config settings / 打开可视化配置设置")]
    Settings(SettingsArgs),
}

/// Shared arguments between `run` and `vibe` subcommands.
#[derive(Debug, Args, Clone)]
pub struct CommonArgs {
    #[arg(
        long = "agent",
        value_name = "AGENT",
        help = "Select an exact occ agent by name"
    )]
    pub profile: Option<String>,

    #[arg(
        short = 'b',
        long = "cli",
        value_name = "CLI",
        help = "Select a CLI (claude, codex, opencode, gemini)"
    )]
    pub backend: Option<String>,

    #[arg(short = 'm', long, help = "Override the model for this run")]
    pub model: Option<String>,

    #[arg(
        short = 'e',
        long,
        alias = "reasoning-effort",
        help = "Override reasoning effort for this run"
    )]
    pub effort: Option<String>,

    #[arg(short = 'C', long, help = "Working directory for the child process")]
    pub cwd: Option<PathBuf>,

    #[arg(short = 'p', long, help = "Task prompt text")]
    pub prompt: Option<String>,

    #[arg(long, help = "Read prompt from a file")]
    pub prompt_file: Option<PathBuf>,

    #[arg(long, help = "Read prompt from stdin")]
    pub stdin: bool,

    #[arg(long, help = "Attach to an existing occ session by ID")]
    pub session: Option<String>,

    #[arg(long, help = "Resume the latest or specified session")]
    pub resume: bool,

    #[arg(long, help = "Override the run artifact directory")]
    pub doc_root: Option<PathBuf>,

    #[arg(
        long,
        value_name = "DURATION",
        help = "Task timeout (e.g. 90s, 5m, 3000ms)"
    )]
    pub timeout: Option<String>,

    #[arg(long, help = "Show the command plan without executing")]
    pub dry_run: bool,

    #[arg(
        last = true,
        help = "Additional arguments passed through to the child CLI"
    )]
    pub child_args: Vec<String>,
}

#[derive(Debug, Args, Clone)]
pub struct RunArgs {
    #[command(flatten)]
    pub common: CommonArgs,

    #[arg(
        long = "agents",
        value_delimiter = ',',
        conflicts_with = "profile",
        value_name = "AGENT[,AGENT]",
        help = "Run the same prompt against multiple occ agents in parallel"
    )]
    pub agents: Vec<String>,

    #[arg(short = 'i', long, help = "Force foreground interactive mode")]
    pub interactive: bool,

    #[arg(short = 'n', long, help = "Force non-interactive automation mode")]
    pub non_interactive: bool,

    #[arg(
        short = 's',
        long,
        help = "Mirror child stdout/stderr to parent stderr in real time"
    )]
    pub stream: bool,

    #[arg(short = 'o', long, value_enum, default_value_t = OutputMode::Text, help = "Output format: text, json, or path")]
    pub output: OutputMode,
}

impl std::ops::Deref for RunArgs {
    type Target = CommonArgs;
    fn deref(&self) -> &CommonArgs {
        &self.common
    }
}

#[derive(Debug, Args, Clone)]
pub struct VibeArgs {
    #[command(flatten)]
    pub common: CommonArgs,

    #[arg(long, help = "Disable occ-managed transcript context accumulation")]
    pub no_transcript: bool,
}

impl std::ops::Deref for VibeArgs {
    type Target = CommonArgs;
    fn deref(&self) -> &CommonArgs {
        &self.common
    }
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
    pub command: Option<ProfilesCommand>,
}

#[derive(Debug, Subcommand)]
pub enum ProfilesCommand {
    #[command(about = "List agents / 列出 agent")]
    List,
    #[command(about = "Show one agent as TOML / 以 TOML 展示单个 agent")]
    Show { name: String },
    #[command(about = "Render an agent command plan / 测试 agent 的命令计划")]
    Test { name: String },
    #[command(about = "Add one configured agent / 新增一个 agent 配置")]
    Add(Box<ProfileAddArgs>),
}

#[derive(Debug, Args)]
pub struct ProfileAddArgs {
    pub name: String,

    #[arg(long = "cli", value_name = "CLI", help = "CLI type for this agent")]
    pub backend: String,

    #[arg(long, value_delimiter = ',', help = "Comma-separated agent aliases")]
    pub aliases: Vec<String>,

    #[arg(long, help = "Command name, e.g. claude")]
    pub command: Option<String>,

    #[arg(long, help = "Executable path overriding command")]
    pub path: Option<PathBuf>,

    #[arg(long, help = "Agent model")]
    pub model: Option<String>,

    #[arg(long, alias = "reasoning-effort", help = "Agent reasoning effort")]
    pub effort: Option<String>,

    #[arg(
        long,
        value_name = "DIR",
        help = "Per-agent CLI system config directory"
    )]
    pub config_dir: Option<PathBuf>,

    #[arg(
        long,
        help = "Explicitly use strict isolated env (default for new agents)"
    )]
    pub strict_env: bool,

    #[arg(
        long,
        conflicts_with = "strict_env",
        help = "Use the default inherited CLI environment instead of isolated strict env"
    )]
    pub inherit_env: bool,

    #[arg(
        long = "env-allow",
        value_name = "KEY",
        value_delimiter = ',',
        help = "Parent environment variable allowed in strict isolated env mode"
    )]
    pub env_allow: Vec<String>,

    #[arg(
        long,
        value_name = "KEY=VALUE",
        help = "Agent-specific environment variable"
    )]
    pub env: Vec<String>,

    #[arg(long, conflicts_with = "project", help = "Write to ~/.occ/config.toml")]
    pub user: bool,

    #[arg(long, conflicts_with = "user", help = "Write to .occ/config.toml")]
    pub project: bool,

    #[arg(long, help = "Set this agent as default_agent")]
    pub set_default: bool,

    #[arg(long, help = "Set this agent as the default for its CLI type")]
    pub set_cli_default: bool,
}

#[derive(Debug, Args)]
pub struct BackendsArgs {
    #[command(subcommand)]
    pub command: Option<BackendsCommand>,
}

#[derive(Debug, Subcommand)]
pub enum BackendsCommand {
    #[command(about = "List CLIs / 列出 CLI")]
    List,
    #[command(about = "Show CLI capabilities / 展示 CLI 能力")]
    Show { name: String },
}

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: Option<ConfigCommand>,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    #[command(about = "Create a sample config file / 创建示例配置文件")]
    Init {
        #[arg(
            long,
            conflicts_with = "project",
            help = "Create user-level config at ~/.occ/config.toml"
        )]
        user: bool,

        #[arg(
            long,
            conflicts_with = "user",
            help = "Create project-level config at .occ/config.toml"
        )]
        project: bool,

        #[arg(long, help = "Overwrite an existing config file")]
        force: bool,
    },
    #[command(about = "Print config search paths / 打印配置搜索路径")]
    Path,
    #[command(about = "Explain the effective config / 解释当前生效配置")]
    Show {
        #[arg(long, help = "Print the raw TOML instead of an explained summary")]
        raw: bool,
    },
    #[command(about = "Validate config semantics / 校验配置语义")]
    Validate,
    #[command(about = "Open editable config UI / 打开配置编辑 UI")]
    Ui {
        #[arg(long, help = "Output path for the generated HTML file")]
        output: Option<PathBuf>,
    },
    #[command(about = "Open form-based config UI in browser / 在浏览器中打开表单式配置 UI")]
    Html {
        #[arg(
            long,
            help = "Persist saves to this file (defaults to last loaded config or project config path)"
        )]
        save_to: Option<PathBuf>,

        #[arg(
            long,
            help = "Bind port (defaults to a random localhost port)",
            value_name = "PORT"
        )]
        port: Option<u16>,

        #[arg(long, help = "Do not open the default browser automatically")]
        no_open: bool,
    },
    #[command(about = "Export standalone config HTML / 导出静态配置 HTML")]
    ExportHtml {
        #[arg(long, help = "Output path for the generated HTML file")]
        output: Option<PathBuf>,

        #[arg(long, value_enum, default_value_t = ConfigTarget::Loaded, help = "Config target: user, project, or loaded")]
        target: ConfigTarget,

        #[arg(long, help = "Open the exported HTML in the default browser")]
        open: bool,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum)]
pub enum ConfigTarget {
    User,
    Project,
    Loaded,
}

#[derive(Debug, Args)]
pub struct SessionsArgs {
    #[command(subcommand)]
    pub command: Option<SessionsCommand>,
}

#[derive(Debug, Subcommand)]
pub enum SessionsCommand {
    #[command(about = "List sessions / 列出会话")]
    List {
        #[arg(
            long,
            default_value_t = 20,
            help = "Maximum number of sessions to show"
        )]
        limit: usize,
    },
    #[command(about = "Show one session / 展示单个会话")]
    Show { session_id: String },
    #[command(about = "Resume a session / 恢复会话")]
    Resume(SessionResumeArgs),
    #[command(about = "Find the latest matching session / 查找最近匹配会话")]
    Latest(SessionLatestArgs),
    #[command(
        about = "Migrate legacy session-index.jsonl into SQLite / 迁移旧 JSONL 会话到 SQLite"
    )]
    Migrate,
}

#[derive(Debug, Args)]
pub struct SessionResumeArgs {
    pub session_id: String,

    #[arg(short = 'p', long, help = "Follow-up prompt text")]
    pub prompt: Option<String>,

    #[arg(long, help = "Read follow-up prompt from a file")]
    pub prompt_file: Option<PathBuf>,

    #[arg(long, help = "Read follow-up prompt from stdin")]
    pub stdin: bool,

    #[arg(short = 's', long, help = "Mirror child output to parent stderr")]
    pub stream: bool,

    #[arg(short = 'C', long, help = "Override working directory")]
    pub cwd: Option<PathBuf>,

    #[arg(short = 'm', long, help = "Override model")]
    pub model: Option<String>,

    #[arg(
        short = 'e',
        long,
        alias = "reasoning-effort",
        help = "Override reasoning effort"
    )]
    pub effort: Option<String>,

    #[arg(long, help = "Override the run artifact directory")]
    pub doc_root: Option<PathBuf>,

    #[arg(short = 'o', long, value_enum, default_value_t = OutputMode::Text, help = "Output format")]
    pub output: OutputMode,

    #[arg(long, help = "Show the command plan without executing")]
    pub dry_run: bool,

    #[arg(
        last = true,
        help = "Additional arguments passed through to the child CLI"
    )]
    pub child_args: Vec<String>,
}

#[derive(Debug, Args)]
pub struct SessionLatestArgs {
    #[arg(long = "agent", value_name = "AGENT", help = "Filter by agent")]
    pub profile: Option<String>,

    #[arg(long = "cli", value_name = "CLI", help = "Filter by CLI")]
    pub backend: Option<String>,

    #[arg(long, help = "Filter by working directory")]
    pub cwd: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct RunsArgs {
    #[command(subcommand)]
    pub command: Option<RunsCommand>,
}

#[derive(Debug, Subcommand)]
pub enum RunsCommand {
    #[command(about = "List runs / 列出运行记录")]
    List {
        #[arg(long, default_value_t = 20, help = "Maximum number of runs to show")]
        limit: usize,
    },
    #[command(about = "Show run metadata / 展示运行元数据")]
    Show { run_id: String },
    #[command(about = "Open or print run result / 打开或打印运行结果")]
    Open {
        run_id: String,

        #[arg(long, help = "Print the result path instead of opening it")]
        print: bool,
    },
}

#[derive(Debug, Args)]
pub struct SkillsArgs {
    #[command(subcommand)]
    pub command: Option<SkillsCommand>,
}

#[derive(Debug, Subcommand)]
pub enum SkillsCommand {
    #[command(about = "List bundled skills / 列出内置 skills")]
    List,
    #[command(about = "Show one skill / 展示单个 skill")]
    Show { name: String },
    #[command(about = "Export one skill / 导出单个 skill")]
    Export {
        name: String,

        #[arg(long, help = "Target directory for the exported skill")]
        target: PathBuf,
    },
    #[command(about = "Install all bundled skills / 安装所有内置 skills")]
    Install {
        #[arg(
            long,
            help = "Target directory for installed skills (defaults to ~/.agents/skills)"
        )]
        target: Option<PathBuf>,
    },
    #[command(about = "Check installed skills / 检查已安装 skills")]
    Doctor {
        #[arg(
            long,
            help = "Target directory to check (defaults to ~/.agents/skills)"
        )]
        target: Option<PathBuf>,
    },
}

#[derive(Debug, Args, Clone)]
pub struct SettingsArgs {
    #[arg(long, value_enum, default_value_t = ConfigTarget::User, help = "Config target: user, project, or loaded")]
    pub target: ConfigTarget,

    #[arg(
        long,
        help = "Compatibility flag; settings uses local server mode unless --output is set"
    )]
    pub server: bool,

    #[arg(
        long,
        help = "Bind port for the local settings server",
        value_name = "PORT"
    )]
    pub port: Option<u16>,

    #[arg(
        long,
        help = "Export static TOML config HTML instead of opening the live settings server"
    )]
    pub output: Option<PathBuf>,

    #[arg(long, help = "Do not open the browser automatically")]
    pub no_open: bool,
}
