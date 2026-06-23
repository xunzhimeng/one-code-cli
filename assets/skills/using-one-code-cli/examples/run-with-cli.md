# Run with a CLI

Use this when the user wants a CLI and allows `occ` to resolve that CLI's default agent.

If the caller specifies a model or reasoning level, pass `--model` or `--effort`. If not, let the selected CLI/default agent use its configured defaults. Do not add `--timeout` or `--dry-run` for normal execution; run the command normally and wait for it to complete.

```bash
occ run --cli claude --cwd <cwd> --prompt "<task prompt>" --non-interactive --stream --output json
```

Add `--model <model>` when the worker must use a specific model.

Add `--effort <level>` when the worker must use a specific reasoning level.

Valid CLI values are `claude`, `codex`, `opencode`, and `gemini`.
