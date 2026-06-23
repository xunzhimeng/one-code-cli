# Resume a session

Use this when a previous `occ run` returned a `session_id` and the delegated worker should continue the same task. Prefer this over starting a fresh run so the worker can reuse the prior native CLI context.

```bash
occ run --session <session-id> --resume --cwd <cwd> --prompt "<follow-up prompt>" --non-interactive --stream --output json
```

Wait for the command to finish, then read `result_path`. If the CLI does not support native resume, `occ` returns `resume_unsupported`.
