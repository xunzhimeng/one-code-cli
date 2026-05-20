# One Code CLI (`occ`)

> [English](README.md) | 中文

[![CI](https://github.com/xunzhimeng/one-code-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/xunzhimeng/one-code-cli/actions/workflows/ci.yml)
[![Release](https://github.com/xunzhimeng/one-code-cli/actions/workflows/release.yml/badge.svg)](https://github.com/xunzhimeng/one-code-cli/actions/workflows/release.yml)
[![GitHub Release](https://img.shields.io/github/v/release/xunzhimeng/one-code-cli?include_prereleases&sort=semver)](https://github.com/xunzhimeng/one-code-cli/releases)
[![GitHub Stars](https://img.shields.io/github/stars/xunzhimeng/one-code-cli?style=social)](https://github.com/xunzhimeng/one-code-cli/stargazers)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

`occ` 是一个统一的 coding-agent CLI 调度器。它让一个 AI agent 可以通过统一协议调用另一个 CLI，例如 Claude Code、Codex CLI、opencode、Gemini CLI，并把每次运行默认记录到用户 `~/.occ/` 下的结构化文件。

## 特性

- **统一入口**：用 `occ run --cli <name>` 调用不同 coding-agent CLI。
- **面向其它 AI**：内置 `using-one-code-cli` skill，方便 Codex/Claude/Gemini/其它 agent 快速接入。
- **自动化友好**：提供 prompt 时走非交互运行，输出 JSON，并写入 `result.md`、`command.json`、`run.toml`。
- **默认前台交互**：未提供 prompt 时默认将子 CLI 接到当前终端；也可显式使用 `--interactive`。
- **默认高权限自动模式**：内置 agent 会传递各 CLI 的免确认/高权限参数。
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
occ skills install
occ skills doctor
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
occ run --cli codex --prompt "Reply with exactly OK" --model gpt-5.4 --non-interactive --output json
occ run --cli codex --prompt "Reply with exactly OK" --model gpt-5.4 --effort xhigh --non-interactive --output json
```

按多个 agent 并行调用同一个任务：

```powershell
occ run --agents claude-cc,deepseek-cc --prompt "Review this repository" --stream --output json
occ run --agents claude-cc,deepseek-cc --prompt "Review this repository" --dry-run --output json
```

多 agent 输出会返回 `batch_id` 和 `runs[]`；每个 run 仍有独立 `run_id`、`session_id`、`result_path`、`metadata_path`。加 `--stream` 时，实时输出会带 `[agent]` 前缀，并过滤空行与纯控制序列；完整原始 stdout/stderr 仍写入各自 run 目录。`--agents` 是精确 agent 选择，不会再经过 `--cli` 的默认映射。

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
- **`model` / `model_source`**：实际生效模型，以及模型来自 CLI 参数、agent 配置还是 session
- **`effort` / `effort_source`**：实际生效 effort，以及 effort 来自 CLI 参数、agent 配置、底层 CLI 配置还是 session
- **`exit_code`**：子进程退出码
- 多 agent 时额外包含 **`batch_id` / `runs[]`**：并行批次 ID 与每个 agent 的独立运行结果

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

这适合其它 AI agent 和自动化脚本。需要实时观察子进程输出时，加 `--stream`，它会把子进程 stdout/stderr 镜像到父进程 stderr，同时保留日志和最终 JSON stdout。多 agent 模式下，实时输出会自动按 agent 加前缀，便于同时观察。

显式前台交互模式：

```powershell
occ run --cli claude --interactive
occ run --cli codex --interactive
```

`--interactive` 会继承当前终端的 stdin/stdout/stderr。

`occ` 当前不会自动打开新的外部终端窗口。如果需要独立可见窗口，可用外部 PowerShell/Windows Terminal wrapper 启动 `occ`。

## 默认高权限自动模式

默认 agent 面向本地自动化委派，权限较高。只在你信任的目录中使用。

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
gemini.cmd --yolo --skip-trust --prompt <prompt>
```

非 Windows 平台使用 `claude`、`codex`、`opencode`、`gemini`，不带 `.cmd`。

## dry-run 验证

执行前检查最终命令：

```powershell
occ run --cli claude --prompt "test" --non-interactive --dry-run --output json
occ run --cli codex --prompt "test" --non-interactive --dry-run --output json
occ run --agents claude-cc,deepseek-cc --prompt "test" --dry-run --output json
```

重点看：

- `command.executable`
- `command.args`
- `context.model`
- `model_source`
- `context.effort`
- `effort_source`
- `command.prompt_via_stdin`
- 多 agent dry-run 的 `runs[].agent`、`runs[].context.cli`、`runs[].command.env_keys`

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

快速新增一个 agent：

```powershell
occ agents add deepseek-cc --cli claude --model deepseek-chat `
  --env ANTHROPIC_BASE_URL=https://api.deepseek.com/anthropic `
  --env-allow HTTPS_PROXY `
  --set-cli-default
occ agents show deepseek-cc
occ agents test deepseek-cc
```

`--cli` 会写入配置里的 `cli_type`；`--env` 可以重复传入多个 `KEY=VALUE`，不要用逗号分隔。新建 agent 默认使用 strict 隔离环境：启动子 CLI 前会清空继承环境，再用一小组启动必需的内置白名单、已配置的代理变量、`--env-allow KEY`、自动隔离变量和 agent `env` 重建环境。只有明确想让子 CLI 使用机器默认继承环境时，才传 `--inherit-env`；该模式下 occ 不会创建或设置 `config_dir`，除非你显式传 `--config-dir`。`--set-cli-default` 会更新 `[cli_type_defaults]`，让 `occ run --cli claude ...` 默认选中这个 agent；`--set-default` 则更新全局 `default_agent`。未指定 `--config` 或 `--project` 时，`agents add` 默认写入 `~/.occ/config.toml`。

默认情况下，`agents add` 还会为该 agent 写入并创建独立的 `config_dir`。如果要复用已有 CLI 系统目录，可以传 `--config-dir <dir>` 覆盖。

同一个 CLI 可以配置多个 agent，互不共享 CLI 系统配置、model、base URL、env 或参数。例如 Claude Code 同时走官方 Claude 和 DeepSeek 兼容后端：

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

`config_dir` 会作为子 CLI 自己的系统根目录传入：

- Claude Code：`CLAUDE_CONFIG_DIR=<config_dir>`
- Codex CLI：`CODEX_HOME=<config_dir>`
- opencode：`OPENCODE_CONFIG_DIR=<config_dir>`
- Gemini CLI：`HOME=<config_dir>`；Windows 下还会隔离 `USERPROFILE`、`APPDATA`、`LOCALAPPDATA`、`HOMEDRIVE`、`HOMEPATH`。

如果 agent 的 `env = { ... }` 显式设置了同名变量，会覆盖这些自动隔离变量。相对 `config_dir` 按运行时工作目录解析；settings UI 推荐的隔离目录会写成稳定路径，放在所选配置文件旁边。

内置/默认 CLI agent 的环境模式默认是 `inherit`，但 `occ agents add` 和 settings UI 新建的隔离 agent 默认写入 `env_mode = "strict"`。如果不同 agent 不能看到父进程里的 `ANTHROPIC_*`、`OPENAI_*` 或其它供应商变量，请使用 strict 模式。strict 模式下，`env_allowlist = ["KEY"]` 只额外复制这些指定父环境变量；当 `[proxy].enabled = true` 时，`proxy.env_keys` 即使在 strict 模式下也会转发，禁用 proxy 时这些代理变量会被移除；`env = { ... }` 始终优先于继承值和代理转发值。在 settings UI 里，选择“使用默认 CLI 系统目录”会清空 `config_dir` 并继承普通 CLI 环境；选择“使用隔离 config_dir”会同时设置每个 agent 的 CLI 系统根目录和 strict env。

选择关系：

- `occ run --agent deepseek-cc ...`：精确选择一个 agent。
- `occ run --agents claude-cc,deepseek-cc ...`：并行精确选择多个 agent。
- `occ run --cli claude ...`：按 `cli_type_defaults.claude` 选择默认 agent；未设置时使用第一个匹配 `cli_type = "claude"` 的 agent。
- Agent 别名是某个具体 agent 的备用名字，用于 `--agent`、`--agents` 和 `/agent`。
- CLI 类型别名是 CLI 类型的备用名字，用于 `--cli` 和 `/cli`；`codex = "codex"` 这种原名别名是冗余的。

resume 会尽量使用各 CLI 的原生机制。Claude Code 与 Gemini 新会话会收到 occ 生成的原生 UUID，后续 `--resume` 可以回到同一个 CLI 原生 session；Codex 和 opencode 使用当前 CLI 的 resume/continue 命令。`--session <id>` 始终绑定该 session 原始 agent；传入冲突的 `--agent` 或 `--cli` 会返回 `session_agent_mismatch`。

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
occ settings
occ settings --output config-ui.html --no-open
occ config export-html
```

`occ settings` 默认通过本地 server 打开完整的表单式 settings UI，这样保存、重新加载、TOML 双向同步都能像交互编辑器一样工作。只有需要导出简单的独立 TOML HTML 时，再使用 `--output`。

这些容器命令不带子命令时会默认列出内容：

```powershell
occ agents
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
/agent claude
/cli codex
/model gpt-5.4
/model
/session
/clear
/exit
```

`/agent` 和 `/cli` 会清空当前 session 与本地 transcript，避免把不同 CLI 的上下文混在一起。

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
