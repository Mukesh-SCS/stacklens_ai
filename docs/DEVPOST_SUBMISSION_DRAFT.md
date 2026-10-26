# Devpost Submission Draft: StackLens AI

**Track:** Best Apps and Agents  
**Hackathon:** Nebius × NVIDIA Global AI Hackathon (October 2026)  
**Required NVIDIA Model:** `nvidia/nemotron-3-super-120b-a12b`  
**Inference Provider:** Nebius Token Factory (`https://api.tokenfactory.us-central1.nebius.com/v1`)  

---

## 1. Project Title and Tagline

- **Project Title:** StackLens AI
- **Tagline:** Evidence-grounded GPU, NVIDIA driver, and CUDA troubleshooting powered by NVIDIA Nemotron on Nebius Token Factory.

---

## 2. Problem Statement

AI compute failures are rarely single-package bugs; they are **stack failures**.

When a PyTorch script, vLLM worker, or CUDA kernel crashes with:
```text
CUDA driver version is insufficient for CUDA runtime version
```
or
```text
CUDA initialization: Unexpected error from cudaGetDeviceCount()
```
engineers often spend hours guessing what broke:
- Was the host driver silently upgraded by `unattended-upgrades`?
- Did `CUDA_HOME` point to a different toolkit than the one discovered by `nvcc` in `PATH`?
- Did the dynamic linker load an older `libcudart.so` from an unexpected virtualenv directory?
- Did the running kernel module version drift from the userspace driver libraries after an incomplete reboot?

Generic LLMs given raw logs often hallucinate non-existent driver versions, suggest destructive commands (`apt-get purge nvidia*`), or jump to conclusions without inspecting the underlying evidence.

---

## 3. Product Description

**StackLens AI** bridges offline systems telemetry and generative AI reasoning through strict **separation of observation, inference, and recommendation**:

1. **Observe (Offline-First Rust Engine):** Standalone native binary inspects sysfs, `/proc/driver/nvidia`, `nvidia-smi` CSV queries, and CUDA toolkits without cloud dependencies, emitting a deterministic, versioned JSON snapshot (Schema v1).
2. **Infer (Authoritative Deterministic Rules):** A conservative diagnostic engine applies verified compatibility rules against authoritative NVIDIA tables—completely independent of the AI model.
3. **Recommend (Evidence-Grounded NVIDIA Nemotron via Nebius):** When enabled, NVIDIA Nemotron (`nemotron-3-super-120b-a12b`) on Nebius Token Factory receives a curated, privacy-redacted fact package. The model explains the exact verified finding IDs and selects read-only follow-up checks strictly from a rule-specific allowlist.

The application **never** automatically executes model-generated commands and never sends private filesystem paths, usernames, or API tokens to cloud endpoints.

---

## 4. Technical Architecture

```text
+-----------------------------------------------------------------------------------+
|                           1. OFFLINE HOST COLLECTION                              |
|                                                                                   |
|  [sysfs / PCI]  [/proc/driver/nvidia]  [nvidia-smi CSV]  [nvcc & libcudart probes] |
|                                       │                                           |
|                                       ▼                                           |
|                     stacklens CLI (Rust Standalone Binary)                        |
|                                       │                                           |
|                                       ▼                                           |
|                         StackLens Schema-v1 Snapshot                              |
|               (Structured JSON: Host, Driver, CUDA, Selectors)                    |
+-----------------------------------------------------------------------------------+
                                        │
                                        │ (Upload or Paste)
                                        ▼
+-----------------------------------------------------------------------------------+
|                        2. STACKLENS AI COMPANION BACKEND                          |
|                                                                                   |
|                             stacklens_ai/server.py                                |
|                                       │                                           |
|                                       ▼                                           |
|                        Deterministic Diagnostic Engine                            |
|                             (stacklens_ai/engine.py)                              |
|                                       │                                           |
|               ┌───────────────────────┴────────────────────────┐                  |
|               ▼                                                ▼                  |
|     Authoritative Tables                            Deterministic Findings        |
|  - CUDA 11.x: driver >= 450                         - CUDA-DRIVER-0001 (High)     |
|  - CUDA 12.x: driver >= 525                         - CUDA-PATH-0001 (Med)        |
|  - CUDA 13.x: driver >= 580                         - CUDA-RUNTIME-0001 (Low)     |
|  - NVIDIA Kernel vs Userspace                       - NVIDIA-DRIVER-0001 (High)   |
|                                                                │                  |
|                                                                ▼                  |
|                                                   Sanitized Evidence Redaction    |
|                                                   (Zero paths/usernames/secrets)  |
+-----------------------------------------------------------------------------------+
                                                                 │
                                                                 ▼
+-----------------------------------------------------------------------------------+
|                          3. NEBIUS TOKEN FACTORY INFERENCE                        |
|                                                                                   |
|                               Nebius Token Factory                                |
|               POST https://api.tokenfactory.us-central1.nebius.com/v1             |
|                                       │                                           |
|                                       ▼                                           |
|                 nvidia/nemotron-3-super-120b-a12b                                 |
|                                       │                                           |
|                                       ▼                                           |
|                          Structured JSON Completion                               |
|        - Grounding Verification: exact finding IDs only (reject invented IDs)     |
|        - Command Guardrail: select only from rule's fixed read-only allowlist     |
+-----------------------------------------------------------------------------------+
                                        │
                                        ▼
+-----------------------------------------------------------------------------------+
|                           4. RESPONSIVE WEB INTERFACE                             |
|                                                                                   |
|  - Status Badge: Real-time Nebius connectivity & NVIDIA Nemotron readiness        |
|  - Synthetic Demonstrations: 3 pre-built scenarios for reviewers without GPUs     |
|  - Separate Presentation:                                                         |
|      * Deterministic Observation & Fact Bullet Points                             |
|      * Nemotron Contextual Explanation (Grounded in Verified Evidence)            |
|      * Safe Allowlisted Verification Commands (One-Click Copy)                    |
|  - Forensic Diff Workflow Guide: stacklens diff <LEFT> <RIGHT>                   |
+-----------------------------------------------------------------------------------+
```

---

## 5. Features Demonstrated

1. **Synthetic & Live Snapshot Ingestion:** Load 3 synthetic failure scenarios (Outdated driver, Conflicting selectors, Driver mismatch) or upload real machine snapshots from `stacklens snapshot`.
2. **Authoritative Compatibility Verification:** Deterministic rules detect driver-branch deficiency (`CUDA-DRIVER-0001`), conflicting environment selectors (`CUDA-PATH-0001`), multiple `libcudart` candidates (`CUDA-RUNTIME-0001`), and kernel-userspace version desynchronization (`NVIDIA-DRIVER-0001`).
3. **Evidence-Grounded Nemotron Explanations:** Real model inference requests explanations tied strictly to confirmed finding IDs.
4. **Command Allowlist Guardrails:** Nemotron proposes read-only verification commands (e.g., `nvidia-smi`, `cat /proc/driver/nvidia/version`, `nvcc --version`) chosen strictly from a fixed, rule-specific allowlist. Dangerous or arbitrary commands are rejected.
5. **Separation of Concerns:** Clear visual and logical distinction between observed telemetry, deterministic inferences, and model-generated explanations.
6. **Snapshot Diffing Workflow:** Instructions and tooling to structurally compare a working baseline snapshot against a degraded host using `stacklens diff`.
7. **Offline-Resilient Operation:** When the API key is not configured, the entire deterministic diagnostic suite continues to function without simulated mocks.

---

## 6. How NVIDIA Nemotron Is Used

StackLens AI uses `nvidia/nemotron-3-super-120b-a12b` via Nebius Token Factory.

Instead of prompting the model with an arbitrary multi-megabyte log dump, StackLens AI supplies a compact, redacted JSON package containing:
- Verified `rule_id`
- Rule title
- Curated factual evidence strings
- Rule interpretation
- Allowlist of permitted read-only verification commands

Nemotron is prompted with a strict system constraint:
1. Explain only the provided `rule_id` items in 1–2 concise sentences grounded strictly in the supplied facts.
2. Select a single read-only follow-up check from `allowed_follow_up_checks`.
3. Do not invent causes, unseen machine state, or unlisted commands.

In the backend (`nebius.py`), model responses are validated:
- Any explanation referencing an unverified finding ID is rejected.
- Any suggested command not in the rule's allowlist is rejected and replaced with the rule's safe default check.

---

## 7. How Nebius Token Factory Is Used

Nebius Token Factory hosts `nvidia/nemotron-3-super-120b-a12b` over an OpenAI-compatible API endpoint:
- **Endpoint:** `https://api.tokenfactory.us-central1.nebius.com/v1/chat/completions`
- **Authentication:** Server-side `Authorization: Bearer $NEBIUS_API_KEY`
- **Budget Control:** `STACKLENS_MODEL_CALL_BUDGET` caps total inference calls per server process to protect developer credits during demonstrations.
- **Rate Limiting:** Per-IP sliding-window limiter prevents abuse.

---

## 8. Technology Stack

- **Systems Telemetry (Rust):**
  - `stacklens-core`: Schema-v1 snapshot data models and structural diff engine.
  - `stacklens-cli`: Command-line tool (`snapshot`, `doctor`, `gpu`, `cuda`, `diff`).
  - `stacklens-nvidia`: Linux sysfs, `/proc`, and `nvidia-smi` parsers.
  - `stacklens-cuda`: nvcc, toolkit, runtime library, and environment discovery.
  - `stacklens-system`: Host info, process execution, and allowlisted env collection.
- **Backend & AI Agent (Python 3.11+ Standard Library):**
  - `stacklens_ai/config.py`: Dependency-free `.env` parser and budget manager.
  - `stacklens_ai/engine.py`: Authoritative deterministic compatibility rules and command allowlists.
  - `stacklens_ai/nebius.py`: Nebius Token Factory HTTP client, grounding validation, and smoke-test tooling.
  - `stacklens_ai/server.py`: Standard library HTTP server with security headers (CSP, nosniff) and request size bounds.
  - `stacklens_ai/preflight.py`: Preflight verification CLI tool.
- **Frontend:**
  - Vanilla HTML5 / Modern CSS3 (custom dark aesthetic, responsive grid, micro-animations, accessible contrast).
  - Vanilla ES6 JavaScript (zero node/npm dependencies, safe DOM construction).
- **DevOps & Testing:**
  - GitHub Actions CI (Rust fmt, clippy, unit tests on MSRV + Python 3.11/3.12 matrix).
  - Containerization via Docker (`Dockerfile` + `.dockerignore`).

---

## 9. Main Engineering Challenges

1. **Preventing LLM Hallucinations in Systems Diagnostics:** In systems programming, a model inventing a library path or recommending `rm -rf /usr/local/cuda` is unacceptable. We resolved this by forcing deterministic code to own all findings, restricting model prose to verified rule IDs, and enforcing an allowlist for follow-up commands.
2. **Authoritative Version Thresholds vs Snapshot Spoofing:** Uploaded JSON files could contain manipulated or corrupted `documented_minimum_driver_branch` fields. We implemented an internal authoritative compatibility table in `engine.py` that overrides untrusted values inside snapshots.
3. **Zero-Dependency Lightweight Architecture:** We avoided heavy dependencies (e.g., langchain, flask, pydantic) in the Python web companion, building the entire backend with Python's standard library. This ensures fast Docker build times (<10s) and zero supply-chain vulnerabilities.
4. **Privacy-Preserving Telemetry:** Raw snapshots contain paths and usernames. We built sanitization routines in `engine.py` ensuring that zero usernames, personal paths, or environment secrets are transmitted to cloud inference endpoints.

---

## 10. Honest Results and Limitations

- **What Works:**
  - Full offline snapshot generation and diffing with Rust CLI.
  - 100% of offline deterministic rules (`CUDA-DRIVER-0001`, `CUDA-PATH-0001`, `CUDA-RUNTIME-0001`, `NVIDIA-DRIVER-0001`).
  - Strict grounding validation and allowlist filtering for Nemotron responses.
  - Responsive web interface with three synthetic demonstration fixtures.
  - 41 Python unit tests and 96 Rust tests passing in CI.
- **Current Limitations:**
  - Phase 1C does not yet implement ELF loader symbol resolution or PyTorch library inspection.
  - Grounding validation ensures rule-ID integrity and command safety; it does not constitute a mathematical proof of arbitrary English prose correctness.
  - Live model inference requires a valid Nebius Token Factory API key; in its absence, the server operates honestly in offline deterministic mode without fake responses.

---

## 11. Disclosure of Pre-Existing Work vs Hackathon Contributions

To comply fully with hackathon rules, we explicitly distinguish the pre-existing repository foundation from work developed during the hackathon submission period:

### Pre-Existing Foundation (Created before August 26, 2026):
- Rust workspace (`stacklens-core`, `stacklens-cli`, `stacklens-cuda`, `stacklens-nvidia`, `stacklens-system`).
- Schema-v1 snapshot format and `stacklens diff` implementation.
- Basic sysfs, `/proc`, and `nvidia-smi` parsers.
- Initial Apache 2.0 license.

### Hackathon Contributions (Developed October 2026):
- **StackLens AI Web Companion:** Modern responsive interface for snapshot inspection and guided troubleshooting.
- **Deterministic Diagnostic Engine (`engine.py`):** Authoritative local compatibility tables and four concrete diagnostic rules.
- **Nebius Token Factory Integration (`nebius.py`):** OpenAI-compatible inference client for `nvidia/nemotron-3-super-120b-a12b`.
- **Command Allowlist Guardrails:** Strict rejection of arbitrary model commands in favor of rule-specific read-only verification commands.
- **Preflight & Smoke-Testing Tooling (`preflight.py`):** Offline sanity verification and one-command live inference smoke test.
- **Configuration & Security (`config.py`):** Server-side secret loading, credit budget caps, per-IP rate limiting, and path redaction.
- **Comprehensive Test Suite:** 41 automated Python unit and regression tests.
- **GitHub Actions Python CI:** Matrix testing across Python 3.11 and 3.12.
- **Deployment & Documentation:** Production `Dockerfile`, `.dockerignore`, Devpost submission materials, and video demonstration script.

---

## 12. Public Repository URL Placeholder

- **GitHub Repository:** `https://github.com/Mukesh-SCS/stacklens`

---

## 13. Working Demonstration URL Placeholder

- **Live Application URL:** `[PENDING DEPLOYMENT / SUBMISSION URL]`
- **Local Quickstart:** `python -m stacklens_ai.server` (visit `http://127.0.0.1:8000`)

---

## 14. Public Video URL Placeholder

- **YouTube Demonstration Video:** `[PENDING RECORDING / YOUTUBE URL]`

---

## 15. Feedback on Nebius and NVIDIA Tooling

- **NVIDIA Nemotron (`nvidia/nemotron-3-super-120b-a12b`):** Nemotron-3 Super is exceptionally capable at structured JSON completions and adherence to strict developer system constraints. Its ability to concisely explain systems-level dependencies without rambling makes it an ideal model for developer tool agents.
- **Nebius Token Factory:** The OpenAI-compatible `/chat/completions` API simplifies integration into Python standard library clients without requiring proprietary SDKs. Providing standard sub-minute token generation speeds enables smooth interactive troubleshooting interfaces. Clear budget tracking and account dashboards are highly appreciated for cost control during hackathon prototyping.
