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
- Whether the user wants supervised foreground execution or automated non-interactive execution.

## Storage model

- Run artifacts are written under `doc_root/runs/<run_id>/`.
- The default `doc_root` is the user's `~/.occ`, so runs are centralized under `~/.occ/runs/...`.
- To write artifacts inside a project, pass `--doc-root <path>` or configure `doc_root = ".occ"` in an occ config file.
- User-level session bookkeeping also uses `~/.occ`.

## Execution paths

### Fast path: known environment

Use this when `occ` is already known to work and the user or project specifies the target/CLI.

1. State the intended `cwd`, agent/CLI, mode, timeout, and whether files may be modified.
2. For non-interactive work, run `occ run ... --dry-run --output json` first when command shape, permissions, or prompt routing need confirmation.
3. Run the task only after the parameters are clear.
4. Parse JSON output, then read `result_path` first.
5. Inspect `metadata_path`, `stdout.log`, `stderr.log`, or `events.jsonl` only when the result is missing, incomplete, failed, or ambiguous.

Do not run `occ doctor`, `occ agents list`, or `occ clis list` on every invocation when the environment is already known.

### Diagnostic path: unknown or failing environment

Use this when agent/CLI selection is unclear, `occ` failed, config may be stale, or the user asked for environment diagnosis.

1. Run `occ doctor`.
2. Run `occ agents list` only if agent selection is unknown or agent resolution failed.
3. Run `occ clis list` only if CLI support is unknown or CLI resolution failed.
4. Retry the smallest command needed to verify the fix, preferably with `--dry-run` before real execution.

## Foreground and background policy

- If the user wants supervision, default to foreground interactive mode. `occ run` without `--prompt`, `--prompt-file`, or `--stdin` enters interactive mode; `--interactive` can be used explicitly.
- Do not launch long non-interactive runs silently. First show or dry-run the command parameters: `cwd`, agent/CLI, prompt source, `doc_root`, timeout, and permission posture.
- Use non-interactive background-style execution only after parameters are set, the task needs automation, or the user accepts that the child CLI TUI will not be visible.
- For supervised non-interactive runs, pass `--stream` so child stdout/stderr is mirrored to parent stderr while JSON remains on stdout.
- For long non-interactive runs, set a timeout, for example `--timeout 10m`, unless the user explicitly wants no timeout.
- Do not terminate a child process only because stdout is quiet. Some CLIs buffer output until completion.

## Commands

Foreground supervised session:

```bash
occ run --cli claude --cwd <cwd> --interactive
```

Non-interactive with a short prompt:

```bash
occ run --agent <agent> --cwd <cwd> --prompt "<task prompt>" --non-interactive --stream --output json --timeout 10m
```

Non-interactive with a longer inline prompt:

```bash
printf '%s\n' "<task prompt>" | occ run --agent <agent> --cwd <cwd> --stdin --non-interactive --stream --output json --timeout 10m
```

Use `--prompt-file` only when the prompt already exists as a file or the worker must reference that exact file.

If the agent is unknown but CLI is known:

```bash
occ run --cli claude --cwd <cwd> --prompt "<task prompt>" --non-interactive --stream --output json --timeout 10m
```

Dry-run before real execution when parameters need inspection:

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

If the parent agent cannot stream the child process output, treat silence as normal until timeout or exit. Give the user the run directory and current wait policy instead of repeatedly killing or restarting the process.

## Resume

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
