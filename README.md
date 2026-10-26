# StackLens

StackLens is a Rust-native diagnostic and forensic toolkit for AI compute environments.

It inspects GPU and AI runtime environments and reports evidence for configuration and compatibility problems.

Status: **early development**

## StackLens AI hackathon companion (October 2026)

The optional [StackLens AI web companion](stacklens_ai/README.md) accepts the
existing JSON snapshot format, applies conservative deterministic rules with
authoritative compatibility tables, and can request evidence-linked NVIDIA Nemotron
explanations through **Nebius Token Factory** with allowlisted read-only verification checks.
The original Rust CLI stays offline-capable; model inference is explicitly opt-in.

Quick start:
```bash
# 1. Run offline preflight validation
python -m stacklens_ai.preflight

# 2. Start the companion server
python -m stacklens_ai.server
```

Open `http://127.0.0.1:8000`, load one of the three synthetic examples, and
select **Analyze Snapshot**. Configure server-side `NEBIUS_API_KEY` in `.env` for live
model calls. Without a key, the application operates in 100% offline deterministic mode.

When your Nebius Token Factory key arrives, test live inference with:
```bash
python -m stacklens_ai.preflight --smoke-test
```

See the [companion README](stacklens_ai/README.md) for full configuration, guardrails,
test procedures, and Docker deployment details.

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
- No telemetry
- Credential-like environment variables are not collected by default
- Cross-platform roadmap; Linux is first

## Current status

Phase 1C. The `stacklens` binary collects host/OS information, Linux NVIDIA GPU/driver evidence (sysfs, `/proc/driver/nvidia/version`, and `nvidia-smi` CSV queries), and Linux CUDA toolkit/runtime evidence. It writes versioned JSON snapshots and diffs them.

StackLens treats these as separate observations, not one "CUDA version":

- NVIDIA driver
- driver CUDA API capability
- CUDA Toolkit
- nvcc
- CUDA runtime (`libcudart`)

It does **not** yet inspect ELF loader resolution, PyTorch, or diagnostic rules. CUDA compatibility output is a data foundation only and is not a diagnosis.

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
| `stacklens doctor` | Partial | Host, NVIDIA/GPU, and CUDA facts; no compatibility conclusions |
| `stacklens gpu` | Implemented | NVIDIA GPU and driver inspection from sysfs, `/proc`, and `nvidia-smi` |
| `stacklens cuda` | Partial | CUDA toolkit, nvcc, runtime-candidate, and environment discovery |
| `stacklens libs [PATH]` | Planned | ELF shared-library inspection |
| `stacklens torch` | Planned | PyTorch / LibTorch inspection |
| `stacklens snapshot` | Partial | Host, NVIDIA/GPU, and CUDA fields; `libraries` and `torch` remain `not_implemented` |
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

`snapshot` currently includes host data, NVIDIA/GPU probes, and CUDA discovery. Library and PyTorch fields remain `not_implemented`.

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

CI runs fmt, clippy (`-D warnings`), workspace tests, and a check against the declared MSRV. Tests do not require an NVIDIA GPU.

## Privacy

Environment collection is allowlisted. Tokens, API keys, and similar credentials are not read.

That does **not** make a raw snapshot safe to publish. Allowlisted values such as `PATH`, `LD_LIBRARY_PATH`, `PYTHONPATH`, `CONDA_PREFIX`, `VIRTUAL_ENV`, and `TORCH_HOME` often contain usernames and local filesystem paths.

Review snapshots before sharing them. Stronger path redaction belongs to the future `bundle` / report work.

## Contributing

The project is early. Useful future contributions include diagnostic rules and reproducible fixtures (command output, ELF samples, snapshots). Open an issue or pull request if you want to work on something.

## License

Apache License 2.0. See [LICENSE](LICENSE).
