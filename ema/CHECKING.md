# EMA Diagnostics (`--check --json`)

EMA VSCode eklentisi, `.ema` dosyalarinda tanilama icin su komutu calistirir:

```bash
ema_compiler <file.ema> --check --json
```

Expected stdout format: JSON array

```json
[
  { "line": 3, "col": 10, "message": "..." }
]
```

Notes:
- In `--check` mode the compiler may return **exit code 1** (normal when there are errors).
- When `--json` is enabled, stdout is expected to contain only JSON.

