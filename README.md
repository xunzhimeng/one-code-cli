# One Code CLI (`occ`)

> English | [中文](README.zh-CN.md)

[![CI](https://github.com/xunzhimeng/one-code-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/xunzhimeng/one-code-cli/actions/workflows/ci.yml)
[![Release](https://github.com/xunzhimeng/one-code-cli/actions/workflows/release.yml/badge.svg)](https://github.com/xunzhimeng/one-code-cli/actions/workflows/release.yml)
[![GitHub Release](https://img.shields.io/github/v/release/xunzhimeng/one-code-cli?include_prereleases&sort=semver)](https://github.com/xunzhimeng/one-code-cli/releases)
[![GitHub Stars](https://img.shields.io/github/stars/xunzhimeng/one-code-cli?style=social)](https://github.com/xunzhimeng/one-code-cli/stargazers)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

`occ` is a unified dispatcher for coding-agent CLIs. It lets one AI agent delegate work to another CLI such as Claude Code, Codex CLI, opencode, or Gemini CLI, then records each run as structured files under the user's `~/.occ/` by default.

## Features

- **One command for multiple agents**: dispatch with `occ run --cli <name>`.
- **Agent-friendly**: ships with the `using-one-code-cli` skill for other AI agents.
- **Automation-friendly**: prompt-driven runs are non-interactive with JSON output and structured artifacts.
- **Foreground by default**: runs without a prompt attach the child CLI to the current terminal; `--interactive` is also supported.
- **High-permission defaults**: default agents pass each CLI's auto-approval / high-permission flags.
- **Windows-friendly**: defaults to `.cmd` npm shims and sends Claude/Codex prompts through stdin.
- **Proxy forwarding**: forwards common proxy environment variables by default.
- **Config UI**: local save-capable UI and static HTML export are supported.

## Installation

### Option 1: Install globally with npm (recommended)

```powershell
npm install -g one-code-cli
occ config init --user
occ config validate
```

The package is published as [`one-code-cli`](https://www.npmjs.com/package/one-code-cli). The install script downloads the matching native binary for your platform (Windows / Linux / macOS Intel / macOS Apple Silicon) from GitHub Releases.

### Option 2: Download GitHub Release binaries

Go to [Releases](https://github.com/xunzhimeng/one-code-cli/releases) and download the archive for your platform:

- Windows: `one-code-cli-x86_64-pc-windows-msvc.zip`
- Linux: `one-code-cli-x86_64-unknown-linux-gnu.tar.gz`
- macOS Intel: `one-code-cli-x86_64-apple-darwin.tar.gz`
- macOS Apple Silicon: `one-code-cli-aarch64-apple-darwin.tar.gz`

Extract the archive and put `occ` / `occ.exe` on your `PATH`.

### Option 3: Install from source with Cargo

Requires a local Rust toolchain:

```powershell
cargo install --git https://github.com/xunzhimeng/one-code-cli.git --locked
occ config init --user
occ config validate
```

Install a specific tag:

```powershell
cargo install --git https://github.com/xunzhimeng/one-code-cli.git --tag v0.1.0 --locked
```

## Quick setup for AI agents

Install the built-in skill:

```powershell
occ skills install
occ skills doctor
```

For Codex CLI:

```powershell
occ skills install --target "$HOME\.codex\skills"
occ skills install --target "$HOME\.codex\superpowers\skills"
occ skills doctor --target "$HOME\.codex\skills"
occ skills doctor --target "$HOME\.codex\superpowers\skills"
```

Then ask the agent:

```text
Use the using-one-code-cli skill. Delegate the task through occ and read result_path.
```

## Usage

```powershell
occ run --cli claude --prompt "Reply with exactly OK" --non-interactive --output json
occ run --cli codex --prompt "Reply with exactly OK" --non-interactive --output json
occ run --cli codex --prompt "Reply with exactly OK" --model gpt-5.4 --non-interactive --output json
occ run --cli codex --prompt "Reply with exactly OK" --model gpt-5.4 --effort xhigh --non-interactive --output json
```

Run the same task across multiple agents in parallel:

```powershell
occ run --agents claude-cc,deepseek-cc --prompt "Review this repository" --stream --output json
occ run --agents claude-cc,deepseek-cc --prompt "Review this repository" --dry-run --output json
```

Multi-agent output returns `batch_id` and `runs[]`. Each run still has its own `run_id`, `session_id`, `result_path`, and `metadata_path`. With `--stream`, live output is prefixed with `[agent]` and empty/control-only noise is filtered while raw stdout/stderr logs are preserved per run. `--agents` selects exact agents and does not go through the `--cli` default mapping.

For long inline tasks:

```powershell
@"
Long task prompt...
"@ | occ run --cli claude --cwd "E:\project\repo" --stdin --non-interactive --stream --output json
```

Read `result_path` first from the JSON response. When selection matters, also inspect `model` / `model_source` and `effort` / `effort_source`. For multi-agent runs, iterate over `runs[]`.

## Dry-run checks

Inspect the final command plan before executing:

```powershell
occ run --cli claude --prompt "test" --non-interactive --dry-run --output json
occ run --cli codex --prompt "test" --non-interactive --dry-run --output json
occ run --agents claude-cc,deepseek-cc --prompt "test" --dry-run --output json
```

Check `command.executable`, `command.args`, `context.model`, `model_source`, `context.effort`, `effort_source`, and `command.prompt_via_stdin`. For multi-agent dry-runs, also check `runs[].agent`, `runs[].context.cli`, and `runs[].command.env_keys` to confirm each agent stays isolated.

## Foreground vs background

Without `--prompt`, `--prompt-file`, or `--stdin`, `occ run` defaults to foreground interactive mode:

```powershell
occ run --cli claude --cwd "E:\project\repo"
occ run --cli codex --cwd "E:\project\repo"
```

Prompt-driven runs use non-interactive automation:

- Captures stdout/stderr
- Writes `~/.occ/runs/<run_id>/result.md` by default
- Writes `~/.occ/runs/<run_id>/command.json` by default
- Does not show the child CLI TUI

Use `--stream` to mirror child stdout/stderr to the parent stderr while preserving logs and JSON stdout. Multi-agent streams are automatically prefixed by agent.

Foreground mode:

```powershell
occ run --cli claude --interactive
occ run --cli codex --interactive
```

## High-permission defaults

Default agents are designed for trusted local automation.

Windows command shapes:

- **Claude Code**: `claude.cmd --print --dangerously-skip-permissions`
- **Codex CLI**: `codex.cmd exec --dangerously-bypass-approvals-and-sandbox --skip-git-repo-check`
- **opencode**: `opencode.cmd run --dangerously-skip-permissions <prompt>`
- **Gemini CLI**: `gemini.cmd --yolo --skip-trust --prompt <prompt>`

On non-Windows platforms, executable names do not use `.cmd`.

## Configuration

`occ config show` prints an explained summary by default. Use raw mode for the complete TOML:

```powershell
occ config show --raw
```

Add a configured agent quickly:

```powershell
occ agents add deepseek-cc --cli claude --model deepseek-chat `
  --env ANTHROPIC_BASE_URL=https://api.deepseek.com/anthropic `
  --env-allow HTTPS_PROXY `
  --set-cli-default
occ agents show deepseek-cc
occ agents test deepseek-cc
```

`--cli` writes `cli_type` in config. Repeat `--env KEY=VALUE` for agent-specific environment variables; it is not comma-separated. New agents default to strict isolated env: occ clears the inherited process environment before launching the child CLI, then rebuilds it from a small launcher-safe built-in allowlist, configured proxy variables, `--env-allow KEY`, automatic isolation variables, and agent `env`. Use `--inherit-env` only when you intentionally want the child CLI to use the default inherited machine environment; in that mode occ does not create or set `config_dir` unless you explicitly pass `--config-dir`. `--set-cli-default` updates `[cli_type_defaults]` so `occ run --cli claude ...` selects this agent by default, while `--set-default` updates global `default_agent`. Without `--config` or `--project`, `agents add` writes to `~/.occ/config.toml`.

By default, `agents add` also writes and creates an isolated `config_dir` for the agent. Override it with `--config-dir <dir>` when you want to point at an existing CLI system directory.

One CLI can have many agents with isolated CLI system config, model, base URL, env, and arguments:

```toml
[[agents]]
name = "claude-cc"
cli_type = "claude"
command = "claude"
config_dir = "C:/Users/you/.occ/agents/claude-cc/system"
model = "claude-sonnet-4-5"
args_strategy = "builtin"
prompt_via = "stdin"

[[agents]]
name = "deepseek-cc"
cli_type = "claude"
command = "claude"
config_dir = "C:/Users/you/.occ/agents/deepseek-cc/system"
model = "deepseek-chat"
env_mode = "strict"
env_allowlist = ["HTTPS_PROXY"]
env = { ANTHROPIC_BASE_URL = "https://api.deepseek.com/anthropic" }
args_strategy = "builtin"
prompt_via = "stdin"
```

`config_dir` is passed to the child CLI as its own system root:

- Claude Code: `CLAUDE_CONFIG_DIR=<config_dir>`
- Codex CLI: `CODEX_HOME=<config_dir>`
- opencode: `OPENCODE_CONFIG_DIR=<config_dir>`
- Gemini CLI: `HOME=<config_dir>`; on Windows, `USERPROFILE`, `APPDATA`, `LOCALAPPDATA`, `HOMEDRIVE`, and `HOMEPATH` are also isolated.

Agent `env = { ... }` values override these automatic isolation variables when the same key is set. Relative `config_dir` values are resolved against the run working directory; the settings UI writes suggested isolated directories as stable paths next to the selected config file.

Environment mode defaults to `inherit` for built-in/default CLI agents, but `occ agents add` and the settings UI create isolated agents with `env_mode = "strict"` by default. Use strict mode when different agents must not see parent `ANTHROPIC_*`, `OPENAI_*`, or other provider variables. In strict mode, `env_allowlist = ["KEY"]` copies only named parent variables in addition to the built-in launcher variables. When `[proxy].enabled = true`, `proxy.env_keys` are forwarded even in strict mode; when disabled, those proxy keys are removed. `env = { ... }` always wins over inherited or proxy-forwarded values. In the settings UI, choose **Use default CLI system directory** to clear `config_dir` and inherit the normal CLI environment, or **Use isolated config_dir** to set the per-agent CLI system root and strict env together.

Selection rules:

- `occ run --agent deepseek-cc ...` selects one exact agent.
- `occ run --agents claude-cc,deepseek-cc ...` selects multiple exact agents in parallel.
- `occ run --cli claude ...` uses `cli_type_defaults.claude`; if unset, it uses the first agent whose `cli_type = "claude"`.
- Agent aliases are alternate names for one exact agent and work with `--agent`, `--agents`, and `/agent`.
- CLI type aliases are alternate names for CLI types and work with `--cli` and `/cli`; identity aliases like `codex = "codex"` are redundant.

Resume support uses each CLI's native mechanism when available. Claude Code and Gemini receive a generated native UUID on new runs so later `--resume` can target the same native session; Codex and opencode use their current CLI resume/continue commands. `--session <id>` always stays bound to the session's original agent; passing a conflicting `--agent` or `--cli` returns `session_agent_mismatch`.

Explanatory output reads `OCC_LANG`, `LANGUAGE`, `LC_ALL`, `LC_MESSAGES`, and `LANG`:

```powershell
$env:OCC_LANG="en-US"
occ config show
$env:OCC_LANG="zh-CN"
occ config show
```

```powershell
occ config show
occ config validate
occ settings
occ settings --output config-ui.html --no-open
occ config export-html
```

`occ settings` opens the full form-based settings UI through a local server so save/reload/TOML sync work like the interactive editor. Use `--output` only when you want the simpler standalone TOML HTML export.

Container commands default to list mode when no subcommand is provided:

```powershell
occ agents
occ clis
occ sessions
occ runs
occ skills
```

## Vibe Slash Commands

`occ vibe` supports slash commands during a chat:

```text
/help
/status
/agent claude
/cli codex
/model gpt-5.4
/model
/session
/clear
/exit
```

`/agent` and `/cli` clear the current session and local transcript so different CLIs do not share stale context.

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=xunzhimeng/one-code-cli&type=Date)](https://www.star-history.com/#xunzhimeng/one-code-cli&Date)
