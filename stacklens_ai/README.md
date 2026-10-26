# StackLens AI (Hackathon Edition)

StackLens AI is an **optional web companion** for the existing, offline-first
Rust StackLens collector. It diagnoses well-defined NVIDIA driver/CUDA
configuration issues from StackLens schema-v1 JSON snapshots and, when enabled,
asks NVIDIA Nemotron on **Nebius Token Factory** to explain those exact findings
and suggest safe, read-only diagnostic verification steps.

It does not train a new model or claim to resolve all GPU environment failures.
Deterministic code owns the findings. Model prose is labeled as AI-generated and
linked strictly by existing rule IDs. Model-suggested follow-up checks are
restricted to a fixed, rule-specific allowlist.

---

## Local Quick Start

### 1. Configure Environment (Optional for AI Inference)

Copy the safe configuration template:

```bash
cp .env.example .env
```

On Linux/macOS:
```bash
# Leave empty for offline / deterministic-only diagnostic mode:
export NEBIUS_API_KEY="paste-key-locally"
python3 -m stacklens_ai.server
```

On Windows PowerShell:
```powershell
# Copy template
Copy-Item .env.example .env

# Set environment variable locally if desired
$env:NEBIUS_API_KEY = "paste-key-locally"
python -m stacklens_ai.server
```

### 2. Run Preflight Validation

Verify configuration, synthetic scenarios, compatibility tables, and allowlists:

```bash
python -m stacklens_ai.preflight
```

When your Nebius Token Factory API key arrives, smoke-test real inference with:

```bash
python -m stacklens_ai.preflight --smoke-test
```

### 3. Open the Web Application

Open **http://127.0.0.1:8000** in your browser. Three explicitly synthetic snapshots are
included so reviewers do not need a physical NVIDIA GPU.

The default model is `nvidia/nemotron-3-super-120b-a12b`, using the
Nebius Token Factory OpenAI-compatible endpoint.

---

## Capturing Real Snapshots with Rust CLI

To create a live snapshot with the Rust collector on supported Linux hosts:

```bash
cargo run --release -- snapshot > snapshot.json
```

To compare a working baseline snapshot against a broken host:

```bash
cargo run --release -- diff baseline.json broken.json
```

Review and redact your snapshot before uploading. The hosted demo should be
used only with synthetic or non-confidential data.

---

## Authoritative Deterministic Diagnostic Rules

| Rule ID | Detection | Severity | Verification / Allowlisted Check |
|---|---|---|---|
| `CUDA-DRIVER-0001` | Observed driver branch below authoritative documented CUDA major-family minimum | High | `nvidia-smi`, `cat /proc/driver/nvidia/version` |
| `CUDA-PATH-0001` | Conflicting CUDA environment selectors (`CUDA_HOME`, `CUDA_PATH`, `CUDA_ROOT`) | Medium | `echo "CUDA_HOME=$CUDA_HOME CUDA_PATH=$CUDA_PATH CUDA_ROOT=$CUDA_ROOT"` |
| `CUDA-RUNTIME-0001` | Multiple `libcudart` candidate libraries discovered | Low | `ldconfig -p \| grep libcudart`, `echo "LD_LIBRARY_PATH=$LD_LIBRARY_PATH"` |
| `NVIDIA-DRIVER-0001` | Kernel module and userspace NVIDIA driver versions disagree | High | `cat /proc/driver/nvidia/version`, `nvidia-smi` |

> **Note:** No rule firing is **not** evidence that the environment is healthy. The existing Rust project has not yet implemented its ELF or PyTorch collectors.

---

## Security, Guardrails, and Privacy

- **Offline Independence:** The Rust collector and deterministic diagnostic engine operate completely offline without cloud dependencies.
- **Zero Heavy Dependencies:** The entire backend is built with Python's standard library.
- **Privacy Redaction:** Raw filesystem paths, usernames, hostnames, and arbitrary snapshot trees are strictly stripped before outbound model calls.
- **Strict Grounding Validation:** Explanations referencing unverified finding IDs are rejected.
- **Safe Command Allowlists:** Follow-up diagnostic commands suggested by Nemotron are checked against a fixed allowlist. Arbitrary, dangerous, or unallowlisted commands are rejected.
- **Credit Protection:** `STACKLENS_MODEL_CALL_BUDGET` caps total inference calls per process, and a sliding-window limiter prevents abuse.
- **No Mocking:** In the absence of a `NEBIUS_API_KEY`, the application honestly reports deterministic-only mode without fabricating model output.

---

## Running Tests

```bash
# Python unit, regression, config, and preflight tests (41 tests)
python -m unittest discover -s stacklens_ai/tests -v

# Rust workspace tests (96 tests)
cargo test --workspace

# Rust style and clippy checks
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

---

## Docker Deployment

Build and run with container healthchecks:

```bash
docker build -t stacklens-ai .
docker run --rm -p 8000:8000 -e NEBIUS_API_KEY stacklens-ai
```

The container exposes a health check endpoint at `/api/health` that validates service status, Nebius configuration, and credit budget remaining.

---

## Pre-Existing Project Disclosure

StackLens's Rust CLI was created prior to the August 26, 2026 hackathon opening. The hackathon contributions developed in October 2026 include:
- `stacklens_ai` web companion, backend server, and responsive frontend
- Deterministic diagnostic rule engine with authoritative compatibility tables
- NVIDIA Nemotron integration via Nebius Token Factory
- Command allowlist guardrails and grounding validation
- Preflight verification and smoke-testing tools
- Configuration management with credit budget caps
- Expanded test suite and GitHub Actions matrix CI
