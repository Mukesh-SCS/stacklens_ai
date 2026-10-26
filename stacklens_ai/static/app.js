/**
 * StackLens AI - Frontend Controller
 * Strictly uses safe DOM manipulation (createElement / textContent).
 */

"use strict";

(function () {
  // DOM Elements
  const el = (id) => document.getElementById(id);

  const scenarioButtons = [...document.querySelectorAll("[data-example]")];
  const dropzone = el("dropzone");
  const fileInput = el("file-input");
  const snapshotInput = el("snapshot-input");
  const btnToggleEditor = el("btn-toggle-editor");
  const editorContainer = document.querySelector(".editor-container");
  const jsonValidityBadge = el("json-validity");
  const btnFormatJson = el("btn-format-json");
  const inputErrorBanner = el("input-error");
  const inputErrorText = el("input-error-text");
  const toggleUseAi = el("toggle-use-ai");
  const btnAnalyze = el("btn-analyze");
  const btnAnalyzeSpinner = el("btn-analyze-spinner");
  const btnAnalyzeText = el("btn-analyze-text");
  const btnAnalyzeIcon = el("btn-analyze-icon");

  const connectionStatus = el("connection-status");
  const statusLabel = el("status-label");

  const stateEmpty = el("state-empty");
  const stateReport = el("state-report");
  const badgeReportStatus = el("badge-report-status");
  const badgeFindingsCount = el("badge-findings-count");
  const badgeHighestSeverity = el("badge-highest-severity");

  const summaryTitle = el("summary-title");
  const summaryBody = el("summary-body");
  const findingsList = el("findings-list");
  const nemotronStatusPill = el("nemotron-status-pill");
  const nemotronBody = el("nemotron-body");
  const limitationsText = el("limitations-text");

  const modalAbout = el("modal-about");
  const btnHowItWorks = el("btn-how-it-works");
  const btnCloseModal = el("btn-close-modal");
  const btnModalDismiss = el("btn-modal-dismiss");

  let backendMeta = null;
  let hasCompletedLiveInference = false;

  // Safe DOM Helper
  function createTag(tagName, className, textContent) {
    const node = document.createElement(tagName);
    if (className) node.className = className;
    if (textContent !== undefined && textContent !== null) {
      node.textContent = textContent;
    }
    return node;
  }

  // Clipboard Helper with SVG feedback
  function copyTextToClipboard(text, btnElement) {
    const originalText = btnElement.textContent;
    const performCopy = () => {
      btnElement.textContent = "Copied!";
      btnElement.classList.add("copied");
      setTimeout(() => {
        btnElement.textContent = originalText;
        btnElement.classList.remove("copied");
      }, 1800);
    };

    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).then(performCopy).catch(() => {
        fallbackCopy(text, performCopy);
      });
    } else {
      fallbackCopy(text, performCopy);
    }
  }

  function fallbackCopy(text, callback) {
    const textArea = document.createElement("textarea");
    textArea.value = text;
    textArea.style.position = "fixed";
    textArea.style.opacity = "0";
    document.body.appendChild(textArea);
    textArea.focus();
    textArea.select();
    try {
      document.execCommand("copy");
      callback();
    } catch (e) {
      console.warn("Clipboard copy failed:", e);
    }
    document.body.removeChild(textArea);
  }

  // Allowlist & Verification Guardrail Helpers
  const VALIDATED_SHELL_COMMANDS = new Set([
    "nvidia-smi",
    "cat /proc/driver/nvidia/version",
    "modinfo nvidia",
    "nvcc --version",
    'echo "CUDA_HOME=$CUDA_HOME CUDA_PATH=$CUDA_PATH CUDA_ROOT=$CUDA_ROOT"',
    "which nvcc || command -v nvcc",
    "env | grep -E '^(CUDA_|PATH|LD_LIBRARY_PATH)'",
    "ldconfig -p | grep libcudart",
    'echo "LD_LIBRARY_PATH=$LD_LIBRARY_PATH"',
    "which -a libcudart.so 2>/dev/null || true",
    "dmesg | grep -i nvidia | tail -n 20",
    "dpkg -l | grep -i nvidia || rpm -qa | grep -i nvidia",
    "python -m stacklens_ai.preflight",
    "python -m stacklens_ai.preflight --smoke-test",
    "stacklens snapshot > host.json",
    "stacklens diff left.json right.json",
  ]);

  function isShellCommand(value) {
    if (!value || typeof value !== "string") return false;
    const trimmed = value.trim();
    if (VALIDATED_SHELL_COMMANDS.has(trimmed)) return true;
    if (/^(Compare|Inspect|Review|Verify|Check|Ensure|Update|Recheck)\b/i.test(trimmed)) {
      return false;
    }
    if (trimmed.endsWith(".") && trimmed.includes(" ")) {
      return false;
    }
    const firstWord = trimmed.split(/\s+/)[0];
    const knownExecs = [
      "nvidia-smi",
      "cat",
      "modinfo",
      "nvcc",
      "echo",
      "which",
      "env",
      "ldconfig",
      "dmesg",
      "dpkg",
      "rpm",
      "stacklens",
      "python",
    ];
    return knownExecs.includes(firstWord);
  }

  function createVerificationCard(text, customLabel) {
    const isCmd = isShellCommand(text);
    const card = createTag("div", `verify-command-card ${isCmd ? "is-command" : "is-guidance"}`);
    const content = createTag("div", "verify-command-content");

    let labelText = customLabel;
    if (!labelText) {
      labelText = isCmd ? "SAFE VERIFICATION COMMAND (READ-ONLY):" : "Recommended Verification";
    }
    content.appendChild(createTag("span", "verify-label", labelText));

    if (isCmd) {
      content.appendChild(createTag("code", "verify-code", text));
    } else {
      content.appendChild(createTag("p", "verify-guidance", text));
    }
    card.appendChild(content);

    const btnLabel = isCmd ? "Copy Command" : "Copy Guidance";
    const copyBtn = createTag("button", "btn-copy", btnLabel);
    copyBtn.type = "button";
    copyBtn.addEventListener("click", () => copyTextToClipboard(text, copyBtn));
    card.appendChild(copyBtn);

    return card;
  }

  // Update Status Pill
  function updateConnectionStatus(state, message) {
    connectionStatus.className = "status-indicator " + state;
    statusLabel.textContent = message;
  }

  // Validate JSON in Editor
  function validateJsonInput() {
    const value = snapshotInput.value.trim();
    if (!value) {
      jsonValidityBadge.textContent = "EMPTY";
      jsonValidityBadge.className = "validity-badge invalid";
      return false;
    }
    try {
      const parsed = JSON.parse(value);
      if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
        jsonValidityBadge.textContent = "NOT AN OBJECT";
        jsonValidityBadge.className = "validity-badge invalid";
        return false;
      }
      if (parsed.schema_version !== 1) {
        jsonValidityBadge.textContent = "SCHEMA != 1";
        jsonValidityBadge.className = "validity-badge invalid";
        return false;
      }
      jsonValidityBadge.textContent = "VALID SCHEMA V1";
      jsonValidityBadge.className = "validity-badge valid";
      hideInputError();
      return true;
    } catch (err) {
      jsonValidityBadge.textContent = "SYNTAX ERROR";
      jsonValidityBadge.className = "validity-badge invalid";
      return false;
    }
  }

  function showInputError(msg) {
    inputErrorText.textContent = msg;
    inputErrorBanner.hidden = false;
  }

  function hideInputError() {
    inputErrorBanner.hidden = true;
  }

  // Load Demonstration Scenario
  async function loadScenario(scenarioKey) {
    hideInputError();
    try {
      const resp = await fetch("api/examples/" + encodeURIComponent(scenarioKey));
      if (!resp.ok) {
        throw new Error(`Scenario not found (${resp.status})`);
      }
      const data = await resp.json();
      snapshotInput.value = JSON.stringify(data, null, 2);
      validateJsonInput();

      scenarioButtons.forEach((btn) => {
        btn.classList.toggle("active", btn.dataset.example === scenarioKey);
      });
    } catch (err) {
      showInputError("Failed to load scenario: " + err.message);
    }
  }

  // Render Findings and AI Reasoning
  function renderAnalysisResults(data) {
    stateEmpty.hidden = true;
    stateReport.hidden = false;

    badgeReportStatus.textContent = "INSPECTION COMPLETE";
    badgeReportStatus.className = "pill-badge pill-status complete";

    const findings = data.findings || [];
    const count = findings.length;

    badgeFindingsCount.hidden = false;
    badgeFindingsCount.textContent = `${count} FINDING${count === 1 ? "" : "S"}`;

    // Determine highest severity
    let highestSeverity = "NONE";
    if (findings.some((f) => f.severity === "high")) {
      highestSeverity = "HIGH";
    } else if (findings.some((f) => f.severity === "medium")) {
      highestSeverity = "REVIEW";
    } else if (findings.some((f) => f.severity === "low")) {
      highestSeverity = "INFO";
    }

    if (count > 0) {
      badgeHighestSeverity.hidden = false;
      badgeHighestSeverity.textContent = `SEVERITY: ${highestSeverity}`;
      badgeHighestSeverity.className = `pill-badge pill-severity severity-${highestSeverity.toLowerCase()}`;
      summaryTitle.textContent = `${count} Compatibility Finding${count === 1 ? "" : "s"} Detected`;
      summaryBody.textContent = "Review each deterministic finding, observed evidence, and safe verification command below.";
    } else {
      badgeHighestSeverity.hidden = true;
      summaryTitle.textContent = "No Supported Compatibility Rules Fired";
      summaryBody.textContent = "No supported deterministic rules triggered. Note: Phase 1C evaluates driver branch and CUDA environment selectors, but does not yet evaluate un-implemented PyTorch or ELF loader probes. This is not proof of full application health.";
    }

    // Render Findings List
    findingsList.replaceChildren();

    findings.forEach((finding) => {
      const card = createTag("article", `finding-item severity-${finding.severity}`);

      // Header
      const header = createTag("div", "finding-item-header");
      const titleGroup = createTag("div", "finding-item-title-group");
      titleGroup.appendChild(createTag("span", "finding-rule-id", finding.rule_id));
      titleGroup.appendChild(createTag("h4", "finding-title", finding.title));
      header.appendChild(titleGroup);

      const severityTag = createTag(
        "span",
        `scenario-badge badge-${finding.severity}`,
        finding.severity === "high" ? "HIGH SEVERITY" : finding.severity === "medium" ? "REVIEW REQUIRED" : "INFORMATIONAL"
      );
      header.appendChild(severityTag);
      card.appendChild(header);

      // Interpretation
      card.appendChild(createTag("p", "finding-interpretation", finding.interpretation));

      // Evidence Block
      if (finding.evidence && finding.evidence.length > 0) {
        const evBlock = createTag("div", "evidence-block");
        evBlock.appendChild(createTag("span", "evidence-header-label", "OBSERVED EVIDENCE (COLLECTED FACTS):"));
        const evList = createTag("ul", "evidence-list");
        finding.evidence.forEach((fact) => {
          evList.appendChild(createTag("li", "", fact));
        });
        evBlock.appendChild(evList);
        card.appendChild(evBlock);
      }

      // Verification Card (Guidance vs Validated Shell Command)
      if (finding.verification) {
        card.appendChild(createVerificationCard(finding.verification));
      }

      findingsList.appendChild(card);
    });

    // Render Nemotron AI Box
    renderNemotronSection(data);

    // Limitations Note
    limitationsText.textContent = data.note || "No finding firing does not prove application health. Unimplemented PyTorch and ELF collectors are not evaluated.";
  }

  function renderNemotronSection(data) {
    nemotronBody.replaceChildren();

    const status = data.ai_status;
    const modelName = data.ai_model || (backendMeta ? backendMeta.nebius_model : "nvidia/nemotron-3-super-120b-a12b");

    if (status === "completed") {
      hasCompletedLiveInference = true;
      nemotronStatusPill.textContent = "LIVE VERIFIED";
      nemotronStatusPill.className = "pill-nemotron verified";
      updateConnectionStatus("status-verified", "Live Inference Verified");

      const headCard = createTag("div", "nemotron-state-card");
      headCard.appendChild(createTag("strong", "", `Model: ${modelName} via Nebius Token Factory`));
      headCard.appendChild(createTag("p", "", "Explanations are generated by NVIDIA Nemotron and strictly constrained to the verified finding IDs below. Suggested commands are selected from an allowlist."));
      nemotronBody.appendChild(headCard);

      (data.ai_explanations || []).forEach((item) => {
        const expItem = createTag("div", "nemotron-explanation-item");

        const itemHead = createTag("div", "nemotron-item-head");
        itemHead.appendChild(createTag("span", "nemotron-item-ruleid", `Finding Grounding: ${item.rule_id}`));
        expItem.appendChild(itemHead);

        expItem.appendChild(createTag("p", "nemotron-item-prose", item.explanation));

        if (item.suggested_check) {
          const customLabel = isShellCommand(item.suggested_check)
            ? "PROPOSED VERIFICATION (SELECTED FROM ALLOWLIST):"
            : "Recommended Verification";
          expItem.appendChild(createVerificationCard(item.suggested_check, customLabel));
        }

        nemotronBody.appendChild(expItem);
      });
    } else if (status === "not_configured") {
      nemotronStatusPill.textContent = "KEY PENDING";
      nemotronStatusPill.className = "pill-nemotron offline";

      const card = createTag("div", "nemotron-state-card");
      card.appendChild(createTag("strong", "", "Deterministic analysis is active."));
      card.appendChild(createTag("p", "", "NVIDIA Nemotron explanations will be enabled when Nebius Token Factory is configured. The deterministic diagnostic findings above are 100% operational without cloud dependencies."));
      card.appendChild(createTag("p", "", "When your Nebius Token Factory API key arrives, configure NEBIUS_API_KEY in .env and run: python -m stacklens_ai.preflight --smoke-test"));
      nemotronBody.appendChild(card);
    } else if (status === "not_requested") {
      nemotronStatusPill.textContent = "AI DISABLED";
      nemotronStatusPill.className = "pill-nemotron disabled";

      const card = createTag("div", "nemotron-state-card");
      card.appendChild(createTag("p", "", "Nemotron explanation was opted out for this analysis. Deterministic findings are shown above."));
      nemotronBody.appendChild(card);
    } else if (status === "no_supported_findings") {
      nemotronStatusPill.textContent = "NO FINDINGS";
      nemotronStatusPill.className = "pill-nemotron disabled";

      const card = createTag("div", "nemotron-state-card");
      card.appendChild(createTag("p", "", "No deterministic compatibility findings triggered, so no model explanation was requested."));
      nemotronBody.appendChild(card);
    } else if (status === "rate_limited") {
      nemotronStatusPill.textContent = "LIMIT REACHED";
      nemotronStatusPill.className = "pill-nemotron error";

      const card = createTag("div", "nemotron-state-card");
      card.appendChild(createTag("strong", "", "Inference Rate Limit or Process Budget Reached"));
      card.appendChild(createTag("p", "", data.ai_error || "The demonstration call budget or per-minute rate limit has been reached. Please try again shortly."));
      nemotronBody.appendChild(card);
    } else if (status === "provider_error") {
      nemotronStatusPill.textContent = "PROVIDER ERROR";
      nemotronStatusPill.className = "pill-nemotron error";

      const card = createTag("div", "nemotron-state-card");
      card.appendChild(createTag("strong", "", "Nebius Token Factory Error"));
      card.appendChild(createTag("p", "", data.ai_error || "The remote inference service returned an error. Deterministic findings remain valid."));
      nemotronBody.appendChild(card);
    }
  }

  // Handle Analysis Execution
  async function executeAnalysis() {
    hideInputError();

    let snapshot;
    try {
      snapshot = JSON.parse(snapshotInput.value);
      if (typeof snapshot !== "object" || snapshot === null) {
        throw new Error("Snapshot must be a valid JSON object.");
      }
    } catch (err) {
      showInputError("Invalid JSON: " + err.message);
      return;
    }

    // Set Loading UI State
    btnAnalyze.disabled = true;
    btnAnalyzeSpinner.hidden = false;
    btnAnalyzeIcon.hidden = true;
    btnAnalyzeText.textContent = "Analyzing Environment...";

    badgeReportStatus.textContent = "RUNNING";
    badgeReportStatus.className = "pill-badge pill-status running";

    try {
      const resp = await fetch("api/analyze", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          snapshot: snapshot,
          use_ai: toggleUseAi.checked,
        }),
      });

      const result = await resp.json();
      if (!resp.ok) {
        throw new Error(result.error || `HTTP ${resp.status} from analysis server.`);
      }

      renderAnalysisResults(result);
    } catch (err) {
      showInputError(err.message);
      badgeReportStatus.textContent = "ERROR";
      badgeReportStatus.className = "pill-badge pill-status error";
    } finally {
      btnAnalyze.disabled = false;
      btnAnalyzeSpinner.hidden = true;
      btnAnalyzeIcon.hidden = false;
      btnAnalyzeText.textContent = "Analyze Environment";
    }
  }

  // Check Backend Health
  async function checkBackendHealth() {
    try {
      const resp = await fetch("api/health");
      if (!resp.ok) throw new Error("Health check returned status " + resp.status);
      backendMeta = await resp.json();

      if (hasCompletedLiveInference) {
        updateConnectionStatus("status-verified", "Live Inference Verified");
      } else if (backendMeta.ai_configured) {
        updateConnectionStatus("status-verified", "Key Configured (Unverified)");
      } else {
        updateConnectionStatus("status-offline", "Offline (Key Pending)");
      }
    } catch (err) {
      updateConnectionStatus("status-error", "Backend Unavailable");
    }
  }

  // Drag-and-Drop Handlers
  function setupDragAndDrop() {
    dropzone.addEventListener("click", () => fileInput.click());

    dropzone.addEventListener("keydown", (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        fileInput.click();
      }
    });

    ["dragenter", "dragover"].forEach((eventName) => {
      dropzone.addEventListener(eventName, (e) => {
        e.preventDefault();
        e.stopPropagation();
        dropzone.classList.add("dragover");
      });
    });

    ["dragleave", "drop"].forEach((eventName) => {
      dropzone.addEventListener(eventName, (e) => {
        e.preventDefault();
        e.stopPropagation();
        dropzone.classList.remove("dragover");
      });
    });

    dropzone.addEventListener("drop", (e) => {
      const files = e.dataTransfer.files;
      if (files && files.length > 0) {
        handleFileSelect(files[0]);
      }
    });

    fileInput.addEventListener("change", (e) => {
      const file = e.target.files[0];
      if (file) handleFileSelect(file);
    });
  }

  function handleFileSelect(file) {
    if (file.size > 256 * 1024) {
      showInputError("File exceeds maximum allowed demo snapshot size of 256 KiB.");
      return;
    }
    const reader = new FileReader();
    reader.onload = (e) => {
      snapshotInput.value = e.target.result;
      validateJsonInput();
      scenarioButtons.forEach((b) => b.classList.remove("active"));
    };
    reader.onerror = () => {
      showInputError("Error reading snapshot file.");
    };
    reader.readAsText(file);
  }

  // JSON Formatter
  function formatJsonEditor() {
    try {
      const parsed = JSON.parse(snapshotInput.value);
      snapshotInput.value = JSON.stringify(parsed, null, 2);
      validateJsonInput();
    } catch (e) {
      showInputError("Cannot format invalid JSON.");
    }
  }

  // Bind UI Events
  function init() {
    // Scenario Buttons
    scenarioButtons.forEach((btn) => {
      btn.addEventListener("click", () => loadScenario(btn.dataset.example));
    });

    // Editor Toggle
    btnToggleEditor.addEventListener("click", () => {
      const isCollapsed = editorContainer.classList.toggle("collapsed");
      btnToggleEditor.setAttribute("aria-expanded", String(!isCollapsed));
    });

    // Format JSON
    btnFormatJson.addEventListener("click", formatJsonEditor);

    // Live validation on textarea input
    snapshotInput.addEventListener("input", validateJsonInput);

    // Analyze Button
    btnAnalyze.addEventListener("click", executeAnalysis);

    // Setup Drag and Drop
    setupDragAndDrop();

    // Copy Code Buttons in Workflow Guide
    document.querySelectorAll(".btn-copy-code").forEach((btn) => {
      btn.addEventListener("click", () => {
        const cmd = btn.dataset.copy;
        if (cmd) copyTextToClipboard(cmd, btn);
      });
    });

    // Modal Handlers
    btnHowItWorks.addEventListener("click", () => {
      modalAbout.showModal();
    });
    btnCloseModal.addEventListener("click", () => modalAbout.close());
    btnModalDismiss.addEventListener("click", () => modalAbout.close());
    modalAbout.addEventListener("click", (e) => {
      if (e.target === modalAbout) modalAbout.close();
    });

    // Parse URL parameters for initial scenario and auto-analysis
    const urlParams = new URLSearchParams(window.location.search);
    const initialScenario = urlParams.get("scenario") || urlParams.get("example") || "driver";
    loadScenario(initialScenario);

    // Collapse JSON editor by default on small screens (<= 768px) to reduce vertical scrolling
    if (window.innerWidth <= 768 && editorContainer) {
      editorContainer.classList.add("collapsed");
      if (btnToggleEditor) btnToggleEditor.setAttribute("aria-expanded", "false");
    }

    // Initial Health Check
    checkBackendHealth();

    if (urlParams.get("auto") === "true") {
      setTimeout(executeAnalysis, 350);
    }
  }

  // Run on DOM Ready
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
