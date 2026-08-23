# StackLens project plan

Internal engineering roadmap. Public status and command documentation live in `README.md`.

## 1. Vision

StackLens is intended to become a deterministic diagnostic and forensic toolkit for AI compute environments.

The product is a standalone Rust binary. A user downloads it, runs it, and gets an evidence-based report. StackLens is not an assistant that guesses what might be wrong. It inspects the machine, records observations, evaluates those observations with deterministic rules, and reports what the evidence supports.

### Principles

- Inspect rather than guess.
- Collect evidence before drawing conclusions.
- Findings are deterministic: the same inputs produce the same report.
- Reports are reproducible and serializable.
- No hidden telemetry. Diagnostics do not make network requests unless a future feature explicitly requires it and documents that fact.
- Safe by default. Environment collection is allowlist-based. Report bundling must redact secrets.
- Useful without internet access.
- One standalone Rust binary. No required Python, Node.js, or JVM runtime.
- Observation, inference, and recommendation stay distinct. A missing library is an observation. "This is why the import failed" is an inference and needs evidence. "Change LD_LIBRARY_PATH" is a recommendation.
- Linux is first. The architecture must not make Windows, macOS, or WSL a rewrite.

### What StackLens is not

- Not an LLM wrapper over logs.
- Not an installer or auto-repair tool.
- Not a replacement for `nvidia-smi`, `ldd`, or vendor documentation.
- Not a telemetry agent.

## 2. Long-term architecture

Conceptual layout. This is the destination, not the current tree.

```text
stacklens/
│
├── crates/
│   ├── stacklens-cli/
│   ├── stacklens-core/
│   ├── stacklens-system/
│   ├── stacklens-gpu/
│   ├── stacklens-nvidia/
│   ├── stacklens-amd/
│   ├── stacklens-cuda/
│   ├── stacklens-rocm/
│   ├── stacklens-pytorch/
│   ├── stacklens-nccl/
│   ├── stacklens-container/
│   ├── stacklens-elf/
│   ├── stacklens-pe/
│   ├── stacklens-macho/
│   ├── stacklens-symbols/
│   ├── stacklens-rules/
│   ├── stacklens-redact/
│   └── stacklens-report/
│
├── rules/
│   ├── cuda/
│   ├── pytorch/
│   ├── nccl/
│   ├── cudnn/
│   ├── tensorrt/
│   └── rocm/
│
├── fixtures/
├── docs/
└── Cargo.toml
```

### Current crate policy

Do not create a crate until it has a real boundary and enough code to justify a package. Empty collector crates lock names and add workspace noise without helping.

Phase 1A creates three crates:

| Crate | Role |
| --- | --- |
| `stacklens-core` | Shared types, snapshot schema, probe states, findings, snapshot diff |
| `stacklens-system` | Host observations: OS, kernel, container/WSL hints, allowlisted environment |
| `stacklens-cli` | `stacklens` binary, clap commands, text/JSON rendering |

Crates that appear in the long-term diagram are introduced when their collector or renderer is implemented:

| Future crate | Created when |
| --- | --- |
| `stacklens-nvidia` / `stacklens-gpu` | Phase 1B GPU and driver inspection |
| `stacklens-cuda` | Phase 1C CUDA toolkit/runtime inspection |
| `stacklens-elf` | Phase 1D ELF parsing and loader modeling |
| `stacklens-pytorch` | Phase 1E filesystem/package inspection |
| `stacklens-rules` | Phase 1F deterministic rule evaluation |
| `stacklens-report` | When output outgrows CLI rendering, or `bundle` lands |
| `stacklens-symbols` | When symbol work no longer fits in `stacklens-elf` |
| `stacklens-redact` | When report bundling needs shared redaction |
| AMD / NCCL / container / PE / Mach-O crates | Phases 2–4 |

### Data flow

```text
Collectors
    ↓
Observations
    ↓
Normalized system model
    ↓
Diagnostic rules
    ↓
Findings
    ↓
Renderer
```

Collectors gather facts. They should not contain most diagnostic business logic.

Rules evaluate a normalized model. They produce findings with evidence.

Renderers format findings for the terminal, JSON, and later Markdown/zip bundles.

### Platform boundary

Platform-specific file and command access stays in collector modules. `stacklens-core` types are platform-neutral. A Windows PE collector later fills the same `LibraryInfo` shape that the Linux ELF collector fills.

## 3. Phase 1

Phase 1 target: **NVIDIA + CUDA + PyTorch + Linux**.

Primary commands:

```bash
stacklens doctor
stacklens gpu
stacklens cuda
stacklens libs
stacklens torch
stacklens snapshot
stacklens diff
```

### `stacklens doctor`

Aggregate diagnosis. Collects from Phase 1 subsystems and presents important findings in one report.

Phase 1A reports host observations and states clearly that GPU, CUDA, library, PyTorch, and rule evaluation are not implemented.

### `stacklens gpu`

NVIDIA GPU hardware and driver information. Future evidence includes detected GPUs, model, driver version, PCI/device identity, compute capability when it can be obtained deterministically, driver state, and multi-GPU layout.

### `stacklens cuda`

CUDA configuration. Future evidence includes driver CUDA capability, toolkit installs, `nvcc`, runtime libraries, install paths, environment, library candidates, and version inconsistencies that can be established from those facts.

### `stacklens libs`

Native shared-library dependencies. Phase 1 is Linux ELF.

```bash
stacklens libs /path/to/libtorch_cuda.so
```

Future evidence includes ELF architecture, `DT_NEEDED`, `RPATH`, `RUNPATH`, SONAME, practical symbol information, search locations, duplicate candidates, likely loader resolution, and GLIBC / GLIBCXX / CXXABI requirements.

A dedicated `inspect-lib` command is not added unless `libs` becomes overloaded. Do not create a synonym now.

### `stacklens torch`

PyTorch and LibTorch inspection without making Python a StackLens runtime dependency.

Future sources: filesystem layout, package metadata, known library locations, wheel metadata, LibTorch directory layout, linked CUDA libraries, and build information that can be determined reliably.

Do not execute arbitrary user Python. If an optional Python probe is added later, it stays isolated and never becomes required.

### `stacklens snapshot`

Structured, JSON-serializable representation of the environment.

```bash
stacklens snapshot > machine.json
```

The schema includes `schema_version` from the start. Fields that cannot be collected yet are recorded as probe states (`not_implemented`, `not_detected`, and so on). Snapshots must not invent GPU, CUDA, or PyTorch facts.

### `stacklens diff`

Compare two snapshots.

```bash
stacklens snapshot > good.json
stacklens snapshot > bad.json
stacklens diff good.json bad.json
```

Phase 1A establishes a deterministic structural diff. Later phases can add identity-aware comparison (match GPUs by UUID, libraries by path/SONAME) on top of that model.

## 4. Phase 1 detection scope

Phase 1 eventually needs to collect or reason about:

```text
GPU
NVIDIA driver
CUDA driver capability
CUDA toolkit
nvcc
environment variables
PyTorch build information
PyTorch CUDA information
shared libraries
ELF
RPATH
RUNPATH
LD_LIBRARY_PATH
duplicate CUDA libraries
dynamic library resolution
glibc
libstdc++
system architecture
WSL detection
Docker detection
```

Not all of this is implemented in the first coding pass.

### Phase 1A: Foundation (this pass)

- Cargo workspace
- CLI surface for all Phase 1 commands
- Core data models
- Host system information
- Allowlisted environment collection
- Structured probe / diagnostic types
- Snapshot schema and JSON serialization
- Deterministic snapshot diff
- Tests that do not require NVIDIA hardware
- CI for fmt, clippy, and tests

### Phase 1B: NVIDIA inspection

- GPU discovery
- Driver inspection
- Deterministic NVIDIA metadata gathering
- Fixture-driven parsers for vendor command output

### Phase 1C: CUDA inspection

- Toolkit installs
- `nvcc`
- Runtime libraries
- Environment relationships
- Version comparisons that the collected facts can support

### Phase 1D: ELF / library inspection

- ELF parser
- Dynamic dependencies
- `RPATH` / `RUNPATH`
- Candidate search
- Loader-resolution model
- GLIBC / GLIBCXX / CXXABI requirements where they can be read from the binary

### Phase 1E: PyTorch / LibTorch inspection

- Package and layout discovery
- Native libraries
- CUDA dependencies
- Library collision detection against system/toolkit candidates

### Phase 1F: Diagnostic rules

Rules consume the normalized model and emit findings. Example IDs (subject to the naming scheme in section 10):

```text
PYTORCH-CUDA-0001   CUDA runtime shadowing
CUDA-DRIVER-0001    Driver insufficient for detected runtime
ELF-LIB-0001        Required library cannot be resolved
ELF-LIB-0002        Multiple conflicting library candidates
PYTORCH-ABI-0001    Potential GLIBCXX/CXXABI incompatibility
```

## 5. Phase 2

Document only. Do not implement in Phase 1.

```text
NCCL
cuDNN
TensorRT
Docker
NVIDIA Container Toolkit
WSL2 (deeper than Phase 1 host hints)
```

Future command:

```bash
stacklens explain error.txt
```

Example input:

```text
ImportError:
libtorch_cuda.so:
undefined symbol: ncclCommRegister
```

Desired reasoning is evidence-based:

```text
Probable cause:
NCCL ABI/version mismatch.

Resolved library:
  /usr/lib/x86_64-linux-gnu/libnccl.so.2

PyTorch-bundled candidate:
  .../site-packages/nvidia/nccl/lib/libnccl.so.2

Evidence:
  LD_LIBRARY_PATH causes the system NCCL library to be considered first.
```

`explain` uses the same collectors and rules as `doctor`. It is not an LLM call.

## 6. Phase 3

Document only. Do not implement in Phase 1.

Windows is a real target:

```text
Windows
PE/COFF
DLL analysis
PyTorch Windows
CUDA Windows
WSL comparisons
```

Binary format map:

```text
Linux   -> ELF
Windows -> PE/COFF
macOS   -> Mach-O
```

The snapshot and library models must remain usable on Windows. Collectors change; the schema should not have to.

## 7. Phase 4

Document only. Do not implement in Phase 1.

AMD stack:

```text
AMD GPUs
ROCm
HIP
rocminfo
ROCm library resolution
gfx architecture
PyTorch ROCm
```

Prefer sharing library-resolution and snapshot infrastructure with the NVIDIA path. Do not fork the product into two toolkits.

## 8. Phase 5

Document only. Do not implement in Phase 1.

Framework and runtime integrations:

```text
JAX
TensorFlow
ONNX Runtime
vLLM
TensorRT-LLM
llama.cpp
Candle
```

Long-term objective: StackLens becomes the diagnostic layer for AI compute environments. Framework crates should reuse GPU, library, and container collectors rather than reimplementing them.

## 9. Future report bundling

```bash
stacklens bundle pytorch
```

Possible output: `stacklens-report.zip`.

Potential contents:

```text
report.md
environment.json
gpu.json
libraries.json
python-env.json
cuda.json
nccl.json
redacted-env.json
```

Not part of Phase 1A.

The report path must strip secrets. Never expose:

- API keys
- access tokens
- cloud credentials
- SSH private keys
- authentication cookies
- secrets stored in environment variables

Safe diagnostic variables (`CUDA_HOME`, `LD_LIBRARY_PATH`, `CUDA_VISIBLE_DEVICES`, and similar) are collected by allowlist. Everything else is omitted unless a later design introduces an explicit, redacted dump.

Paths that embed a username or home directory should be normalizable during redaction so a report can be shared without leaking account names.

## 10. Rule system

Deterministic rules. Conceptual shape, not a locked API:

```rust
DiagnosticRule {
    id: "PYTORCH-CUDA-0017",
    title: "System NCCL shadows PyTorch NCCL",
    severity: Severity::High,
}
```

A rule produces:

- stable rule ID
- severity
- title
- finding
- evidence
- explanation
- affected components
- optional recommendation

Evidence and conclusion stay separate:

```text
Evidence:
  libtorch_cuda.so requires libcudart.so.12
  loader candidate is /usr/local/cuda/lib64/libcudart.so.12
  another candidate exists in the Python environment

Finding:
  More than one CUDA runtime candidate exists.

Recommendation:
  Verify that the intended runtime has precedence.
```

Do not state certainty the evidence does not support. "May" and "cannot be determined" are valid outcomes.

### Rule ID scheme

```text
<AREA>-<TOPIC>-<NNNN>
```

- `AREA`: `CUDA`, `NVIDIA`, `ELF`, `PYTORCH`, `SYS`, later `NCCL`, `CUDNN`, `TRT`, `ROCM`
- `TOPIC`: short component or failure class (`DRIVER`, `LIB`, `ABI`, `CUDA`)
- `NNNN`: zero-padded integer, stable once published

IDs are never reused for a different meaning. Retired rules stay documented.

Rule definitions will eventually live under `rules/` so they can be reviewed as data, with Rust evaluation code in `stacklens-rules`.

## 11. Architecture principles

- Prefer fewer crates with real code over many empty packages.
- Prefer modules over traits when there is one implementation.
- Collectors return `Probe<T>`. Missing NVIDIA tooling is a probe state, not a panic.
- Snapshot JSON is versioned (`schema_version`). Unknown future fields must not make old snapshots unloadable without a migration path.
- Diff is structural and deterministic in Phase 1A. Smarter matching comes later.
- Tests use fixtures. Default CI must pass on a machine with no GPU.
- External commands (`nvidia-smi`, `nvcc`) are optional probes, not runtime dependencies of the binary.
- No custom argument parser. Use clap.
- No `unwrap()` on ordinary diagnostic paths.

## 12. Error and probe model

Distinguish application failure from diagnostic state.

| State | Meaning |
| --- | --- |
| Not implemented | This build does not collect the field yet |
| Not detected | Collector ran; the component was not found |
| Not installed | A specific expected install is absent |
| Permission denied | The collector could not read a path or device |
| Unsupported | Platform or format is out of scope for this collector |
| Probe failed | The collector ran and failed unexpectedly |
| Malformed | Input existed but could not be parsed |
| Internal error | StackLens bug; this is an application error |

`nvidia-smi` missing, CUDA absent, PyTorch absent, an unset environment variable, or no NVIDIA GPU are diagnostic states.

## 13. Testing and quality gates

Required locally and in CI:

```bash
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Fixture-driven tests are the default for:

- vendor command output
- `nvcc` output
- ELF metadata
- environment allowlists
- snapshots
- snapshot diff
- rule evaluation

Do not require NVIDIA hardware for the default test suite.

## 14. Open questions before Phase 1B

These should be decided before NVIDIA collection lands:

1. **Vendor CLI vs. sysfs vs. NVML.** Prefer reading `/proc` and `/sys` when the evidence is there. Use `nvidia-smi` when it is the only reliable source. Bind to NVML only if process/library inspection justifies the coupling.
2. **Where GPU types grow.** Keep `GpuInfo` / `NvidiaDriverInfo` in `stacklens-core`. Put parsers and probes in a new `stacklens-nvidia` crate once they exist.
3. **Command output stability.** Treat `nvidia-smi` text as an unstable interface. Parser tests must pin fixtures; prefer query flags that return compact, documented fields.
4. **Privilege.** Some device nodes and proc files are unreadable without extra rights. That is `permission_denied`, not a crash.
