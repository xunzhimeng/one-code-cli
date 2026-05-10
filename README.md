# One Code CLI (`occ`)

> 中文 | [English](#english)

[![CI](https://github.com/xunzhimeng/one-code-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/xunzhimeng/one-code-cli/actions/workflows/ci.yml)
[![Release](https://github.com/xunzhimeng/one-code-cli/actions/workflows/release.yml/badge.svg)](https://github.com/xunzhimeng/one-code-cli/actions/workflows/release.yml)
[![GitHub Release](https://img.shields.io/github/v/release/xunzhimeng/one-code-cli?include_prereleases&sort=semver)](https://github.com/xunzhimeng/one-code-cli/releases)
[![GitHub Stars](https://img.shields.io/github/stars/xunzhimeng/one-code-cli?style=social)](https://github.com/xunzhimeng/one-code-cli/stargazers)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

`occ` 是一个统一的 coding-agent CLI 调度器。它让一个 AI agent 可以通过统一协议调用另一个 CLI，例如 Claude Code、Codex CLI、opencode、Gemini CLI，并把每次运行记录为 `.occ/` 下的结构化文件。

## 特性

- **统一入口**：用 `occ run --backend <name>` 调用不同 coding-agent CLI。
- **面向其它 AI**：内置 `using-one-code-cli` skill，方便 Codex/Claude/Gemini/其它 agent 快速接入。
- **自动化友好**：默认非交互运行，输出 JSON，并写入 `result.md`、`command.json`、`run.toml`。
- **支持前台交互**：用 `--interactive` 将子 CLI 接到当前终端。
- **默认高权限自动模式**：内置 profile 会传递各 CLI 的免确认/高权限参数。
- **Windows 友好**：Windows 默认使用 npm shim 的 `.cmd` 入口，并对 Claude/Codex 通过 stdin 传 prompt，避免长 Markdown 参数触发 batch 问题。
- **代理转发**：默认转发 `HTTP_PROXY`、`HTTPS_PROXY`、`ALL_PROXY`、`NO_PROXY` 及小写变体。
- **配置 UI**：支持本地可保存配置 UI，也支持静态 HTML 导出。

## 安装

### 方式一：npm 全局安装

发布到 npm 后，用户可以直接安装：

```powershell
npm install -g one-code-cli
occ config init --user
occ config validate
```

npm wrapper 会在安装时从 GitHub Release 下载当前 npm 包版本对应的原生二进制，例如 `v0.1.0` 会下载：

```text
https://github.com/xunzhimeng/one-code-cli/releases/download/v0.1.0/occ-<target>
```

本地测试 npm wrapper：

```powershell
npm install -g .
```

如果还没有发布对应 GitHub Release，本地安装会 fallback 到 `cargo build --release`。

### 方式二：从 GitHub 源码安装

只要仓库已经推到 GitHub，别人有 Rust/Cargo 就能直接安装：

```powershell
cargo install --git https://github.com/xunzhimeng/one-code-cli.git --locked
occ config init --user
occ config validate
```

安装指定 tag：

```powershell
cargo install --git https://github.com/xunzhimeng/one-code-cli.git --tag v0.1.0 --locked
```

这是不依赖 npm registry 的公开安装方案。

### 方式三：下载 GitHub Release 二进制

本仓库包含 `.github/workflows/release.yml`。推送 tag 后会构建这些压缩包和 npm wrapper 使用的裸二进制，并发布到 GitHub Releases：

- `one-code-cli-x86_64-pc-windows-msvc.zip`
- `one-code-cli-x86_64-unknown-linux-gnu.tar.gz`
- `one-code-cli-x86_64-apple-darwin.tar.gz`
- `one-code-cli-aarch64-apple-darwin.tar.gz`
- `occ-x86_64-pc-windows-msvc.exe`
- `occ-x86_64-unknown-linux-gnu`
- `occ-x86_64-apple-darwin`
- `occ-aarch64-apple-darwin`

发布 release：

```powershell
git tag v0.1.0
git push origin v0.1.0
```

Windows 用户下载 zip 后，把 `occ.exe` 所在目录加入 `PATH`。

macOS/Linux 用户下载 tar.gz 后，把 `occ` 放到 `~/.local/bin`、`/usr/local/bin` 或其它 PATH 目录。

### 方式四：本地开发安装

```powershell
cargo install --path . --force
occ config init --user --force
occ config validate
```

### npm 发布状态

npm wrapper 和 `package.json` 已经实现，但还没有发布到 npm registry。

真正发布 npm 需要：

- npm 账号
- 本地执行 `npm login`
- 包名 `one-code-cli` 在 npm 上可用，或改成 scope 包名，例如 `@xunzhimeng/one-code-cli`
- 已经发布同版本 GitHub Release，例如 npm `0.1.0` 对应 GitHub tag `v0.1.0`
- 如果用 GitHub Actions 发布，需要在仓库 Secrets 配置 `NPM_TOKEN`

发布命令：

```powershell
npm publish
```

如果使用 scope 包名：

```powershell
npm publish --access public
```

Tag release 会通过 `.github/workflows/release.yml` 自动发布 npm。也可以通过 `.github/workflows/npm-publish.yml` 手动补发；这种方式不需要本机 `npm login`，但仍然需要 npm 账号和 `NPM_TOKEN`。

## GitHub Actions

已包含两个 workflow：

- **CI**：`.github/workflows/ci.yml`
  - `cargo fmt --all -- --check`
  - `cargo check --all-targets`
  - `cargo test --all-targets`
  - 覆盖 Ubuntu、macOS、Windows

- **Release**：`.github/workflows/release.yml`
  - 在 `v*` tag 上构建 release binary
  - 上传 Windows/Linux/macOS 压缩包
  - 上传 npm wrapper 下载用的裸二进制
  - 自动创建 GitHub Release
  - GitHub Release 完成后发布 npm package

- **Publish npm**：`.github/workflows/npm-publish.yml`
  - 手动 `workflow_dispatch` 触发
  - 作为 npm 发布备用入口
  - 需要在 GitHub 仓库 Secrets 中配置 `NPM_TOKEN`

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

按 backend 调用：

```powershell
occ run --backend claude --prompt "Reply with exactly OK" --output json
occ run --backend codex --prompt "Reply with exactly OK" --output json
```

长任务推荐 prompt file：

```powershell
occ run --backend claude --cwd "E:\project\repo" --prompt-file task.md --output json
```

JSON 输出包含：

- **`result_path`**：最终 Markdown 结果，优先读取
- **`metadata_path`**：运行元数据
- **`run_id` / `session_id`**：运行与会话 ID
- **`exit_code`**：子进程退出码

## 后台与前台

默认 `occ run` 是非交互自动执行模式：

- 捕获 stdout/stderr
- 写入 `.occ/runs/<run_id>/result.md`
- 写入 `.occ/runs/<run_id>/command.json`
- 不显示子 CLI TUI

这适合其它 AI agent 和自动化脚本。

前台交互模式：

```powershell
occ run --backend claude --interactive
occ run --backend codex --interactive
```

`--interactive` 会继承当前终端的 stdin/stdout/stderr。

`occ` 当前不会自动打开新的外部终端窗口。如果需要独立可见窗口，可用外部 PowerShell/Windows Terminal wrapper 启动 `occ`。

## 默认高权限自动模式

默认 profile 面向本地自动化委派，权限较高。只在你信任的目录中使用。

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
occ run --backend claude --prompt "test" --dry-run --output json
occ run --backend codex --prompt "test" --dry-run --output json
```

重点看：

- `command.executable`
- `command.args`
- `command.prompt_via_stdin`

## 本地 smoke test

Claude Code：

```powershell
occ run --backend claude --prompt "Reply with exactly OCC_CLAUDE_FINAL_OK" --output json --timeout 90s
```

Codex CLI：

```powershell
occ run --backend codex --prompt "Reply with exactly OCC_CODEX_FINAL_OK" --output json --timeout 180s
```

两者都应返回 `success = true`，然后读取 `result_path`。

## 配置

配置搜索顺序：

1. `<cwd>/.occ.toml`
2. `<cwd>/.occ/config.toml`
3. `~/.occ/config.toml`
4. 内置默认配置

常用命令：

```powershell
occ config show
occ config validate
occ config ui
occ config export-html
```

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

`occ` is a unified dispatcher for coding-agent CLIs. It lets one AI agent delegate work to another CLI such as Claude Code, Codex CLI, opencode, or Gemini CLI, then records each run as structured files under `.occ/`.

## Features

- **One command for multiple agents**: dispatch with `occ run --backend <name>`.
- **Agent-friendly**: ships with the `using-one-code-cli` skill for other AI agents.
- **Automation-first**: non-interactive by default, JSON output, structured run artifacts.
- **Foreground mode**: use `--interactive` to attach the child CLI to the current terminal.
- **High-permission defaults**: default profiles pass each CLI's auto-approval / high-permission flags.
- **Windows-friendly**: defaults to `.cmd` npm shims and sends Claude/Codex prompts through stdin.
- **Proxy forwarding**: forwards common proxy environment variables by default.
- **Config UI**: local save-capable UI and static HTML export are supported.

## Installation

### Option 1: Install globally with npm

After the npm package is published:

```powershell
npm install -g one-code-cli
occ config init --user
occ config validate
```

The npm wrapper downloads the native binary for the current package version from GitHub Releases. For example, npm package `0.1.0` downloads from tag `v0.1.0`:

```text
https://github.com/xunzhimeng/one-code-cli/releases/download/v0.1.0/occ-<target>
```

Test the npm wrapper locally:

```powershell
npm install -g .
```

If the matching GitHub Release does not exist yet, local installation falls back to `cargo build --release`.

### Option 2: Install from GitHub source

Once this repository is public on GitHub:

```powershell
cargo install --git https://github.com/xunzhimeng/one-code-cli.git --locked
occ config init --user
occ config validate
```

Install a specific tag:

```powershell
cargo install --git https://github.com/xunzhimeng/one-code-cli.git --tag v0.1.0 --locked
```

This public installation path does not depend on npm registry.

### Option 3: Download GitHub Release binaries

The release workflow builds archives and raw binaries used by the npm wrapper:

- `one-code-cli-x86_64-pc-windows-msvc.zip`
- `one-code-cli-x86_64-unknown-linux-gnu.tar.gz`
- `one-code-cli-x86_64-apple-darwin.tar.gz`
- `one-code-cli-aarch64-apple-darwin.tar.gz`
- `occ-x86_64-pc-windows-msvc.exe`
- `occ-x86_64-unknown-linux-gnu`
- `occ-x86_64-apple-darwin`
- `occ-aarch64-apple-darwin`

Create a release:

```powershell
git tag v0.1.0
git push origin v0.1.0
```

Download the matching archive from GitHub Releases and put `occ` / `occ.exe` on your `PATH`.

### Option 4: Local development install

```powershell
cargo install --path . --force
occ config init --user --force
occ config validate
```

### npm publish status

The npm wrapper and `package.json` are implemented, but the package has not been published to npm registry yet.

Publishing to npm requires:

- An npm account
- `npm login`
- The package name `one-code-cli` to be available, or a scoped name such as `@xunzhimeng/one-code-cli`
- A matching GitHub Release for the same version, for example npm `0.1.0` and GitHub tag `v0.1.0`
- `NPM_TOKEN` in repository Secrets if publishing through GitHub Actions

Publish:

```powershell
npm publish
```

For a scoped public package:

```powershell
npm publish --access public
```

Tag releases publish npm through `.github/workflows/release.yml`. You can also publish manually through `.github/workflows/npm-publish.yml`; that path does not require local `npm login`, but still requires an npm account and `NPM_TOKEN`.

## GitHub Actions

This repository includes:

- **CI**: `.github/workflows/ci.yml`
- **Release**: `.github/workflows/release.yml`
- **Publish npm**: `.github/workflows/npm-publish.yml`

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
occ run --backend claude --prompt "Reply with exactly OK" --output json
occ run --backend codex --prompt "Reply with exactly OK" --output json
```

For long tasks:

```powershell
occ run --backend claude --cwd "E:\project\repo" --prompt-file task.md --output json
```

Read `result_path` first from the JSON response.

## Foreground vs background

Default mode is non-interactive:

- Captures stdout/stderr
- Writes `.occ/runs/<run_id>/result.md`
- Writes `.occ/runs/<run_id>/command.json`
- Does not show the child CLI TUI

Foreground mode:

```powershell
occ run --backend claude --interactive
occ run --backend codex --interactive
```

## High-permission defaults

Default profiles are designed for trusted local automation.

Windows command shapes:

- **Claude Code**: `claude.cmd --print --dangerously-skip-permissions`
- **Codex CLI**: `codex.cmd exec --dangerously-bypass-approvals-and-sandbox --skip-git-repo-check`
- **opencode**: `opencode.cmd run --dangerously-skip-permissions <prompt>`
- **Gemini CLI**: `gemini.cmd --yolo --skip-trust -p <prompt>`

On non-Windows platforms, executable names do not use `.cmd`.

## Configuration

```powershell
occ config show
occ config validate
occ config ui
occ config export-html
```

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=xunzhimeng/one-code-cli&type=Date)](https://www.star-history.com/#xunzhimeng/one-code-cli&Date)

