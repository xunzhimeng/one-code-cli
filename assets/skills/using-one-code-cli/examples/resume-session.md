# Resume a session

Use this when a previous `occ run` returned a `session_id` and the delegated worker should continue the same task.

```bash
occ run --session <session-id> --resume --cwd <cwd> --prompt "<follow-up prompt>" --output json
```

If the CLI does not support native resume, `occ` returns `resume_unsupported`.
