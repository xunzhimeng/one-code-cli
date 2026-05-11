# Run with a profile

Use this when the user or project has specified the exact `occ` profile.

```bash
occ run --profile <profile> --cwd <cwd> --prompt "<task prompt>" --output json
```

For longer inline prompts:

```bash
printf '%s\n' "<task prompt>" | occ run --profile <profile> --cwd <cwd> --stdin --output json
```

After the command finishes, parse JSON and read `result_path`.
