# Run with an agent

Use this when the user or project has specified the exact `occ` agent.

```bash
occ run --agent <agent> --cwd <cwd> --prompt "<task prompt>" --non-interactive --stream --output json
```

For longer inline prompts:

```bash
printf '%s\n' "<task prompt>" | occ run --agent <agent> --cwd <cwd> --stdin --non-interactive --stream --output json
```

After the command finishes, parse JSON and read `result_path`.
