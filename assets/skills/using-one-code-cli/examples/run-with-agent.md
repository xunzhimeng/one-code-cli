# Run with an agent

Use this when the user or project has specified the exact `occ` agent.

If the caller specifies a model or reasoning level, pass `--model` or `--effort`. If not, let the agent, session, native CLI config, or CLI itself choose model and effort.

```bash
occ run --agent <agent> --cwd <cwd> --prompt "<task prompt>" --non-interactive --stream --output json
```

With an explicit model:

```bash
occ run --agent <agent> --cwd <cwd> --prompt "<task prompt>" --model <model> --non-interactive --stream --output json
```

With an explicit reasoning level:

```bash
occ run --agent <agent> --cwd <cwd> --prompt "<task prompt>" --effort <level> --non-interactive --stream --output json
```

For longer inline prompts:

```bash
printf '%s\n' "<task prompt>" | occ run --agent <agent> --cwd <cwd> --stdin --non-interactive --stream --output json
```

Run the command in normal blocking shell execution and wait for it to finish. Do not use `&`, `Start-Job`, `nohup`, or `Start-Process` without `-Wait`. After completion, parse JSON and read `result_path` first.
