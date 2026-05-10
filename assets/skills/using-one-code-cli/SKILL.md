---
name: using-one-code-cli
description: Protocol for agents to delegate coding tasks through occ and read result documents.
---

# Use One Code CLI

## Purpose

Use this skill when you are an agent that needs to delegate a coding task to another coding-agent CLI through `occ`.

`occ` is the dispatcher. The target worker is selected with `--profile` or `--backend`.

## Required inputs

- Working directory for the delegated task.
- Task prompt, preferably stored in a prompt file.
- Either an exact `occ` profile or a backend type: `claude`, `codex`, `opencode`, or `gemini`.

## Preferred protocol

1. Write the delegated task to a prompt file.
2. Run `occ run` with `--output json`.
3. Parse the JSON response.
4. Read `result_path` first.
5. If needed, inspect `metadata_path`, `events.jsonl`, `stdout.log`, and `stderr.log`.
6. Preserve `session_id` if you may continue the same task later.

## Basic command

```bash
occ run --profile <profile> --cwd <cwd> --prompt-file <task.md> --output json
```

If the exact profile is unknown but the backend is known:

```bash
occ run --backend claude --cwd <cwd> --prompt-file <task.md> --output json
```

## Resume command

```bash
occ run --session <session-id> --resume --prompt-file <task.md> --output json
```

If you do not have a session id, resume the latest matching session:

```bash
occ run --resume --profile <profile> --cwd <cwd> --prompt-file <task.md> --output json
```

## Expected JSON output

```json
{
  "success": true,
  "run_id": "run_...",
  "session_id": "sess_...",
  "profile": "claude-sonnet",
  "backend": "claude",
  "cwd": "E:/project/repo",
  "result_path": "E:/project/repo/.occ/runs/run_.../result.md",
  "metadata_path": "E:/project/repo/.occ/runs/run_.../run.toml",
  "exit_code": 0
}
```

## Error handling

If `success` is false:

- Read `error.code` and `error.message`.
- Check `exit_code` when present.
- Inspect `stderr.log` if `metadata_path` or `result_path` is present.
- If `error.code` is `resume_unsupported`, stop trying to resume with that profile.

## Safety rules for calling agents

- Do not guess a profile if the user requested a specific one.
- Prefer `--prompt-file` over inline `--prompt` for long tasks.
- Use `--dry-run` before execution if you need to inspect the resolved command.
- Treat `result.md` as the authoritative delegated result.
- Do not assume stdout alone contains the final answer.
- Do not configure or install sub CLIs unless the user asks.
