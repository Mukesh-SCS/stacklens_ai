# Demonstration Video Script: StackLens AI

**Target Duration:** Under 3 minutes (02:45 total target)  
**Track:** Best Apps and Agents — Nebius × NVIDIA Global AI Hackathon  
**Key Principle:** Clear separation of **Observation, Inference, and Recommendation**.  
**Requirement Notice:** Do not record or describe the video as showing successful live Nebius inference until a real API call has succeeded with your active Nebius Token Factory key.

---

## Storyboard & Timing Breakdown

| Timestamp | Segment | Visual On-Screen | Speaker Narration / Action |
|---|---|---|---|
| **0:00 - 0:30** | The Problem | Terminal showing a typical cryptic PyTorch / CUDA driver error (`CUDA driver version is insufficient`). | "Every AI engineer has lost hours to mysterious GPU environment failures. You upgrade a package, run a model, and suddenly CUDA initialization fails. Is it the driver? An old toolkit? Conflicting environment variables? Typical LLMs guess blindly and hallucinate destructive commands. We built StackLens AI to ground troubleshooting strictly in evidence." |
| **0:30 - 0:55** | The Offline Observation | Terminal showing `stacklens snapshot > host.json` and `stacklens diff host-working.json host-broken.json`. | "StackLens begins as an offline-first Rust systems tool. It queries sysfs, `/proc/driver/nvidia`, and CUDA toolkits without cloud dependencies, producing a deterministic, versioned snapshot. And with `stacklens diff`, you can structurally compare a working baseline against a broken host in seconds." |
| **0:55 - 1:30** | Web UI & Deterministic Rules | StackLens AI web interface (`http://127.0.0.1:8000`). Load **Scenario 01: Outdated Driver** and click **Analyze Snapshot**. | "Now let's open the StackLens AI web companion. Reviewers don't even need an NVIDIA GPU—three synthetic scenarios are built right in. When we analyze Scenario 1, our deterministic diagnostic engine immediately flags rule `CUDA-DRIVER-0001`. Using an authoritative local compatibility table, it verifies that driver branch 470 is below the documented CUDA 12.x minimum of 525. Notice: this finding is 100% deterministic, not an LLM guess." |
| **1:30 - 2:05** | NVIDIA Nemotron via Nebius | NVIDIA Nemotron panel in UI showing the model badge (`nvidia/nemotron-3-super-120b-a12b`), grounded explanation, and safe check. | "Next, we turn on NVIDIA Nemotron on Nebius Token Factory. Only curated finding IDs and path-free facts are sent—never private usernames or environment secrets. Running on Nebius's fast inference endpoint, Nemotron-3 Super 120B explains the exact limitation and selects a safe, read-only diagnostic command—`nvidia-smi`—strictly from our rule allowlist. The system will never execute arbitrary or dangerous commands." |
| **2:05 - 2:25** | Scenario 2: Path Disagreement & Uncertainty | Select **Scenario 02: Conflicting Paths**, click **Analyze**. Show `CUDA-PATH-0001` labeled REVIEW REQUIRED (Medium). | "In Scenario 2, StackLens detects that `CUDA_HOME` and `CUDA_PATH` point to different directories. Rather than claiming this is a proven runtime crash, StackLens honestly labels it as a warning that requires verification. Nemotron recommends inspecting `nvcc --version` before touching any system files." |
| **2:25 - 2:45** | Architecture & Conclusion | Show architecture diagram, GitHub repository (`Mukesh-SCS/stacklens`), and test suite passing. | "StackLens AI combines the performance and safety of Rust with the reasoning of NVIDIA Nemotron on Nebius Token Factory. The Rust core remains completely offline; the AI companion provides grounded, verifiable clarity. Check out our open-source repository on GitHub and test it locally today. Thank you!" |

---

## Production & Recording Checklist

- [ ] Web application running at `http://127.0.0.1:8000` with clean browser window (1080p, 100% scale).
- [ ] Terminal window open with high-contrast font showing `stacklens snapshot` and `cargo test`.
- [ ] Nebius API key configured in `.env` and confirmed via `python -m stacklens_ai.preflight --smoke-test`.
  *(Note: If key approval is still pending, demonstrate the clean offline deterministic mode and preflight tool as planned).*
- [ ] Microphone audio clear with zero background noise.
- [ ] Record using OBS Studio or screen capture at 60 FPS / 1080p.
- [ ] Verify final video runtime is strictly under 3 minutes (02:45 recommended).
- [ ] Upload to YouTube as Unlisted/Public and add link to Devpost submission draft.
