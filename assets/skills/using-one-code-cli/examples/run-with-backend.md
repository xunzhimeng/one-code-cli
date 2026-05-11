# Run with a backend

Use this when the user wants a backend type and allows `occ` to resolve the default profile.

```bash
occ run --backend claude --cwd <cwd> --prompt "<task prompt>" --output json
```

Valid backend values are `claude`, `codex`, `opencode`, and `gemini`.
