# StackLens

StackLens inspects GPU and AI runtime environments and reports **evidence**, not guesses.

This repo has two parts:

1. **StackLens AI** — a local web app that diagnoses NVIDIA driver / CUDA issues from a JSON snapshot. This is the fastest way to try the project.
2. **`stacklens` CLI** — a Rust collector that captures a live machine snapshot (Linux first). Optional if you only want to run the web demo.

No pip packages are required for StackLens AI. It uses Python’s standard library only.

---

## Run StackLens AI

### Requirements

- Python 3.10 or newer
- A browser
- (Optional) a [Nebius Token Factory](https://tokenfactory.nebius.com) API key for NVIDIA Nemotron explanations

You do **not** need an NVIDIA GPU to try the demo. Three synthetic snapshots are included.

### 1. Open a terminal in this folder

```powershell
cd path\to\stacklens
```

On macOS or Linux:

```bash
cd path/to/stacklens
```

### 2. (Optional) copy the env file

Works fully offline without a key. Copy the template only if you want AI explanations later.

```powershell
Copy-Item .env.example .env
```

```bash
cp .env.example .env
```

Leave `NEBIUS_API_KEY` empty for deterministic-only mode. To enable Nemotron, put your key in `.env`:

```text
NEBIUS_API_KEY=your-key-here
NEBIUS_MODEL=nvidia/nemotron-3-super-120b-a12b
```

### 3. Check that everything is in place

```powershell
python -m stacklens_ai.preflight
```

```bash
python3 -m stacklens_ai.preflight
```

You should see `[PASS]` checks. `[INFO] Nebius API Key - Empty` is expected if you skipped the key.

### 4. Start the server

```powershell
python -m stacklens_ai.server
```

```bash
python3 -m stacklens_ai.server
```

You should see:

```text
StackLens AI server listening on http://127.0.0.1:8000
Mode: OFFLINE / DETERMINISTIC ONLY
```

or `Mode: NVIDIA Nemotron (...)` if a key is configured.

### 5. Open the app

In your browser go to:

**http://127.0.0.1:8000**

Then:

1. Pick one of the three example snapshots (driver too old, selector conflict, or driver component mismatch).
2. Click **Analyze Snapshot**.
3. Read the deterministic findings (rule IDs such as `CUDA-DRIVER-0001`).
4. If you configured a Nebius key, you can request an evidence-linked Nemotron explanation. Without a key, findings still appear; there is no fake AI output.

Stop the server with `Ctrl+C`.

### If Python is not found

Windows:

```powershell
py -3 -m stacklens_ai.preflight
py -3 -m stacklens_ai.server
```

Confirm the version:

```powershell
python --version
```

---

## What the demo examples show

| Example | What it demonstrates |
| --- | --- |
| Driver too old | Driver branch below the documented CUDA family minimum (`CUDA-DRIVER-0001`) |
| Selector conflict | `CUDA_HOME` / `CUDA_PATH` / `CUDA_ROOT` disagree, plus extra `libcudart` candidates |
| Driver component mismatch | Kernel module and userspace NVIDIA driver versions disagree (`NVIDIA-DRIVER-0001`) |

No finding firing does **not** mean the environment is healthy. The collector does not yet inspect ELF loader resolution or PyTorch.

---

## Optional: live NVIDIA Nemotron

When `NEBIUS_API_KEY` is set, StackLens AI can ask NVIDIA Nemotron (`nvidia/nemotron-3-super-120b-a12b`) on Nebius Token Factory to explain **existing** findings and suggest **allowlisted, read-only** checks.

Test the live connection:

```powershell
python -m stacklens_ai.preflight --smoke-test
```

Guardrails:

- Findings come from local rules, not from the model.
- Paths and usernames are stripped before any model call.
- Suggested commands must match a fixed allowlist.
- `STACKLENS_MODEL_CALL_BUDGET` (default 100) caps inference calls per server process.

---

## Optional: Docker

```bash
docker build -t stacklens-ai .
docker run --rm -p 8000:8000 stacklens-ai
```

With a key:

```bash
docker run --rm -p 8000:8000 -e NEBIUS_API_KEY=your-key-here stacklens-ai
```

Then open http://127.0.0.1:8000

---

## Optional: capture a real snapshot (Rust CLI)

On a Linux host with Rust installed:

```bash
cargo run --release -p stacklens-cli -- snapshot > snapshot.json
```

Review and redact the file before uploading it in the web app. Raw snapshots can contain usernames and local paths (`PATH`, `LD_LIBRARY_PATH`, and similar).

Other CLI commands:

```bash
cargo run --release -p stacklens-cli -- doctor
cargo run --release -p stacklens-cli -- gpu
cargo run --release -p stacklens-cli -- cuda
```

The CLI is offline-capable. It does not call Nebius.

---

## Tests

```powershell
python -m unittest discover -s stacklens_ai/tests -v
```

```bash
cargo test --workspace
```

---

## Privacy

- Environment collection is allowlisted. Tokens and API keys are not collected by the CLI.
- StackLens AI does not send a snapshot to Nebius unless you enable AI and a key is configured.
- A raw snapshot is still not automatically safe to publish. Review it before sharing.

---

## License

Apache License 2.0. See [LICENSE](LICENSE).

More implementation detail for the web companion: [stacklens_ai/README.md](stacklens_ai/README.md).
The engineering roadmap lives in [project_plan.md](project_plan.md).
