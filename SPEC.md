# One Code CLI 规格说明

- **名称**：One Code CLI
- **命令**：`occ`
- **包名 / 项目标识**：`one-code-cli`
- **状态**：草案
- **日期**：2026-05-10
- **技术栈**：Rust

本文档定义一个基于 Rust 的统一 Coding Agent CLI 调度工具。它用于封装 Claude Code、Codex CLI、OpenCode 等子 CLI，为其它 agents 或自动化脚本提供统一入口、统一配置、统一文档输出和会话恢复能力。

## 1. 命名结论

### 1.1 候选名称搜索结论

已初步搜索以下名称：

- **`onecli`**：不推荐。已有 `onecli.sh` 和 GitHub 组织/项目，且定位也与 AI agent、CLI gateway 相关，冲突较强。
- **`acode`**：不推荐。已有 Acode/Autocode 相关 CLI、GitHub 项目和 crates.io 包。
- **`anycode`**：不推荐。已有 Microsoft `vscode-anycode` 和其它 AnyCode/anycode 项目。
- **`codemux`**：不推荐。已有 CodeMux 项目，且同样面向多 coding agent / 多引擎场景。
- **`agentmux`**：不推荐。已有 AgentMux 项目，且与 Claude Code、Codex CLI、多 agent terminal workflows 重叠。
- **`codewrap`**：可用性较好，但语义更像 wrapper。
- **`codeany`**：可接受。搜索结果中存在 `codeany-ai`、`CodeAny-inc` 等组织/项目，但没有看到强占“统一 coding-agent CLI 调度器”定位的项目。
- **`one-code-cli`**：采用。相比 `onecli`，`one-code-cli` 更明确指向 code agent CLI 聚合器；简称 `occ`，便于调用。

### 1.2 推荐名称

推荐采用：

```text
One Code CLI
```

推荐主命令：

```bash
occ
```

项目包名：

```bash
one-code-cli
```

说明：

- `one-code-cli` 比 `anycodecli` 语义更清楚。
- `occ` 足够短，适合作为高频调用命令。
- `One Code CLI` 表示“一个入口调用多种 coding agent CLI”。
- 如果发布 crates.io 时 `one-code-cli` 包名不可用，可以使用 `one-code-cli-rs` 作为 crate 名，但安装后的 binary 仍建议叫 `occ`。

## 2. 产品定位

One Code CLI 是一个统一的 Coding Agent CLI 调度层。

它负责：

- 统一调用 Claude Code、Codex CLI、OpenCode 等子 CLI。
- 通过 backend 和 profile 管理多个 CLI 类型与多个实例。
- 支持每个 backend 的默认配置。
- 支持工作目录 `cwd`。
- 支持 prompt 输入归一化。
- 支持默认非交互模式和可选交互模式。
- 支持 Markdown 文档输出。
- 支持运行记录。
- 支持会话管理与恢复。
- 支持通过 HTML 配置页面管理多个 CLI 实例。

它不负责：

- 多 agent 编排。
- 沙箱隔离。
- 权限控制。
- 安全策略。
- 模型供应商管理。
- 替代 Claude Code、Codex CLI、OpenCode 自身能力。

权限、安全、工具调用策略由子 CLI 自身或子 CLI 配置控制。

## 3. 核心概念

### 3.1 Backend

Backend 表示子 CLI 类型。

首版必须支持：

- `claude`：Claude Code CLI。
- `codex`：Codex CLI。
- `opencode`：OpenCode。

每个 backend 都应有内置默认配置。即使用户没有写 profile，也可以通过 backend 类型进行基础调用。

示例：

```bash
occ run --backend claude --cwd ./repo --prompt "修复测试"
```

内置 backend 默认配置包括：

- 默认可执行文件名。
- 默认参数映射。
- 默认 prompt 传递方式。
- 默认交互/非交互处理方式。
- 是否支持原生 session resume 的能力声明。

### 3.2 Profile

Profile 表示某个 backend 的一个具体实例。

示例：

- `claude-sonnet`
- `claude-ds`
- `claude-opus`
- `codex-gpt5`
- `opencode-default`

Profile 是推荐的主调用入口：

```bash
occ run --profile claude-ds --cwd ./repo --prompt "审查这个项目"
```

同一个 backend 可以有多个 profile，用于表达：

- 不同模型。
- 不同配置目录。
- 不同环境变量。
- 不同 wrapper 命令。
- 不同子 CLI 参数。

### 3.3 Backend 默认 Profile

需要支持 backend 级默认 profile。

配置示例：

```toml
[backend_defaults]
claude = "claude-sonnet"
codex = "codex-gpt5"
opencode = "opencode-default"
```

选择规则：

1. 如果指定 `--profile`，使用该 profile。
2. 如果指定 `--backend`，且 `[backend_defaults]` 中配置了该 backend 的默认 profile，使用该默认 profile。
3. 如果指定 `--backend`，但没有配置 backend 默认 profile，使用该 backend 在配置文件中出现的第一个 profile。
4. 如果指定 `--backend`，配置文件中没有对应 profile，则使用该 backend 的内置默认配置。
5. 如果没有指定 `--profile` 或 `--backend`，使用全局 `default_profile`。
6. 如果全局 `default_profile` 未设置，则报配置错误，不自动猜测。

### 3.4 Run

Run 表示 One Code CLI 的一次执行。

每个 run 应记录：

- run id。
- profile。
- backend。
- model。
- cwd。
- prompt 来源。
- 是否交互模式。
- session id。
- 子进程退出码。
- 开始/结束时间。
- 输出文档路径。

### 3.5 Session

Session 表示 One Code CLI 维护的一段连续子 CLI 会话。

每个 session 应记录：

- One Code CLI 自己的 `session_id`。
- 子 CLI 原生会话 id，即 `backend_session_id`。
- profile。
- backend。
- cwd。
- model。
- 最近一次 run id。
- 创建/更新时间。

Run 和 Session 是不同概念：

- 一个 run 是一次执行。
- 一个 session 可以包含多个 run。

## 4. 命令设计

### 4.1 `run`

执行一次 agent 任务。

示例：

```bash
occ run --profile claude-sonnet --cwd ./repo --prompt "修复失败的测试"
```

```bash
occ run --backend claude --cwd ./repo --prompt-file task.md
```

```bash
Get-Content task.md | occ run --profile codex-gpt5 --cwd ./repo --stdin
```

参数：

- `--profile <name>`：选择具体 profile。
- `--backend <name>`：选择 backend 类型，并按 backend 默认 profile / 第一个 profile / 内置默认配置解析。
- `--model <name>`：覆盖 profile 中的默认模型。
- `--cwd <path>`：设置子 CLI 工作目录，默认当前目录。
- `--prompt <text>`：直接传入任务文本。
- `--prompt-file <path>`：从文件读取任务文本。
- `--stdin`：从标准输入读取任务文本。
- `--interactive`：交互模式。
- `--non-interactive`：非交互模式，默认值。
- `--session <session-id>`：指定 One Code CLI session。
- `--resume`：恢复会话。
- `--doc-root <path>`：覆盖文档和状态目录。
- `--output <text|json|path>`：控制 One Code CLI 自身最终输出。
- `--timeout <duration>`：可选超时。
- `--dry-run`：只打印解析后的配置和命令，不执行子 CLI。
- `--`：后续参数原样透传给子 CLI。

Prompt 规则：

1. 非交互任务中，`--prompt`、`--prompt-file`、`--stdin` 只能三选一。
2. 交互模式可以不提供 prompt。
3. 如果同时提供多个 prompt 来源，应报参数错误。

### 4.2 `doctor`

检查环境和配置。

```bash
occ doctor
```

检查内容：

- 配置文件是否可发现。
- 配置文件是否可解析。
- backend 默认配置是否有效。
- profile 是否有效。
- 子 CLI 可执行文件是否存在。
- 文档目录是否可写。
- session 索引是否可写。
- resume 能力是否与配置一致。

### 4.3 `profiles`

管理 profile。

```bash
occ profiles list
occ profiles show claude-sonnet
occ profiles test claude-sonnet
```

行为：

- `list`：列出所有 profile，包括内置默认 profile 和用户配置 profile。
- `show`：显示 profile 的最终解析结果。
- `test`：解析命令、环境变量和参数模板，但不执行任务。

### 4.4 `backends`

查看 backend。

```bash
occ backends list
occ backends show claude
```

显示内容：

- backend 名称。
- 默认可执行文件名。
- 内置默认 profile。
- backend 默认 profile。
- 是否支持模型参数。
- 是否支持交互模式。
- 是否支持非交互模式。
- 是否支持原生 resume。

### 4.5 `config`

管理配置。

```bash
occ config init
occ config path
occ config show
occ config validate
occ config ui
occ config export-html
```

行为：

- `init`：创建示例配置。
- `path`：显示配置查找路径。
- `show`：显示合并后的有效配置。
- `validate`：校验配置。
- `ui`：启动本地 HTML 配置页面。
- `export-html`：导出一个静态 HTML 配置页面。

### 4.6 `sessions`

管理会话。

```bash
occ sessions list
occ sessions show <session-id>
occ sessions resume <session-id> --prompt "继续任务"
occ sessions latest --profile claude-sonnet --cwd ./repo
```

行为：

- `list`：列出 session。
- `show`：按 session id 查看会话。
- `resume`：按 session id 恢复会话。
- `latest`：按 profile/backend + cwd 查找最近 session。

必须支持直接通过 session id 查找，不强制要求提供 cwd。

### 4.7 `runs`

管理运行记录。

```bash
occ runs list
occ runs show <run-id>
occ runs open <run-id>
```

行为：

- `list`：列出最近 run。
- `show`：查看 run 元数据。
- `open`：打开或打印 `result.md` 路径。

### 4.8 `skills`

管理给其它 agents 使用的 skill 包。

```bash
occ skills list
occ skills show using-one-code-cli
occ skills export using-one-code-cli --target ~/.agents/skills
occ skills install --target ~/.agents/skills
occ skills doctor
```

行为：

- `list`：列出内置 skills、用户级 skills 和项目级 skills。
- `show`：显示 skill 的说明、入口文件和调用示例。
- `export`：将指定 skill 导出到目标 agents skills 目录。
- `install`：安装首版推荐的内置 skills。
- `doctor`：检查目标 skills 目录、`occ` 可执行文件、配置和示例命令是否可用。

## 5. 配置文件

### 5.1 配置格式

使用 TOML。

原因：

- Rust 生态支持好。
- 比 YAML 更少歧义。
- 适合 CLI 配置。
- 便于用 `serde` 解析。

### 5.2 配置查找顺序

按以下顺序查找：

1. 命令行 `--config <path>`。
2. `<cwd>/.occ.toml`。
3. `<cwd>/.occ/config.toml`。
4. 用户级 `~/.occ/config.toml`。
5. 内置默认配置。

项目级配置覆盖用户级配置。

### 5.3 默认配置

即使用户没有创建配置文件，One Code CLI 也应内置三个 backend 的默认配置。

内置默认 profile：

```text
claude
codex
opencode
```

默认含义：

- profile `claude` 使用 backend `claude` 和命令 `claude`。
- profile `codex` 使用 backend `codex` 和命令 `codex`。
- profile `opencode` 使用 backend `opencode` 和命令 `opencode`。

这些默认配置允许：

```bash
occ run --backend claude --prompt "说明这个仓库"
```

但如果用户不传 `--backend` 或 `--profile`，仍要求配置 `default_profile`，否则报错。

### 5.4 配置示例

使用 TOML array-of-tables 表达有序 profiles，便于“按 backend 选第一个 profile”。

```toml
version = 1
default_profile = "claude-sonnet"
doc_root = ".occ"

[backend_defaults]
claude = "claude-sonnet"
codex = "codex-gpt5"
opencode = "opencode-default"

[[profiles]]
name = "claude-sonnet"
backend = "claude"
command = "claude"
model = "claude-3-5-sonnet"
config_dir = "C:/Users/xiaoy/.claude"
args_strategy = "builtin"

[[profiles]]
name = "claude-ds"
backend = "claude"
command = "claude"
model = "deepseek-chat"
config_dir = "C:/Users/xiaoy/.claude-ds"
args_strategy = "append"
extra_args = ["--some-child-cli-flag"]
env = { EXAMPLE_CONFIG_HOME = "{config_dir}" }

[[profiles]]
name = "codex-gpt5"
backend = "codex"
command = "codex"
model = "gpt-5-codex"
args_strategy = "builtin"

[[profiles]]
name = "opencode-default"
backend = "opencode"
command = "opencode"
args_strategy = "builtin"
```

注意：不同子 CLI 切换配置目录的方式可能不同。One Code CLI 不应盲目硬编码不稳定机制，而应允许用户通过 `env`、`args`、`extra_args`、`path` 或 wrapper 命令表达。

### 5.5 Profile 字段

必填字段：

- `name`
- `backend`

可选字段：

- `command`：在 `PATH` 中查找的命令名。
- `path`：精确可执行文件路径，优先级高于 `command`。
- `model`：默认模型。
- `config_dir`：逻辑配置目录，可用于模板变量。
- `env`：子进程环境变量。
- `args_strategy`：`builtin`、`append`、`override`。
- `args`：`override` 策略下的完整参数模板。
- `extra_args`：`append` 策略下追加到内置参数后的参数。
- `prompt_via`：`stdin`、`arg` 或 `file`。
- `resume_args`：原生 resume 参数模板。
- `interactive_args`：交互模式参数。
- `non_interactive_args`：非交互模式参数。

### 5.6 模板变量

支持：

- `{profile}`
- `{backend}`
- `{model}`
- `{cwd}`
- `{prompt}`
- `{prompt_file}`
- `{config_dir}`
- `{session_id}`
- `{backend_session_id}`
- `{run_id}`
- `{doc_root}`

One Code CLI 必须用参数数组启动子进程，不应拼接 shell 字符串。

## 6. HTML 配置页面

One Code CLI 需要支持通过 HTML 页面配置多个 CLI 类型和多个实例。

### 6.1 目标

HTML 配置页面用于降低多 profile 配置成本。

它应支持：

- 添加/删除 backend 实例。
- 配置 Claude Code 多实例。
- 配置 Codex CLI 多实例。
- 配置 OpenCode 多实例。
- 设置每个实例的名称、命令、路径、模型、配置目录、环境变量和额外参数。
- 设置全局 `default_profile`。
- 设置 backend 级默认 profile。
- 导入现有 TOML。
- 导出 TOML。
- 校验必填项。

### 6.2 推荐实现方式

首版推荐两种模式。

#### 本地 UI 模式

```bash
occ config ui
```

行为：

- 启动仅监听 `127.0.0.1` 的本地服务。
- 在浏览器打开配置页面。
- 页面读取当前有效配置。
- 用户修改后由本地服务写回 TOML 配置文件。
- 服务关闭后页面失效。

#### 静态 HTML 导出模式

```bash
occ config export-html
```

行为：

- 导出一个自包含 HTML 文件。
- 用户可以在浏览器中编辑配置。
- 页面提供“复制 TOML”或“下载 TOML”功能。
- 如果浏览器支持 File System Access API，可以选择直接保存到本地文件。

### 6.3 配置数据源

TOML 仍是权威配置格式。

HTML 页面只是配置编辑器，不是唯一配置源。

## 7. Skills 系统

One Code CLI 需要提供 skills 系统，方便 Claude Code、Codex CLI、OpenCode 之外的其它 agents 以稳定方式调用 `occ`。

这里的 skill 不是新的 agent backend，也不是多 agent 编排系统，而是一组面向其它 agents 的“可安装调用说明 + 示例 + 输入输出约定”。

### 7.1 设计目标

Skills 系统目标：

- 让其它 agents 不需要理解 One Code CLI 的全部实现细节。
- 为其它 agents 提供稳定、简短、可复制的调用协议。
- 明确如何传入 prompt、cwd、profile/backend、session、resume 等参数。
- 明确如何读取 `result.md`、`run.toml`、`events.jsonl` 等输出文件。
- 降低其它 agents 调用子 agent CLI 的 prompt 成本。
- 支持把 skills 导出到常见 agents skills 目录，例如 `~/.agents/skills`。

### 7.2 Skill 类型

首版内置以下 skills：

- `using-one-code-cli`：通用调用说明，适合任意 agent 调用 `occ run`。
- `delegate-to-claude-code`：通过 `occ` 委托 Claude Code 执行任务。
- `delegate-to-codex-cli`：通过 `occ` 委托 Codex CLI 执行任务。
- `delegate-to-opencode`：通过 `occ` 委托 OpenCode 执行任务。
- `resume-one-code-session`：说明如何按 session id 或 profile + cwd 恢复会话。
- `read-one-code-result`：说明如何读取和总结 run 输出文档。

后续可以增加项目级自定义 skill，但 MVP 先提供内置 skills 的导出和安装。

### 7.3 Skill 安装位置

One Code CLI 自身内置 skills 模板。

导出目标由用户指定：

```bash
occ skills export using-one-code-cli --target ~/.agents/skills
```

推荐默认目标：

```text
~/.agents/skills/
```

项目级导出目标：

```text
<cwd>/.occ/skills/
```

用户级缓存位置：

```text
~/.occ/skills/
```

### 7.4 Skill 目录结构

每个 skill 是一个目录。

示例：

```text
using-one-code-cli/
  SKILL.md
  skill.toml
  examples/
    run-with-profile.md
    run-with-backend.md
    resume-session.md
    read-result.md
```

字段说明：

- `SKILL.md`：给其它 agents 阅读的主说明文件。
- `skill.toml`：机器可读元数据。
- `examples/`：可复制的调用示例和提示词模板。

### 7.5 `skill.toml` 格式

示例：

```toml
version = 1
name = "using-one-code-cli"
title = "Use One Code CLI"
description = "Delegate coding tasks to Claude Code, Codex CLI, or OpenCode through occ."
entry = "SKILL.md"
requires_command = "occ"

[inputs]
cwd = "Required working directory for the delegated task."
prompt = "Required task prompt or prompt file."
profile = "Optional exact One Code CLI profile."
backend = "Optional backend type: claude, codex, opencode."
session = "Optional One Code CLI session id."

[outputs]
result_path = "Path to result.md."
metadata_path = "Path to run.toml."
session_id = "One Code CLI session id."
run_id = "One Code CLI run id."
```

### 7.6 `SKILL.md` 内容规范

`SKILL.md` 必须面向调用方 agent，内容应短、明确、操作性强。

推荐结构：

````markdown
# Use One Code CLI

## When to use

Use this skill when you need to delegate a coding task to another coding-agent CLI through `occ`.

## Required inputs

- Working directory
- Task prompt
- Profile or backend

## Basic command

```bash
occ run --profile claude-sonnet --cwd <path> --prompt-file <task.md> --output json
```

## Read the result

Read `result_path` from the JSON output, then inspect `result.md`.

## Resume

Use `session_id` from the previous JSON output:

```bash
occ run --session <session-id> --resume --prompt-file <task.md> --output json
```
````

### 7.7 Agent 调用协议

其它 agents 调用 `occ` 时，推荐遵守以下协议。

#### 任务输入

优先使用 prompt 文件：

```bash
occ run --profile <profile> --cwd <cwd> --prompt-file <task.md> --output json
```

原因：

- 避免命令行转义问题。
- 方便保存任务上下文。
- 方便审计。

#### 输出读取

调用方 agent 不应只依赖 stdout 文本，而应读取 JSON 输出中的路径：

```json
{
  "success": true,
  "run_id": "run_...",
  "session_id": "sess_...",
  "result_path": ".../result.md",
  "metadata_path": ".../run.toml"
}
```

调用方 agent 应优先读取：

1. `result_path`
2. `metadata_path`
3. `events.jsonl`
4. `stdout.log`
5. `stderr.log`

#### 错误处理

调用方 agent 应检查：

- `success`
- `error.code`
- `error.message`
- 子进程 `exit_code`

如果错误码是 `resume_unsupported`，调用方应停止 resume，而不是重试 metadata-only resume。

### 7.8 Skill 与 Profile 的关系

Skill 不直接绑定某个 profile，除非是专用 delegate skill。

通用 skill 应让调用方显式传：

- `--profile`
- 或 `--backend`

专用 delegate skill 可以推荐固定 backend：

```bash
occ run --backend claude --cwd <cwd> --prompt-file <task.md> --output json
```

如果用户配置了 backend 默认 profile，则 `--backend claude` 会自动解析到对应 profile。

### 7.9 Skill 与 Session 的关系

Skill 应明确告诉调用方：

- 首次任务不传 `--resume`。
- 后续任务优先使用 `--session <session-id> --resume`。
- 如果没有 session id，可以使用 `--resume --profile <profile> --cwd <cwd>` 查找最近会话。
- 如果 backend/profile 不支持原生 resume，`occ` 会直接失败。

### 7.10 Skill 与文档输出的关系

Skill 的核心输出不是自然语言 stdout，而是 One Code CLI 生成的文档。

调用方 agent 的推荐流程：

1. 写入任务文件 `task.md`。
2. 调用 `occ run --prompt-file task.md --output json`。
3. 解析 JSON。
4. 读取 `result.md`。
5. 如需继续，使用 `session_id` 调用 resume。

### 7.11 内置 skill 模板维护

内置 skill 模板应作为源码的一部分维护。

推荐目录：

```text
assets/
  skills/
    using-one-code-cli/
      SKILL.md
      skill.toml
      examples/
    delegate-to-claude-code/
      SKILL.md
      skill.toml
    delegate-to-codex-cli/
      SKILL.md
      skill.toml
    delegate-to-opencode/
      SKILL.md
      skill.toml
    resume-one-code-session/
      SKILL.md
      skill.toml
    read-one-code-result/
      SKILL.md
      skill.toml
```

Rust 实现可以通过 `include_str!` 或构建时嵌入方式打包这些模板。

### 7.12 Skills 命令

首版命令：

```bash
occ skills list
occ skills show <skill-name>
occ skills export <skill-name> --target <dir>
occ skills install --target <dir>
occ skills doctor --target <dir>
```

命令行为：

- `list`：列出内置 skills 和本地已安装 skills。
- `show`：打印 `SKILL.md` 内容或摘要。
- `export`：导出单个 skill 到目标目录。
- `install`：导出所有推荐内置 skills。
- `doctor`：检查目标目录是否存在、是否可写、skill 文件是否完整、`occ` 是否可执行。

### 7.13 非目标

Skills 系统首版不做：

- 在线 skill 市场。
- 远程下载 skill。
- 自动执行未信任 skill。
- skill 插件运行时。
- 多 agent 编排。
- 权限策略。

## 8. 参数映射策略

“内置参数映射 + profile 覆盖”表示：

- One Code CLI 为每个 backend 内置默认调用规则。
- 用户普通使用时无需手写完整子 CLI 参数。
- 特殊实例可以追加参数。
- 特殊实例也可以完全覆盖参数模板。

### 8.1 `builtin`

使用 backend 内置参数映射。

```toml
args_strategy = "builtin"
```

### 8.2 `append`

使用内置参数映射，并追加 `extra_args`。

```toml
args_strategy = "append"
extra_args = ["--verbose"]
```

### 8.3 `override`

完全使用 profile 自定义参数模板。

```toml
args_strategy = "override"
args = ["--model", "{model}", "--print", "{prompt}"]
```

## 9. 工作目录

`--cwd` 表示子 CLI 的启动工作目录。

规则：

- 默认当前 shell 目录。
- 运行前必须存在。
- 写入 run 元数据。
- 写入 session 元数据。
- 用于查找最近 session。
- 不代表隔离边界。

One Code CLI 不限制子 CLI 的文件访问能力。

## 10. 文档和状态目录

### 10.1 默认目录

默认项目级目录：

```text
<cwd>/.occ/
```

可通过配置项 `doc_root` 或命令行 `--doc-root` 覆盖。

### 10.2 目录结构

```text
.occ/
  config.toml
  index.jsonl
  session-index.jsonl
  skills/
  docs/
    index.md
    runs/
      <run-id>.md
    sessions/
      <session-id>.md
  runs/
    <run-id>/
      run.toml
      prompt.md
      result.md
      stdout.log
      stderr.log
      events.jsonl
      command.json
      artifacts/
  sessions/
    <session-id>/
      session.toml
      runs.jsonl
```

### 10.3 Run 文件

每次 run 生成：

- `run.toml`：结构化元数据。
- `prompt.md`：最终传给子 CLI 的 prompt。
- `result.md`：标准 Markdown 结果文档。
- `stdout.log`：非交互模式 stdout。
- `stderr.log`：非交互模式 stderr。
- `events.jsonl`：One Code CLI 事件流。
- `command.json`：解析后的命令、参数、环境变量 key、cwd、时间信息。
- `artifacts/`：预留目录。

### 10.4 `result.md` 模板

```markdown
# One Code CLI Run Result

## Summary

<!-- 子 CLI 输出摘要；如果无法摘要，则放原始输出。 -->

## Run

- Run ID: ...
- Session ID: ...
- Profile: ...
- Backend: ...
- Model: ...
- Working Directory: ...
- Interactive: ...
- Success: ...
- Exit Code: ...
- Started At: ...
- Finished At: ...

## Prompt

See `prompt.md`.

## Output

...

## Logs

- stdout: `stdout.log`
- stderr: `stderr.log`
- events: `events.jsonl`
```

### 10.5 索引文件

`index.jsonl` 记录 run。

示例：

```json
{"run_id":"run_20260510_080101_abcd","session_id":"sess_abc","profile":"claude-sonnet","backend":"claude","cwd":"E:/project/foo","success":true,"result_path":"E:/project/foo/.occ/runs/run_20260510_080101_abcd/result.md","created_at":"2026-05-10T00:01:01Z"}
```

`session-index.jsonl` 记录 session。

示例：

```json
{"session_id":"sess_abc","profile":"claude-sonnet","backend":"claude","cwd":"E:/project/foo","session_path":"E:/project/foo/.occ/sessions/sess_abc/session.toml","updated_at":"2026-05-10T00:02:40Z"}
```

还应维护用户级 session 索引：

```text
~/.occ/session-index.jsonl
```

这样 `occ sessions show <session-id>` 可以不依赖 cwd。

## 11. 会话恢复

### 11.1 Session ID

One Code CLI 生成自己的 session id：

```text
sess_<timestamp>_<short-random>
```

示例：

```text
sess_20260510_080101_a1b2
```

### 11.2 Run ID

One Code CLI 生成 run id：

```text
run_<timestamp>_<short-random>
```

示例：

```text
run_20260510_080201_c3d4
```

### 11.3 按 session id 恢复

明确指定 session id 优先级最高。

```bash
occ run --session sess_20260510_080101_a1b2 --resume --prompt "继续"
```

等价命令：

```bash
occ sessions resume sess_20260510_080101_a1b2 --prompt "继续"
```

查找顺序：

1. 当前 doc root 的 session index。
2. 用户级 `~/.occ/session-index.jsonl`。
3. 配置中的额外 doc roots。
4. 找不到则报错。

### 11.4 按 profile + cwd 恢复最近会话

如果传入 `--resume` 但没有传 `--session`，则按以下条件找最近 session：

1. profile。
2. cwd。
3. `updated_at` 最新。

示例：

```bash
occ run --profile claude-sonnet --cwd ./repo --resume --prompt "继续刚才的任务"
```

### 11.5 Resume 能力要求

如果 backend 或 profile 不支持子 CLI 原生 resume，必须直接失败。

不允许 metadata-only resume。

错误示例：

```json
{
  "success": false,
  "error": {
    "code": "resume_unsupported",
    "message": "Profile 'opencode-default' does not support native resume."
  }
}
```

## 12. 交互模式

默认是非交互模式。

```bash
occ run --profile claude-sonnet --prompt "修复失败测试"
```

非交互规则：

- One Code CLI 管理 stdin/stdout/stderr。
- 捕获 stdout 和 stderr。
- 必须生成 `result.md`。
- 最终输出由 `--output` 控制。

交互模式：

```bash
occ run --profile claude-sonnet --interactive --cwd ./repo
```

交互规则：

- 子进程继承终端 stdin/stdout/stderr。
- 仍创建 run 元数据。
- 日志捕获尽力而为。
- 如果提供 prompt，则按 backend/profile 策略传入。

## 13. 输出模式

`--output` 控制 One Code CLI 自身输出，不控制子 CLI 原始输出文档。

支持：

- `text`：简短文本，默认。
- `json`：机器可读输出，供其它 agents 调用。
- `path`：只输出 `result.md` 路径。

JSON 示例：

```json
{
  "success": true,
  "run_id": "run_20260510_080201_c3d4",
  "session_id": "sess_20260510_080101_a1b2",
  "profile": "claude-sonnet",
  "backend": "claude",
  "cwd": "E:/project/foo",
  "result_path": "E:/project/foo/.occ/runs/run_20260510_080201_c3d4/result.md",
  "metadata_path": "E:/project/foo/.occ/runs/run_20260510_080201_c3d4/run.toml",
  "exit_code": 0
}
```

## 14. 错误处理

内部应使用结构化错误。`--output json` 时必须输出 JSON 错误。

常见错误：

- `config_not_found`
- `config_parse_failed`
- `profile_not_found`
- `backend_not_found`
- `executable_not_found`
- `cwd_not_found`
- `invalid_prompt_source`
- `session_not_found`
- `resume_unsupported`
- `child_process_failed`
- `timeout`
- `doc_root_not_writable`

JSON 错误示例：

```json
{
  "success": false,
  "error": {
    "code": "profile_not_found",
    "message": "Profile 'claude-ds' was not found."
  }
}
```

## 15. Rust 架构

### 15.1 模块结构

```text
src/
  main.rs
  cli.rs
  config.rs
  profile.rs
  backend/
    mod.rs
    claude.rs
    codex.rs
    opencode.rs
  runner.rs
  session.rs
  run_record.rs
  documents.rs
  ids.rs
  output.rs
  error.rs
  config_ui.rs
  skills.rs
```

### 15.2 推荐依赖

- `clap`：CLI 参数解析。
- `serde`：序列化。
- `toml`：TOML 配置。
- `serde_json`：JSON 输出与索引。
- `anyhow`：应用级错误。
- `thiserror`：结构化错误。
- `time` 或 `chrono`：时间。
- `uuid` 或随机后缀生成器：id。
- `directories`：用户目录。
- `tokio`：异步进程、超时、本地 config UI 服务。
- `axum` 或 `tiny_http`：本地 HTML 配置服务。

### 15.3 子进程执行

要求：

- 直接 spawn 子进程，不通过 shell 字符串。
- 设置 cwd。
- 注入 profile env。
- 解析 executable。
- 构造安全参数数组。
- 支持非交互捕获。
- 支持交互继承 stdio。
- 支持 timeout。
- 保存 command metadata。

## 16. MVP 范围

### 16.1 首版包含

- Rust CLI 项目。
- 主命令 `occ`。
- 包名 / 项目标识 `one-code-cli`。
- TOML 配置。
- 内置 backend：`claude`、`codex`、`opencode`。
- 每个 backend 的内置默认配置。
- ordered profiles。
- backend 级默认 profile。
- `run`。
- `doctor`。
- `profiles list/show/test`。
- `backends list/show`。
- `config init/path/show/validate/ui/export-html`。
- `sessions list/show/resume/latest`。
- `runs list/show/open`。
- `skills list/show/export/install/doctor`。
- `--cwd`。
- `--profile` 和 `--backend`。
- `--prompt`、`--prompt-file`、`--stdin`。
- 默认非交互模式。
- 交互模式。
- Markdown result 文档。
- run metadata。
- session metadata。
- 按 session id 恢复。
- 按 profile + cwd 恢复最近 session。
- 不支持原生 resume 时直接失败。
- `--output text|json|path`。
- `--dry-run`。
- `--` 参数透传。
- 内置 skills 模板。
- skills 导出到其它 agents skills 目录。

### 16.2 首版不包含

- 多 agent 编排。
- 沙箱隔离。
- 权限策略。
- 安全 allowlist/denylist。
- 模型 provider 管理。
- HTTP API 服务。
- MCP server。
- GUI/TUI。
- prompt 模板市场。
- 插件系统。
- 在线 skill 市场。

## 17. 后续扩展

可后续加入：

- prompt 模板。
- backend 插件系统。
- MCP server 模式。
- HTTP API 模式。
- 更完整 transcript 标准化。
- 文件变更摘要。
- Git 集成。
- 跨 run 搜索。
- 全局 run 数据库。
- shell completion。
- npm/Homebrew/Scoop 安装包。
- 第三方 skill 仓库。

## 18. 待确认问题

1. 是否最终采用 `One Code CLI` 作为项目名、`occ` 作为主命令？
2. 是否使用 `one-code-cli` 作为包名 / 项目标识？
3. 默认项目目录是否确定为 `.occ/`？
4. `config init` 默认创建项目级配置，还是询问创建用户级/项目级配置？
5. HTML 配置页面首版是否必须支持本地服务写回 TOML，还是只需要静态 HTML 导入/导出 TOML？
6. backend 内置默认参数映射需要以当前安装的 CLI 版本实测后确定，是否允许首版先做保守默认并依赖 `args_strategy = "override"` 兜底？
7. 内置 skills 首版是否默认安装到 `~/.agents/skills`，还是只提供 `occ skills install --target` 手动安装？


