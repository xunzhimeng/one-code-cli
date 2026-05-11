use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

use crate::cli::{OutputMode, RunArgs, VibeArgs};
use crate::error::{OccError, OccResult};
use crate::output;
use crate::runner;

#[derive(Debug, Clone)]
struct VibeMessage {
    role: &'static str,
    content: String,
}

pub fn start(config_arg: Option<&PathBuf>, args: VibeArgs) -> OccResult<()> {
    let initial_messages = read_initial_messages(&args)?;
    let mut state = VibeState::new(args.session.clone());

    print_banner(&args, &state)?;
    for message in initial_messages {
        send_message(config_arg, &args, &mut state, message)?;
    }

    let mut line = String::new();
    loop {
        print!("occ> ");
        io::stdout().flush().map_err(|error| {
            OccError::io("child_process_failed", "Failed to flush stdout", error)
        })?;
        line.clear();
        let bytes = io::stdin().read_line(&mut line).map_err(|error| {
            OccError::io(
                "invalid_prompt_source",
                "Failed to read console input",
                error,
            )
        })?;
        if bytes == 0 {
            break;
        }
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

    println!("bye");
    Ok(())
}

struct VibeState {
    session_id: Option<String>,
    transcript: Vec<VibeMessage>,
}

impl VibeState {
    fn new(session_id: Option<String>) -> Self {
        Self {
            session_id,
            transcript: Vec::new(),
        }
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
    println!("One Code CLI vibe");
    println!("type /help for commands, /exit to quit");
    if let Some(profile) = &args.profile {
        println!("profile: {}", profile);
    }
    if let Some(backend) = &args.backend {
        println!("backend: {}", backend);
    }
    if let Some(model) = &args.model {
        println!("model: {}", model);
    }
    if let Some(cwd) = &args.cwd {
        println!("cwd: {}", output::display_path(cwd));
    }
    if let Some(session_id) = &state.session_id {
        println!("session_id: {}", session_id);
    }
    if args.no_transcript {
        println!("transcript: off");
    } else if args.resume {
        println!("transcript: native resume mode");
    } else {
        println!("transcript: occ-managed prompt context");
    }
    println!();
    Ok(())
}

fn handle_command(message: &str, state: &mut VibeState) -> OccResult<bool> {
    match message.trim() {
        "/help" => {
            println!("commands:");
            println!("  /help      show commands");
            println!("  /session   show current session id");
            println!("  /clear     clear occ-managed transcript context");
            println!("  /exit      quit");
            Ok(true)
        }
        "/session" => {
            println!("session_id: {}", state.session_id.as_deref().unwrap_or(""));
            Ok(true)
        }
        "/clear" => {
            state.transcript.clear();
            println!("transcript cleared");
            Ok(true)
        }
        _ => Ok(false),
    }
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
        profile: args.profile.clone(),
        backend: args.backend.clone(),
        model: args.model.clone(),
        cwd: args.cwd.clone(),
        prompt: Some(prompt),
        prompt_file: None,
        stdin: false,
        interactive: false,
        non_interactive: true,
        session: state.session_id.clone(),
        resume: args.resume,
        doc_root: args.doc_root.clone(),
        output: OutputMode::Path,
        timeout: args.timeout.clone(),
        dry_run: args.dry_run,
        child_args: args.child_args.clone(),
    };

    let Some(execution) = runner::run_once(config_arg, run_args)? else {
        println!("dry-run complete");
        return Ok(());
    };

    state.session_id = Some(execution.body.session_id.clone());
    let assistant_message = child_message(&execution.stdout, &execution.stderr);
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
    prompt
}

fn child_message(stdout: &str, stderr: &str) -> String {
    let stdout = stdout.trim_end();
    let stderr = stderr.trim_end();
    if !stdout.trim().is_empty() && !stderr.trim().is_empty() {
        format!("{}\n\n[stderr]\n{}", stdout, stderr)
    } else if !stdout.trim().is_empty() {
        stdout.to_string()
    } else if !stderr.trim().is_empty() {
        stderr.trim_end().to_string()
    } else {
        "No output.".to_string()
    }
}

fn render_user_message(message: &str) {
    println!("\n--- user ---");
    println!("{}", message.trim_end());
}

fn render_assistant_message(execution: &runner::RunExecution, message: &str) {
    println!(
        "\n--- assistant [{} | {}] ---",
        execution.body.backend, execution.body.run_id
    );
    println!("{}", message.trim_end());
    println!("\n--- run ---");
    println!("success: {}", execution.body.success);
    println!("session_id: {}", execution.body.session_id);
    println!(
        "result_path: {}",
        output::display_path(&execution.body.result_path)
    );
    if let Some(error) = &execution.body.error {
        println!("error: {}: {}", error.code, error.message);
    }
    println!();
}
