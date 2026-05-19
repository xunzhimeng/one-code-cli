use std::env;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;

use colored::Colorize;
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Editor, Helper};

use crate::backend;
use crate::cli::{CommonArgs, OutputMode, RunArgs, VibeArgs};
use crate::config;
use crate::error::{OccError, OccResult};
use crate::i18n;
use crate::output;
use crate::runner;

#[derive(Debug, Clone)]
struct VibeMessage {
    role: &'static str,
    content: String,
}

pub fn start(config_arg: Option<&PathBuf>, args: VibeArgs) -> OccResult<()> {
    let initial_messages = read_initial_messages(&args)?;
    let mut state = VibeState::new(&args);
    let completion = VibeCompletion::from_config(config_arg, &args);
    let mut input = VibeInput::new(completion)?;

    print_banner(&args, &state)?;
    for message in initial_messages {
        send_message(config_arg, &args, &mut state, message)?;
    }

    loop {
        let Some(line) = input.read_line()? else {
            break;
        };
        let message = line.trim_end_matches(&['\r', '\n'][..]);
        if message.trim().is_empty() {
            continue;
        }
        if matches!(message.trim(), "/exit" | "/quit") {
            break;
        }
        if handle_command(message, &mut state)? {
            continue;
        }
        send_message(config_arg, &args, &mut state, message.to_string())?;
    }

    println!("{}", i18n::t("vibe.bye"));
    Ok(())
}

type VibeEditor = Editor<VibeLineHelper, rustyline::history::DefaultHistory>;

enum VibeInput {
    Editor(Box<VibeEditor>),
    Plain(String),
}

impl VibeInput {
    fn new(completion: VibeCompletion) -> OccResult<Self> {
        if io::stdin().is_terminal() && io::stdout().is_terminal() {
            let mut editor = VibeEditor::new().map_err(|error| {
                OccError::new(
                    "invalid_prompt_source",
                    format!("Failed to initialize console input: {}", error),
                )
            })?;
            editor.set_helper(Some(VibeLineHelper { completion }));
            Ok(Self::Editor(Box::new(editor)))
        } else {
            Ok(Self::Plain(String::new()))
        }
    }

    fn read_line(&mut self) -> OccResult<Option<String>> {
        match self {
            Self::Editor(editor) => match editor.readline("occ❯ ") {
                Ok(line) => {
                    if !line.trim().is_empty() {
                        let _ = editor.add_history_entry(line.as_str());
                    }
                    Ok(Some(line))
                }
                Err(ReadlineError::Interrupted) => Ok(Some(String::new())),
                Err(ReadlineError::Eof) => Ok(None),
                Err(error) => Err(OccError::new(
                    "invalid_prompt_source",
                    format!("Failed to read console input: {}", error),
                )),
            },
            Self::Plain(line) => {
                print!("{} ", "occ❯".cyan().bold());
                io::stdout()
                    .flush()
                    .map_err(|error| OccError::io("io_error", "Failed to flush stdout", error))?;
                line.clear();
                let bytes = io::stdin().read_line(line).map_err(|error| {
                    OccError::io(
                        "invalid_prompt_source",
                        "Failed to read console input",
                        error,
                    )
                })?;
                if bytes == 0 {
                    Ok(None)
                } else {
                    Ok(Some(line.clone()))
                }
            }
        }
    }
}

#[derive(Clone)]
struct VibeCompletion {
    profiles: Vec<String>,
    backends: Vec<String>,
}

impl VibeCompletion {
    fn from_config(config_arg: Option<&PathBuf>, args: &VibeArgs) -> Self {
        let cwd = args
            .cwd
            .clone()
            .or_else(|| env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        let loaded = config::load(config_arg, &cwd).ok();
        let mut profiles = Vec::new();
        if let Some(config) = &loaded {
            for profile in &config.profiles {
                profiles.push(profile.name.clone());
                profiles.extend(profile.aliases.iter().cloned());
            }
        }
        let mut backends = backend::all()
            .iter()
            .map(|backend| backend.name.to_string())
            .collect::<Vec<_>>();
        if let Some(config) = &loaded {
            backends.extend(config.backend_aliases.keys().cloned());
        }
        Self {
            profiles: sorted_unique(profiles),
            backends: sorted_unique(backends),
        }
    }

    fn complete_line(&self, line: &str, pos: usize) -> (usize, Vec<Pair>) {
        let prefix = &line[..pos];
        if !prefix.starts_with('/') {
            return (pos, Vec::new());
        }
        let first_token_finished = prefix.chars().any(char::is_whitespace);
        if !first_token_finished {
            return (0, command_pairs(prefix));
        }
        let command = prefix.split_whitespace().next().unwrap_or("");
        let start = prefix
            .rfind(char::is_whitespace)
            .map(|index| index + 1)
            .unwrap_or(0);
        let query = &prefix[start..];
        let values = match command {
            "/agent" => &self.profiles,
            "/cli" => &self.backends,
            _ => return (pos, Vec::new()),
        };
        let pairs = fuzzy_values(query, values)
            .into_iter()
            .map(|value| Pair {
                display: value.clone(),
                replacement: value,
            })
            .collect();
        (start, pairs)
    }

    fn hint_line(&self, line: &str, pos: usize) -> Option<String> {
        if pos != line.len() || !line.starts_with('/') {
            return None;
        }
        if !line.chars().any(char::is_whitespace) {
            if line.len() <= 1 {
                return None;
            }
            return command_names()
                .into_iter()
                .find(|candidate| candidate.starts_with(line) && candidate.len() > line.len())
                .map(|candidate| candidate[line.len()..].to_string());
        }
        let command = line.split_whitespace().next().unwrap_or("");
        let start = line
            .rfind(char::is_whitespace)
            .map(|index| index + 1)
            .unwrap_or(0);
        let query = &line[start..];
        if query.is_empty() {
            return None;
        }
        let values = match command {
            "/agent" => &self.profiles,
            "/cli" => &self.backends,
            _ => return None,
        };
        values
            .iter()
            .find(|value| value.starts_with(query) && value.len() > query.len())
            .map(|value| value[query.len()..].to_string())
    }
}

struct VibeLineHelper {
    completion: VibeCompletion,
}

impl Helper for VibeLineHelper {}
impl Validator for VibeLineHelper {}
impl Highlighter for VibeLineHelper {}

impl Completer for VibeLineHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        Ok(self.completion.complete_line(line, pos))
    }
}

impl Hinter for VibeLineHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<String> {
        self.completion.hint_line(line, pos)
    }
}

fn command_pairs(query: &str) -> Vec<Pair> {
    fuzzy_values(query, &command_names())
        .into_iter()
        .map(|name| Pair {
            display: command_display(&name),
            replacement: name,
        })
        .collect()
}

fn command_names() -> Vec<String> {
    [
        "/help", "/status", "/agent", "/cli", "/model", "/effort", "/session", "/clear", "/exit",
        "/quit",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn command_display(name: &str) -> String {
    let detail = match name {
        "/agent" => "select configured agent",
        "/cli" => "select coding CLI",
        "/model" => "set or clear model",
        "/effort" => "set or clear effort",
        "/status" => "show state",
        "/session" => "show session id",
        "/clear" => "clear transcript",
        "/exit" | "/quit" => "quit",
        _ => "show commands",
    };
    format!("{name}\t{detail}")
}

fn fuzzy_values(query: &str, candidates: &[String]) -> Vec<String> {
    let mut scored = candidates
        .iter()
        .filter_map(|candidate| fuzzy_score(query, candidate).map(|score| (score, candidate)))
        .collect::<Vec<_>>();
    scored.sort_by(|(left_score, left), (right_score, right)| {
        left_score.cmp(right_score).then_with(|| left.cmp(right))
    });
    scored
        .into_iter()
        .take(10)
        .map(|(_, candidate)| candidate.clone())
        .collect()
}

fn fuzzy_score(query: &str, candidate: &str) -> Option<usize> {
    let query = query.trim_start_matches('/').to_ascii_lowercase();
    let candidate_match = candidate.trim_start_matches('/').to_ascii_lowercase();
    if query.is_empty() {
        return Some(0);
    }
    if candidate_match.starts_with(&query) {
        return Some(candidate_match.len() - query.len());
    }
    if let Some(index) = candidate_match.find(&query) {
        return Some(50 + index);
    }
    let mut score = 100;
    let mut last_index = 0;
    for needle in query.chars() {
        let haystack = &candidate_match[last_index..];
        let found = haystack.find(needle)?;
        score += found;
        last_index += found + needle.len_utf8();
    }
    Some(score)
}

fn sorted_unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

struct VibeState {
    session_id: Option<String>,
    profile: Option<String>,
    backend: Option<String>,
    model: Option<String>,
    effort: Option<String>,
    transcript: Vec<VibeMessage>,
}

impl VibeState {
    fn new(args: &VibeArgs) -> Self {
        Self {
            session_id: args.session.clone(),
            profile: args.profile.clone(),
            backend: args.backend.clone(),
            model: args.model.clone(),
            effort: args.effort.clone(),
            transcript: Vec::new(),
        }
    }

    fn reset_after_cli_change(&mut self) {
        self.session_id = None;
        self.transcript.clear();
    }
}

fn read_initial_messages(args: &VibeArgs) -> OccResult<Vec<String>> {
    let count =
        args.prompt.is_some() as usize + args.prompt_file.is_some() as usize + args.stdin as usize;
    if count > 1 {
        return Err(OccError::new(
            "invalid_prompt_source",
            "Use only one of --prompt, --prompt-file, or --stdin.",
        ));
    }
    if let Some(prompt) = &args.prompt {
        return Ok(vec![prompt.clone()]);
    }
    if let Some(path) = &args.prompt_file {
        let text = fs::read_to_string(path).map_err(|error| {
            OccError::io(
                "invalid_prompt_source",
                format!(
                    "Failed to read prompt file '{}'",
                    output::display_path(path)
                ),
                error,
            )
        })?;
        return Ok(vec![text]);
    }
    if args.stdin {
        let mut text = String::new();
        io::stdin().read_to_string(&mut text).map_err(|error| {
            OccError::io(
                "invalid_prompt_source",
                "Failed to read prompt from stdin",
                error,
            )
        })?;
        return Ok(vec![text]);
    }
    Ok(Vec::new())
}

fn print_banner(args: &VibeArgs, state: &VibeState) -> OccResult<()> {
    println!(
        "{} {}",
        i18n::t("vibe.title").bold().cyan(),
        concat!("v", env!("CARGO_PKG_VERSION")).dimmed()
    );
    println!("{}", i18n::t("vibe.hint").dimmed());
    if let Some(profile) = &state.profile {
        println!("  {} {}", "agent:".dimmed(), profile.cyan());
    }
    if let Some(backend) = &state.backend {
        println!("  {} {}", "cli:".dimmed(), backend.cyan());
    }
    if let Some(model) = &state.model {
        println!("  {} {}", "model:".dimmed(), model.cyan());
    }
    if let Some(effort) = &state.effort {
        println!("  {} {}", "effort:".dimmed(), effort.cyan());
    }
    if let Some(cwd) = &args.cwd {
        println!(
            "  {} {}",
            "cwd:".dimmed(),
            output::display_path(cwd).dimmed()
        );
    }
    if let Some(session_id) = &state.session_id {
        println!("  {} {}", "session:".dimmed(), session_id);
    }
    let transcript_label = if args.no_transcript {
        i18n::t("vibe.transcript_off")
    } else if args.resume {
        i18n::t("vibe.transcript_resume")
    } else {
        i18n::t("vibe.transcript_managed")
    };
    println!("  {} {}", "transcript:".dimmed(), transcript_label);
    println!();
    Ok(())
}

fn handle_command(message: &str, state: &mut VibeState) -> OccResult<bool> {
    let trimmed = message.trim();
    let mut parts = trimmed.split_whitespace();
    let command = parts.next().unwrap_or("");
    match command {
        "/help" => {
            let t = i18n::t;
            println!("{}", t("vibe.help_title").bold());
            println!("  {}   {}", "/help".cyan(), t("vibe.help_help").dimmed());
            println!("  {}  {}", "/status".cyan(), t("vibe.help_status").dimmed());
            println!(
                "  {}  {}",
                "/agent <name>".cyan(),
                t("vibe.help_profile").dimmed()
            );
            println!(
                "  {}     {}",
                "/cli <name>".cyan(),
                t("vibe.help_backend").dimmed()
            );
            println!(
                "  {} {}",
                "/model <name>".cyan(),
                t("vibe.help_model_set").dimmed()
            );
            println!(
                "  {}  {}",
                "/model".cyan(),
                t("vibe.help_model_clear").dimmed()
            );
            println!(
                "  {} {}",
                "/effort <level>".cyan(),
                t("vibe.help_effort_set").dimmed()
            );
            println!(
                "  {} {}",
                "/effort".cyan(),
                t("vibe.help_effort_clear").dimmed()
            );
            println!(
                "  {} {}",
                "/session".cyan(),
                t("vibe.help_session").dimmed()
            );
            println!("  {}   {}", "/clear".cyan(), t("vibe.help_clear").dimmed());
            println!("  {}    {}", "/exit".cyan(), t("vibe.help_exit").dimmed());
            Ok(true)
        }
        "/status" => {
            print_status(state);
            Ok(true)
        }
        "/agent" => {
            let Some(profile) = parts.next() else {
                println!("{}: /agent <name>", "usage".dimmed());
                return Ok(true);
            };
            state.profile = Some(profile.to_string());
            state.backend = None;
            state.reset_after_cli_change();
            println!("  {} {}", "agent:".dimmed(), profile.cyan());
            println!(
                "  {} {}",
                "cli:".dimmed(),
                i18n::t("vibe.backend_cleared").dimmed()
            );
            Ok(true)
        }
        "/cli" => {
            let Some(backend) = parts.next() else {
                println!("{}: /cli <name>", "usage".dimmed());
                return Ok(true);
            };
            state.backend = Some(backend.to_string());
            state.profile = None;
            state.reset_after_cli_change();
            println!("  {} {}", "cli:".dimmed(), backend.cyan());
            println!(
                "  {} {}",
                "agent:".dimmed(),
                i18n::t("vibe.backend_cleared").dimmed()
            );
            Ok(true)
        }
        "/model" => {
            if let Some(model) = parts.next() {
                state.model = Some(model.to_string());
                println!("  {} {}", "model:".dimmed(), model.cyan());
            } else {
                state.model = None;
                println!(
                    "  {} {}",
                    "model:".dimmed(),
                    i18n::t("vibe.backend_cleared").dimmed()
                );
            }
            Ok(true)
        }
        "/effort" => {
            if let Some(effort) = parts.next() {
                state.effort = Some(effort.to_string());
                println!("  {} {}", "effort:".dimmed(), effort.cyan());
            } else {
                state.effort = None;
                println!(
                    "  {} {}",
                    "effort:".dimmed(),
                    i18n::t("vibe.backend_cleared").dimmed()
                );
            }
            Ok(true)
        }
        "/session" => {
            println!(
                "  {} {}",
                "session_id:".dimmed(),
                state.session_id.as_deref().unwrap_or("-")
            );
            Ok(true)
        }
        "/clear" => {
            state.transcript.clear();
            println!("  {} {}", "✓".green(), i18n::t("vibe.transcript_cleared"));
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn print_status(state: &VibeState) {
    println!("{}", i18n::t("vibe.status_title").bold());
    println!(
        "  {} {}",
        "agent:".dimmed(),
        state.profile.as_deref().unwrap_or("-").cyan()
    );
    println!(
        "  {} {}",
        "cli:".dimmed(),
        state.backend.as_deref().unwrap_or("-").cyan()
    );
    println!(
        "  {} {}",
        "model:".dimmed(),
        state.model.as_deref().unwrap_or("-").cyan()
    );
    println!(
        "  {} {}",
        "effort:".dimmed(),
        state.effort.as_deref().unwrap_or("-").cyan()
    );
    println!(
        "  {} {}",
        "session:".dimmed(),
        state.session_id.as_deref().unwrap_or("-")
    );
    println!("  {} {}", "transcript:".dimmed(), state.transcript.len());
}

fn send_message(
    config_arg: Option<&PathBuf>,
    args: &VibeArgs,
    state: &mut VibeState,
    user_message: String,
) -> OccResult<()> {
    render_user_message(&user_message);
    let prompt = build_prompt(args, state, &user_message);
    let run_args = RunArgs {
        common: CommonArgs {
            profile: state.profile.clone(),
            backend: state.backend.clone(),
            model: state.model.clone(),
            effort: state.effort.clone(),
            cwd: args.cwd.clone(),
            prompt: Some(prompt),
            prompt_file: None,
            stdin: false,
            session: state.session_id.clone(),
            resume: args.resume,
            doc_root: args.doc_root.clone(),
            timeout: args.timeout.clone(),
            dry_run: args.dry_run,
            child_args: args.child_args.clone(),
        },
        agents: Vec::new(),
        stream: false,
        interactive: false,
        non_interactive: true,
        output: OutputMode::Path,
    };

    let Some(execution) = runner::run_once(config_arg, run_args)? else {
        println!("dry-run complete");
        return Ok(());
    };

    state.session_id = Some(execution.body.session_id.clone());
    let assistant_message = child_message(&execution);
    render_assistant_message(&execution, &assistant_message);
    state.transcript.push(VibeMessage {
        role: "user",
        content: user_message,
    });
    state.transcript.push(VibeMessage {
        role: "assistant",
        content: assistant_message,
    });
    Ok(())
}

fn build_prompt(args: &VibeArgs, state: &VibeState, user_message: &str) -> String {
    if args.no_transcript || args.resume || state.transcript.is_empty() {
        return user_message.to_string();
    }

    let mut prompt = String::new();
    prompt.push_str("You are being controlled through One Code CLI vibe mode. Continue the coding conversation using this transcript.\n\n");
    prompt.push_str("# Transcript\n\n");
    for message in &state.transcript {
        prompt.push_str("## ");
        prompt.push_str(message.role);
        prompt.push_str("\n\n");
        prompt.push_str(message.content.trim());
        prompt.push_str("\n\n");
    }
    prompt.push_str("## user\n\n");
    prompt.push_str(user_message.trim());

    let char_count = prompt.chars().count();
    if char_count > 50_000 {
        eprintln!(
            "{} transcript is {} chars. Consider /clear to avoid context overflow.",
            "warning:".yellow().bold(),
            char_count
        );
    }

    prompt
}

fn child_message(execution: &runner::RunExecution) -> String {
    let stdout = execution.stdout.trim_end();
    let stderr = execution.stderr.trim_end();
    if !stdout.trim().is_empty() {
        stdout.to_string()
    } else if !execution.body.success && !stderr.trim().is_empty() {
        format!("[stderr]\n{}", stderr)
    } else if !stderr.trim().is_empty() {
        execution
            .body
            .result_path
            .parent()
            .map(|run_dir| {
                format!(
                    "No stdout. Stderr was captured in `{}`.",
                    output::display_path(&run_dir.join("stderr.log"))
                )
            })
            .unwrap_or_else(|| "No stdout. Stderr was captured in run artifacts.".to_string())
    } else {
        "No output.".to_string()
    }
}

fn render_user_message(message: &str) {
    println!("\n{}", "━━━ user ━━━".blue().bold());
    println!("{}", message.trim_end());
}

fn render_assistant_message(execution: &runner::RunExecution, message: &str) {
    println!(
        "\n{}",
        format!(
            "━━━ assistant [{} | {}] ━━━",
            execution.body.backend, execution.body.run_id
        )
        .green()
        .bold()
    );
    println!("{}", message.trim_end());
    println!("\n{}", "━━━ run ━━━".dimmed());
    println!(
        "  {} {}",
        "success:".bold(),
        if execution.body.success {
            "true".green().to_string()
        } else {
            "false".red().to_string()
        }
    );
    println!("  {} {}", "session:".bold(), execution.body.session_id);
    println!(
        "  {} {}",
        "model:".bold(),
        execution.body.model.as_deref().unwrap_or("-")
    );
    println!(
        "  {} {}",
        "model_source:".bold(),
        execution.body.model_source
    );
    println!(
        "  {} {}",
        "effort:".bold(),
        execution.body.effort.as_deref().unwrap_or("-")
    );
    println!(
        "  {} {}",
        "effort_source:".bold(),
        execution.body.effort_source
    );
    println!(
        "  {} {}",
        "result:".bold(),
        output::display_path(&execution.body.result_path).cyan()
    );
    if let Some(error) = &execution.body.error {
        println!(
            "  {} {}: {}",
            "error:".red().bold(),
            error.code,
            error.message
        );
    }
    println!();
}
