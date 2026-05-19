# Run with a CLI

Use this when the user wants a CLI and allows `occ` to resolve that CLI's default agent.

```bash
occ run --cli claude --cwd <cwd> --prompt "<task prompt>" --non-interactive --stream --output json
```

Add `--model <model>` when the worker must use a specific model.

Add `--effort <level>` when the worker must use a specific reasoning level.

Valid CLI values are `claude`, `codex`, `opencode`, and `gemini`.
