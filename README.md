# StackLens

StackLens is a Rust-native diagnostic and forensic toolkit for AI compute environments.

It inspects GPU and AI runtime environments and reports evidence for configuration and compatibility problems.

Status: **early development**

## Motivation

AI compute failures are usually stack failures, not single-package failures:

```text
GPU
 ↓
driver
 ↓
CUDA
 ↓
native libraries
 ↓
PyTorch
 ↓
containers
 ↓
application
```

A problem at any layer can surface as a misleading error at another. StackLens gathers the relevant evidence in one place so the report can point at what was observed, not at a guess.

## Principles

- Rust-native standalone binary
- Deterministic diagnostics
- Evidence before conclusions
- No required Python, Node.js, or JVM runtime
- Offline-capable
- Safe reporting (environment collection is allowlisted; no telemetry)
- Cross-platform roadmap; Linux is first

## Current status

Phase 1A foundation. The `stacklens` binary builds, parses commands, collects host/OS information, writes versioned JSON snapshots, and diffs snapshots.

It does **not** yet inspect GPUs, CUDA, ELF libraries, or PyTorch. Those commands exist so the CLI surface is stable; they report that the work is not implemented rather than inventing results.

## Phase 1

Initial focus:

- Linux
- NVIDIA
- CUDA
- PyTorch / LibTorch
- ELF / shared-library diagnostics

See [project_plan.md](project_plan.md) for milestones 1A–1F and later phases.

## Commands

| Command | Status | Purpose |
| --- | --- | --- |
| `stacklens doctor` | Partial | Host summary; GPU/CUDA/library/PyTorch diagnosis not implemented |
| `stacklens gpu` | Planned | NVIDIA GPU and driver inspection |
| `stacklens cuda` | Planned | CUDA toolkit, runtime, and environment |
| `stacklens libs [PATH]` | Planned | ELF shared-library inspection |
| `stacklens torch` | Planned | PyTorch / LibTorch inspection |
| `stacklens snapshot` | Partial | JSON snapshot of collected host data; GPU/CUDA/torch fields are `not_implemented` |
| `stacklens diff <LEFT> <RIGHT>` | Partial | Structural diff of two snapshots |

```bash
stacklens --help
stacklens doctor --help
stacklens libs --help
```

## Example future workflow

```bash
stacklens snapshot > server-good.json
stacklens snapshot > server-bad.json
stacklens diff server-good.json server-bad.json
```

This is useful when a machine that worked yesterday does not work today, or when comparing a known-good host to a broken one. The diff is meant to highlight driver, CUDA, library path, and runtime differences instead of requiring a manual side-by-side review.

Today, `snapshot` and `diff` operate on the host fields that are actually collected. They will gain GPU, CUDA, and library fields as those collectors land.

## Long-term roadmap

Later phases (not started):

- NCCL, cuDNN, TensorRT
- Containers and NVIDIA Container Toolkit
- Windows, WSL, PE/COFF
- AMD / ROCm
- JAX, TensorFlow, ONNX Runtime, vLLM, TensorRT-LLM, llama.cpp, Candle
- `stacklens bundle pytorch` report archives with secret redaction

Details: [project_plan.md](project_plan.md).

## Build

```bash
cargo build --release
```

The binary is `stacklens`.

## Development

```bash
cargo test --workspace
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
```

CI runs fmt, clippy (`-D warnings`), and the workspace tests. Tests do not require an NVIDIA GPU.

## Contributing

The project is early. Useful future contributions include diagnostic rules and reproducible fixtures (command output, ELF samples, snapshots). Open an issue or pull request if you want to work on something.

## License

Apache License 2.0. See [LICENSE](LICENSE).
