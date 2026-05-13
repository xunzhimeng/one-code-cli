# One Code CLI (`occ`)

> 中文 | [English](#english)

[![CI](https://github.com/xunzhimeng/one-code-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/xunzhimeng/one-code-cli/actions/workflows/ci.yml)
[![Release](https://github.com/xunzhimeng/one-code-cli/actions/workflows/release.yml/badge.svg)](https://github.com/xunzhimeng/one-code-cli/actions/workflows/release.yml)
[![GitHub Release](https://img.shields.io/github/v/release/xunzhimeng/one-code-cli?include_prereleases&sort=semver)](https://github.com/xunzhimeng/one-code-cli/releases)
[![GitHub Stars](https://img.shields.io/github/stars/xunzhimeng/one-code-cli?style=social)](https://github.com/xunzhimeng/one-code-cli/stargazers)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

`occ` 是一个统一的 coding-agent CLI 调度器。它让一个 AI agent 可以通过统一协议调用另一个 CLI，例如 Claude Code、Codex CLI、opencode、Gemini CLI，并把每次运行默认记录到用户 `~/.occ/` 下的结构化文件。

## 特性

- **统一入口**：用 `occ run --cli <name>` 调用不同 coding-agent CLI（`--backend` 仍兼容）。
- **面向其它 AI**：内置 `using-one-code-cli` skill，方便 Codex/Claude/Gemini/其它 agent 快速接入。
- **自动化友好**：提供 prompt 时走非交互运行，输出 JSON，并写入 `result.md`、`command.json`、`run.toml`。
- **默认前台交互**：未提供 prompt 时默认将子 CLI 接到当前终端；也可显式使用 `--interactive`。
- **默认高权限自动模式**：内置 target 会传递各 CLI 的免确认/高权限参数。
- **Windows 友好**：Windows 默认使用 npm shim 的 `.cmd` 入口，并对 Claude/Codex 通过 stdin 传 prompt，避免长 Markdown 参数触发 batch 问题。
- **代理转发**：默认转发 `HTTP_PROXY`、`HTTPS_PROXY`、`ALL_PROXY`、`NO_PROXY` 及小写变体。
- **配置 UI**：支持本地可保存配置 UI，也支持静态 HTML 导出。

## 安装

### 方式一：npm 全局安装（推荐）

```powershell
npm install -g one-code-cli
occ config init --user
occ config validate
```

npm 包名为 [`one-code-cli`](https://www.npmjs.com/package/one-code-cli)，安装时会自动从 GitHub Release 下载对应平台的原生二进制（Windows / Linux / macOS Intel / macOS Apple Silicon）。

### 方式二：下载 GitHub Release 二进制

到 [Releases](https://github.com/xunzhimeng/one-code-cli/releases) 下载对应平台的压缩包：

- Windows：`one-code-cli-x86_64-pc-windows-msvc.zip`
- Linux：`one-code-cli-x86_64-unknown-linux-gnu.tar.gz`
- macOS Intel：`one-code-cli-x86_64-apple-darwin.tar.gz`
- macOS Apple Silicon：`one-code-cli-aarch64-apple-darwin.tar.gz`

Windows 解压后把 `occ.exe` 所在目录加入 `PATH`。macOS / Linux 解压后把 `occ` 放到 `~/.local/bin`、`/usr/local/bin` 或其它 PATH 目录。

### 方式三：使用 Cargo 从源码安装

需要本机有 Rust 工具链：

```powershell
cargo install --git https://github.com/xunzhimeng/one-code-cli.git --locked
occ config init --user
occ config validate
```

安装指定版本：

```powershell
cargo install --git https://github.com/xunzhimeng/one-code-cli.git --tag v0.1.0 --locked
```

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=xunzhimeng/one-code-cli&type=Date)](https://www.star-history.com/#xunzhimeng/one-code-cli&Date)

## 子 CLI 要求

按需安装并登录这些 CLI：

- **Claude Code**：`claude`
- **Codex CLI**：`codex`
- **opencode**：`opencode`
- **Gemini CLI**：`gemini`

检查命令：

```powershell
Get-Command occ, claude, codex, opencode, gemini -ErrorAction SilentlyContinue
```

Windows 上，如果这些 CLI 是 npm 安装的，`occ` 默认使用 `.cmd` shim，例如 `claude.cmd`、`codex.cmd`。

## 给其它 AI 快速接入

`occ` 内置一个 agent-facing skill：`using-one-code-cli`。

### 通用安装

```powershell
occ skills install --target "$HOME\.agents\skills"
occ skills doctor --target "$HOME\.agents\skills"
```

### Codex CLI 安装

不同 Codex 版本可能扫描不同 skill 目录。建议两个都装：

```powershell
occ skills install --target "$HOME\.codex\skills"
occ skills install --target "$HOME\.codex\superpowers\skills"
occ skills doctor --target "$HOME\.codex\skills"
occ skills doctor --target "$HOME\.codex\superpowers\skills"
```

之后可以在 Codex 中这样说：

```text
Use the using-one-code-cli skill. Delegate the task through occ and read result_path.
```

查看 skill 内容：

```powershell
occ skills show using-one-code-cli
```

## 基本使用

按 CLI 调用：

```powershell
occ run --cli claude --prompt "Reply with exactly OK" --non-interactive --output json
occ run --cli codex --prompt "Reply with exactly OK" --non-interactive --output json
```

长任务推荐通过 stdin 传入，避免额外 prompt 文件：

```powershell
@"
Long task prompt...
"@ | occ run --cli claude --cwd "E:\project\repo" --stdin --non-interactive --stream --output json
```

JSON 输出包含：

- **`result_path`**：最终 Markdown 结果，优先读取
- **`metadata_path`**：运行元数据
- **`run_id` / `session_id`**：运行与会话 ID
- **`exit_code`**：子进程退出码

## 后台与前台

未提供 `--prompt`、`--prompt-file` 或 `--stdin` 时，`occ run` 默认进入前台交互模式：

```powershell
occ run --cli claude --cwd "E:\project\repo"
occ run --cli codex --cwd "E:\project\repo"
```

前台交互会继承当前终端的 stdin/stdout/stderr。

提供 prompt 后，`occ run` 进入非交互自动执行模式；也建议显式加上 `--non-interactive` 表达意图：

- 捕获 stdout/stderr
- 默认写入 `~/.occ/runs/<run_id>/result.md`
- 默认写入 `~/.occ/runs/<run_id>/command.json`
- 不显示子 CLI TUI

这适合其它 AI agent 和自动化脚本。需要实时观察子进程输出时，加 `--stream`，它会把子进程 stdout/stderr 镜像到父进程 stderr，同时保留日志和最终 JSON stdout。

显式前台交互模式：

```powershell
occ run --cli claude --interactive
occ run --cli codex --interactive
```

`--interactive` 会继承当前终端的 stdin/stdout/stderr。

`occ` 当前不会自动打开新的外部终端窗口。如果需要独立可见窗口，可用外部 PowerShell/Windows Terminal wrapper 启动 `occ`。

## 默认高权限自动模式

默认 target 面向本地自动化委派，权限较高。只在你信任的目录中使用。

Windows 默认非交互命令形态：

### Claude Code

```text
claude.cmd --print --dangerously-skip-permissions
```

Prompt 通过 stdin 传入，避免 Windows `.cmd` 长参数问题。

### Codex CLI

```text
codex.cmd exec --dangerously-bypass-approvals-and-sandbox --skip-git-repo-check
```

Prompt 通过 stdin 传入。

### opencode

```text
opencode.cmd run --dangerously-skip-permissions <prompt>
```

### Gemini CLI

```text
gemini.cmd --yolo --skip-trust -p <prompt>
```

非 Windows 平台使用 `claude`、`codex`、`opencode`、`gemini`，不带 `.cmd`。

## dry-run 验证

执行前检查最终命令：

```powershell
occ run --cli claude --prompt "test" --non-interactive --dry-run --output json
occ run --cli codex --prompt "test" --non-interactive --dry-run --output json
```

重点看：

- `command.executable`
- `command.args`
- `command.prompt_via_stdin`

## 本地 smoke test

Claude Code：

```powershell
occ run --cli claude --prompt "Reply with exactly OCC_CLAUDE_FINAL_OK" --non-interactive --output json --timeout 90s
```

Codex CLI：

```powershell
occ run --cli codex --prompt "Reply with exactly OCC_CODEX_FINAL_OK" --non-interactive --output json --timeout 180s
```

两者都应返回 `success = true`，然后读取 `result_path`。

## 配置

配置搜索顺序：

1. `<cwd>/.occ.toml`
2. `<cwd>/.occ/config.toml`
3. `~/.occ/config.toml`
4. 内置默认配置

运行记录默认写入用户 `~/.occ/runs/`。如果需要项目内记录，可在配置里设置 `doc_root = ".occ"`，或在命令中传 `--doc-root <path>`。

`occ config show` 默认输出带解释的配置概览；需要完整 TOML 时使用：

```powershell
occ config show --raw
```

说明性输出会读取 `OCC_LANG`、`LANGUAGE`、`LC_ALL`、`LC_MESSAGES`、`LANG`，例如：

```powershell
$env:OCC_LANG="zh-CN"
occ config show
$env:OCC_LANG="en-US"
occ config show
```

常用命令：

```powershell
occ config show
occ config validate
occ config ui
occ config export-html
```

这些容器命令不带子命令时会默认列出内容：

```powershell
occ targets
occ clis
occ sessions
occ runs
occ skills
```

## vibe slash 命令

`occ vibe` 进入连续对话模式。对话中可用 `/` 命令切换 CLI 或查看状态：

```text
/help
/status
/target claude
/cli codex
/model gpt-5.4
/model
/session
/clear
/exit
```

`/target` 和 `/cli` 会清空当前 session 与本地 transcript，避免把不同 CLI 的上下文混在一起（旧的 `/profile`、`/backend` 仍兼容）。

## 代理转发

默认启用代理转发。`occ` 会把这些环境变量转发给子 CLI：

- `HTTP_PROXY`
- `HTTPS_PROXY`
- `ALL_PROXY`
- `NO_PROXY`
- 小写变体

关闭：

```toml
[proxy]
enabled = false
```

---

## English

`occ` is a unified dispatcher for coding-agent CLIs. It lets one AI agent delegate work to another CLI such as Claude Code, Codex CLI, opencode, or Gemini CLI, then records each run as structured files under the user's `~/.occ/` by default.

## Features

- **One command for multiple agents**: dispatch with `occ run --cli <name>` (`--backend` remains compatible).
- **Agent-friendly**: ships with the `using-one-code-cli` skill for other AI agents.
- **Automation-friendly**: prompt-driven runs are non-interactive with JSON output and structured artifacts.
- **Foreground by default**: runs without a prompt attach the child CLI to the current terminal; `--interactive` is also supported.
- **High-permission defaults**: default targets pass each CLI's auto-approval / high-permission flags.
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
occ skills install --target "$HOME\.agents\skills"
occ skills doctor --target "$HOME\.agents\skills"
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
```

For long inline tasks:

```powershell
@"
Long task prompt...
"@ | occ run --cli claude --cwd "E:\project\repo" --stdin --non-interactive --stream --output json
```

Read `result_path` first from the JSON response.

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

Use `--stream` to mirror child stdout/stderr to the parent stderr while preserving logs and JSON stdout.

Foreground mode:

```powershell
occ run --cli claude --interactive
occ run --cli codex --interactive
```

## High-permission defaults

Default targets are designed for trusted local automation.

Windows command shapes:

- **Claude Code**: `claude.cmd --print --dangerously-skip-permissions`
- **Codex CLI**: `codex.cmd exec --dangerously-bypass-approvals-and-sandbox --skip-git-repo-check`
- **opencode**: `opencode.cmd run --dangerously-skip-permissions <prompt>`
- **Gemini CLI**: `gemini.cmd --yolo --skip-trust -p <prompt>`

On non-Windows platforms, executable names do not use `.cmd`.

## Configuration

`occ config show` prints an explained summary by default. Use raw mode for the complete TOML:

```powershell
occ config show --raw
```

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
occ config ui
occ config export-html
```

Container commands default to list mode when no subcommand is provided:

```powershell
occ targets
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
/target claude
/cli codex
/model gpt-5.4
/model
/session
/clear
/exit
```

`/target` and `/cli` clear the current session and local transcript so different CLIs do not share stale context (`/profile` and `/backend` remain compatible).

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=xunzhimeng/one-code-cli&type=Date)](https://www.star-history.com/#xunzhimeng/one-code-cli&Date)
