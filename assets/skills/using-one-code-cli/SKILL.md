---
name: using-one-code-cli
description: Delegate coding tasks through occ with visible setup, controlled execution, and result artifact review.
---

# Use One Code CLI

Use this skill when an agent delegates a coding task to another coding-agent CLI through `occ`.

`occ` is the dispatcher. The worker is selected with either an exact `--agent` or a CLI such as `--cli claude`, `--cli codex`, `--cli opencode`, or `--cli gemini`.

## Required inputs

- Working directory for the delegated task.
- Task prompt text, unless starting a foreground interactive session.
- Either an exact agent or a CLI.
- Optional model selection when the user, project, or delegating agent specifies a model.
- Optional effort selection when the user, project, or delegating agent specifies a reasoning level.
- Whether the user wants supervised foreground execution or automated non-interactive execution.

## Storage model

- Run artifacts are written under `doc_root/runs/<run_id>/`.
- The default `doc_root` is the user's `~/.occ`, so runs are centralized under `~/.occ/runs/...`.
- To write artifacts inside a project, pass `--doc-root <path>` or configure `doc_root = ".occ"` in an occ config file.
- User-level session bookkeeping also uses `~/.occ`.

## Execution paths

### Fast path: known environment

Use this when `occ` is already known to work and the user or project specifies the agent/CLI.

1. State the intended `cwd`, agent/CLI, mode, prompt source, `doc_root`, and whether files may be modified.
   Include model and effort when they are specified. If they are not specified, do not invent placeholder values; let the selected CLI, occ agent, session, or native CLI config choose defaults.
2. Run the task directly once the parameters are clear. Do not run `--dry-run` as a routine preflight.
3. Wait for `occ run` or the shell execution tool to finish before reporting the delegated result.
4. Parse JSON output, note `model` / `model_source` and `effort` / `effort_source` when present, then read `result_path` first.
5. Inspect `metadata_path`, `stdout.log`, `stderr.log`, or `events.jsonl` only when the result is missing, incomplete, failed, or ambiguous.

Do not run `occ doctor`, `occ agents list`, or `occ clis list` on every invocation when the environment is already known.

### Diagnostic path: unknown or failing environment

Use this when agent/CLI selection is unclear, `occ` failed, config may be stale, or the user asked for environment diagnosis.

1. Run `occ doctor`.
2. Run `occ agents list` only if agent selection is unknown or agent resolution failed.
3. Run `occ clis list` only if CLI support is unknown or CLI resolution failed.
4. Retry the smallest real command needed to verify the fix. Use `--dry-run` only when inspecting command construction is the goal or real execution would be unwanted.

## Foreground and background policy

- If the user wants supervision, default to foreground interactive mode. `occ run` without `--prompt`, `--prompt-file`, or `--stdin` enters interactive mode; `--interactive` can be used explicitly.
- Do not launch long non-interactive runs silently. First show the command parameters: `cwd`, agent/CLI, prompt source, `doc_root`, and permission posture.
- Pass `--model` and `--effort` when the caller provides a model or reasoning level. If none is provided, omit those flags and use configured/default selection.
- Prefer normal blocking execution from shell tools: start `occ run`, wait for it to complete, then read the JSON result. Avoid backgrounding delegated CLIs unless the user asks.
- Use non-interactive execution when the task needs automation or the user accepts that the child CLI TUI will not be visible.
- For supervised non-interactive runs, pass `--stream` so child stdout/stderr is mirrored to parent stderr while JSON remains on stdout.
- By default, do not add `--timeout`; some CLIs and models are slow, and failures usually exit on their own. Add a timeout only when the user or project requires a hard limit. If you set one, prefer at least `10m` so long tasks have time to finish. `occ` passes the value through verbatim — it does not raise a short timeout — so a too-small cap will cut the run short. Resume still works after a kill because `occ` persists the session id before launching the worker.
- Do not terminate a child process only because stdout is quiet. Some CLIs buffer output until completion.

### Shell execution guidance

- Run `occ` directly through the shell tool and let that shell call block until `occ` exits.
- Do not use background forms such as `&`, `Start-Job`, `nohup`, or `Start-Process` without `-Wait`.
- If `Start-Process` is unavoidable, use `-Wait -PassThru`, then check the process exit code before reading run artifacts.
- Do not treat "process started" as completion. Read `result_path` only after the blocking command returns successfully or reports a failure JSON.

## Commands

Foreground supervised session:

```bash
occ run --cli claude --cwd <cwd> --interactive
```

Non-interactive with a short prompt:

```bash
occ run --agent <agent> --cwd <cwd> --prompt "<task prompt>" --non-interactive --stream --output json
```

Explicit model selection:

```bash
occ run --agent <agent> --cwd <cwd> --prompt "<task prompt>" --model <model> --non-interactive --stream --output json
```

Explicit effort selection:

```bash
occ run --agent <agent> --cwd <cwd> --prompt "<task prompt>" --effort <level> --non-interactive --stream --output json
```

Non-interactive with a longer inline prompt:

```bash
printf '%s\n' "<task prompt>" | occ run --agent <agent> --cwd <cwd> --stdin --non-interactive --stream --output json
```

Use `--prompt-file` only when the prompt already exists as a file or the worker must reference that exact file.

If the agent is unknown but CLI is known:

```bash
occ run --cli claude --cwd <cwd> --prompt "<task prompt>" --non-interactive --stream --output json
```

The same pattern works with `--model <model>` and `--effort <level>` when the caller specifies a model or reasoning level. Do not copy example model names into real commands unless they were requested.

Add `--timeout <duration>` only when a hard execution cap is needed, and prefer at least `10m`. `occ` honours the value verbatim, so a short cap will cut the run short — though resume still works, since the session is persisted up front.

Dry-run only when command construction needs inspection and execution is not desired:

```bash
occ run --cli claude --cwd <cwd> --prompt "test" --non-interactive --dry-run --output json
```

## Monitoring and artifacts

Expected JSON output:

```json
{
  "success": true,
  "run_id": "run_...",
  "session_id": "sess_...",
  "agent": "claude",
  "cli": "claude",
  "model": "default-or-detected-model",
  "model_source": "cli-config",
  "effort": null,
  "effort_source": "none",
  "cwd": "E:/project/repo",
  "result_path": "C:/Users/user/.occ/runs/run_.../result.md",
  "metadata_path": "C:/Users/user/.occ/runs/run_.../run.toml",
  "exit_code": 0
}
```

Read order after completion:

1. `result_path`
2. `metadata_path`
3. `stdout.log`
4. `stderr.log`
5. `events.jsonl`

If the parent agent cannot stream the child process output, treat silence as normal until completion, a configured timeout, or user interruption. Give the user the run directory and current wait policy instead of repeatedly killing, restarting, or posting status spam.

## Resume

When continuing a previous delegated task, prefer resume mode so the worker can recover its native CLI context. Do not start a fresh task just to continue a prior session when a `session_id` is available.

Do not assume every CLI supports native resume. `occ` keeps its own `session_id` and may also know a CLI-native session id.

```bash
occ run --session <session-id> --resume --prompt "<follow-up prompt>" --output json
```

If no session id is available:

```bash
occ run --resume --agent <agent> --cwd <cwd> --prompt "<follow-up prompt>" --output json
```

If `backend_session_missing` or `resume_unsupported` is returned, stop retrying resume for that session/agent unless the user asks otherwise.

## Safety rules

- Do not guess an agent when the user requested a specific one.
- Prefer exact `--agent` when configured by the user or project.
- Prefer `--stdin` over temporary prompt files for long inline prompts.
- Treat `result.md` as the authoritative delegated result.
- Do not assume stdout alone contains the final answer.
- Do not configure or install sub CLIs unless the user asks.
- Do not modify repository files from a delegated review task unless the user explicitly requested implementation.
