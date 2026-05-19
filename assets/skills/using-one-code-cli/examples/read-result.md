# Read a result

After `occ run --output json`, first inspect `model` / `model_source` and `effort` / `effort_source` in the JSON when selection matters, then read files in this order:

1. `result_path`
2. `metadata_path`
3. `events.jsonl`
4. `stdout.log`
5. `stderr.log`

Do not rely on natural-language stdout as the only result source.
