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
- Task prompt text.
- Either an exact `occ` profile or a backend type: `claude`, `codex`, `opencode`, or `gemini`.

## Preferred protocol

1. Discover the local environment with `occ doctor`, `occ profiles list`, and `occ backends list` instead of hardcoding assumptions.
2. Pass the delegated task with `--prompt` for short text or `--stdin` for longer inline text.
3. Use `--prompt-file` only when the task prompt already exists as a file or the worker must reference that file.
4. Run `occ run` with `--output json`.
5. Select workers by exact `--profile` when the user or project specifies one.
6. Select workers by `--backend claude`, `--backend codex`, `--backend opencode`, or `--backend gemini` when the user allows `occ` to resolve the default profile.
7. Parse the JSON response.
8. Read `result_path` first.
9. If needed, inspect `metadata_path`, `events.jsonl`, `stdout.log`, and `stderr.log`.
10. Preserve `session_id` only if you may continue the same delegated task later.

## Basic command

```bash
occ run --profile <profile> --cwd <cwd> --prompt "<task prompt>" --output json
```

For longer inline prompts:

```bash
printf '%s\n' "<task prompt>" | occ run --profile <profile> --cwd <cwd> --stdin --output json
```

If the exact profile is unknown but the backend is known:

```bash
occ run --backend claude --cwd <cwd> --prompt "<task prompt>" --output json
```

## Resume command

Do not assume every backend supports native resume. `occ` keeps its own `session_id` and may also know a backend-native session id. If the selected backend/profile requires a backend-native session id and the session does not have one, `occ` returns `backend_session_missing`.

```bash
occ run --session <session-id> --resume --prompt "<follow-up prompt>" --output json
```

If you do not have a session id, resume the latest matching session:

```bash
occ run --resume --profile <profile> --cwd <cwd> --prompt "<follow-up prompt>" --output json
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
- If `error.code` is `backend_session_missing`, stop retrying resume for that session/profile and start a new task unless the user asks otherwise.

## Safety rules for calling agents

- Do not guess a profile if the user requested a specific one.
- Prefer `--stdin` over `--prompt-file` for long inline tasks.
- Use `--prompt-file` only for file-based, reusable, or explicitly file-required tasks.
- Use `--dry-run` before execution if you need to inspect the resolved command.
- Treat `result.md` as the authoritative delegated result.
- Do not assume stdout alone contains the final answer.
- Do not configure or install sub CLIs unless the user asks.
