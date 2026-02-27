# PlainSight

PlainSight is a Rust project that generates project documentation from source code using local LLM models via Ollama.

It currently generates:
- Per-file summaries (`summary.md`)
- Per-file docs (`docs.md`)
- Project summary (`summary.md`)
- Project architecture (`architecture.md`)

## Status

This project is primarily for personal use.

It does not aim to support every language, workflow, or edge case. Behavior and output formats may change at any time as the project evolves.

That said, pull requests are welcome.

## Requirements

- Rust toolchain
- Ollama running locally
- At least one local model installed in Ollama

## Run

Run with defaults (current directory as project root, `docs` as output root):

```bash
cargo run -p plainsight_bin
```

Run with explicit paths:

```bash
cargo run -p plainsight_bin -- /path/to/project --docs-root /path/to/docs
```

Set a custom docs project name:

```bash
cargo run -p plainsight_bin -- /path/to/project --docs-root /path/to/docs --project-name my_project
```

## Output

Generated files are written under your configured docs root, for example:

- `docs/<project>/summary.md`
- `docs/<project>/architecture.md`
- `docs/<project>/.meta.json`
- `docs/<project>/.memory.json`
- `docs/<project>/.source_index.json`
- `docs/<project>/files/<path>/summary.md`
- `docs/<project>/files/<path>/docs.md`

## Notes

- This is an early-stage tool. Expect rough edges.
- Generated content can be wrong. Always verify against source code.
