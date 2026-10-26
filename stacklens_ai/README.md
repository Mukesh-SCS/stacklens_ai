# StackLens AI (hackathon MVP)

StackLens AI is an **optional web companion** for the existing, offline-first
Rust StackLens collector. It diagnoses a few well-defined NVIDIA driver/CUDA
configuration issues from StackLens schema-v1 JSON snapshots and, when enabled,
asks NVIDIA Nemotron on **Nebius Token Factory** to explain those exact findings.

It does not train a new model or claim to resolve all GPU environment failures.
Deterministic code owns the findings. Model prose is labeled as AI-generated and
linked by existing rule IDs. This is a first iteration, not a security boundary
or a proof that every explanation is factually correct.

## Local quick start

From the repository root:

```bash
python3 -m stacklens_ai.server
```

Open **http://127.0.0.1:8000**. Three explicitly synthetic snapshots are
included so reviewers do not need a physical NVIDIA GPU.

To enable **real model inference**, set `NEBIUS_API_KEY` in the server's
environment. Never put it in HTML, JavaScript, a commit, or a screenshot.

On Linux/macOS:

```bash
export NEBIUS_API_KEY="paste-key-locally"
python3 -m stacklens_ai.server
```

On Windows PowerShell:

```powershell
$env:NEBIUS_API_KEY = "paste-key-locally"
python -m stacklens_ai.server
```

The default model is `nvidia/nemotron-3-super-120b-a12b`, using the
Nebius Token Factory OpenAI-compatible endpoint. To change an available model,
configure `NEBIUS_MODEL`; to change region or endpoint, configure
`NEBIUS_BASE_URL`. These must still point to a qualifying Nebius-hosted
NVIDIA open model for hackathon eligibility.

To create a live snapshot with the Rust collector on supported Linux hosts:

```bash
cargo run --release -- snapshot > snapshot.json
```

Review and redact your snapshot before uploading. The hosted demo should be
used only with synthetic or non-confidential data.

## Diagnostic rules

| Rule | Meaning | Strength |
| --- | --- | --- |
| `CUDA-DRIVER-0001` | Observed driver branch below documented CUDA major-family minimum | Strong evidence for this compatibility limitation; not an application-level root cause |
| `CUDA-PATH-0001` | CUDA_HOME/PATH/ROOT point to different directories | Warning, not proof of an error |
| `CUDA-RUNTIME-0001` | Multiple libcudart candidates found | Informational; loader selection not inspected |
| `NVIDIA-DRIVER-0001` | Kernel and userspace NVIDIA versions disagree | Direct observation, repair not assumed |

No rule firing is **not** evidence that the environment is healthy.
The existing Rust project has not yet implemented its ELF or PyTorch collectors.

## Design and privacy

- Rust collector remains offline and free of mandatory AI dependencies.
- Web companion is implemented with Python's standard library only.
- Incoming snapshot JSON is parsed in memory and not saved by the web server.
- **Only curated finding IDs, titles, and path-free evidence** are sent to
  Nebius; raw environment variables, usernames, filesystem paths, and the
  full snapshot are not included in that model request.
- The snapshot **does reach the web demo server**, so it is not suitable for
  secrets or confidential machine profiles. Future versions should add
  client-side redaction before transport.
- The Nebius API key stays on the server. The public demo has a per-minute
  rate limit and a process-level request cap; configure infrastructure-level
  cost limits before prolonged public deployment.
- No API key means deterministic-only mode, clearly labeled. There is no
  simulated model response.
- Model rule-ID checks prevent fabricated rule identifiers being displayed,
  but do not establish the semantic correctness of model explanations.

## Run tests

```bash
python3 -m unittest discover -s stacklens_ai/tests -v
cargo test --workspace
```

The Python tests are independent of GPUs, network access, and a Nebius key.
The Rust tests still require a Rust toolchain.

## Docker deployment

Build and run:

```bash
docker build -t stacklens-ai .
docker run --rm -p 8000:8000 -e NEBIUS_API_KEY stacklens-ai
```

For Render or another Docker-compatible platform, deploy this repository's
`Dockerfile` and set `NEBIUS_API_KEY` as a server-side secret. Use the
platform-provided `PORT` (the app reads it automatically). Keep the service
reachable through the end of judging. Confirm live model inference from the
deployed service before submitting.

## Suggested demo sequence (under 3 minutes)

1. Introduce StackLens: one Rust snapshot, no guessing from incomplete logs.
2. Load the synthetic old-driver fixture and run analysis.
3. Show the deterministic branch comparison and verification advice.
4. Show a **real** Nemotron explanation with model name and Nebius service
   visible; never pretend the model ran if it did not.
5. Load the selector-conflict fixture to demonstrate that potential issues
   are labeled uncertain, not called confirmed root causes.
6. Show the README, GitHub license, and a genuine local `stacklens snapshot`
   command on a suitable Linux/NVIDIA machine.

## Pre-existing project disclosure

StackLens's Rust CLI predates the August 26, 2026 submission opening. Disclose
this in Devpost. Describe the **new** companion and diagnostic rules only if
they were actually implemented during the submission period, and retain
commit history that demonstrates the changes. Do not present the prior Rust
collectors as new hackathon work.

## Next work, not claimed as complete

- Confirm live inference with real Token Factory credits.
- Validate actual snapshots from supported Linux/NVIDIA environments.
- Add richer PyTorch / ELF collection and deeper compatibility rules.
- Add local-first client-side snapshot redaction.
- Deploy a publicly accessible working URL, then record an honest video.
