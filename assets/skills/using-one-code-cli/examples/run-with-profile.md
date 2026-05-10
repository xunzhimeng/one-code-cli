# Run with a profile

Use this when the user or project has specified the exact `occ` profile.

```bash
occ run --profile <profile> --cwd <cwd> --prompt-file <task.md> --output json
```

After the command finishes, parse JSON and read `result_path`.
