"use strict";

const el = (id) => document.getElementById(id);
const sampleButtons = [...document.querySelectorAll("[data-example]")];
const severityLabels = { high: "HIGH SEVERITY", medium: "REVIEW REQUIRED", low: "INFORMATIONAL" };

function createNode(tag, className, textContent) {
  const elem = document.createElement(tag);
  if (className) elem.className = className;
  if (textContent !== undefined) elem.textContent = textContent;
  return elem;
}

function copyToClipboard(text, btnElem) {
  navigator.clipboard.writeText(text).then(() => {
    const originalText = btnElem.textContent;
    btnElem.textContent = "Copied!";
    btnElem.style.borderColor = "var(--lime)";
    setTimeout(() => {
      btnElem.textContent = originalText;
      btnElem.style.borderColor = "";
    }, 1800);
  }).catch(() => {
    // Fallback if clipboard API is blocked
    const input = document.createElement("input");
    input.value = text;
    document.body.appendChild(input);
    input.select();
    document.execCommand("copy");
    document.body.removeChild(input);
    btnElem.textContent = "Copied!";
    setTimeout(() => { btnElem.textContent = "Copy Command"; }, 1800);
  });
}

async function loadExample(name) {
  try {
    const response = await fetch("/api/examples/" + encodeURIComponent(name));
    if (!response.ok) throw new Error("Scenario fixture not found");
    const json = await response.json();
    el("snapshot").value = JSON.stringify(json, null, 2);
    sampleButtons.forEach((btn) => btn.classList.toggle("active", btn.dataset.example === name));
  } catch (error) {
    showError("Failed to load scenario: " + error.message);
  }
}

function showError(message) {
  el("empty").hidden = true;
  el("report").hidden = false;
  el("report-state").textContent = "ERROR";
  el("report-state").className = "badge-status error";
  el("summary").textContent = "Diagnostic Error: " + message;
  el("findings").replaceChildren();
  el("model-state").textContent = "Unavailable";
  el("model-results").replaceChildren();
  el("limitations").textContent = "";
}

function renderReport(data) {
  el("empty").hidden = true;
  el("report").hidden = false;
  el("report-state").textContent = "COMPLETE";
  el("report-state").className = "badge-status complete";

  const count = data.finding_count || 0;
  const summaryBox = el("summary");
  if (count > 0) {
    summaryBox.textContent = `Identified ${count} deterministic compatibility finding${count === 1 ? "" : "s"}. Review each finding and verification step below.`;
  } else {
    summaryBox.textContent = "No deterministic compatibility rules triggered. Note: Phase 1C does not evaluate un-implemented PyTorch or ELF loader probes; this is not proof of full compatibility.";
  }

  // Render Findings
  const findingsContainer = el("findings");
  findingsContainer.replaceChildren();

  (data.findings || []).forEach((finding) => {
    const card = createNode("article", "finding-card");

    const header = createNode("div", "finding-header");
    const title = createNode("h3", "finding-title", finding.title);
    const severityPill = createNode(
      "span",
      "severity-pill " + (finding.severity || "low"),
      severityLabels[finding.severity] || finding.severity
    );
    header.appendChild(title);
    header.appendChild(severityPill);
    card.appendChild(header);

    const ruleId = createNode("div", "finding-ruleid", `RULE ID: ${finding.rule_id}`);
    card.appendChild(ruleId);

    const interpretation = createNode("p", "finding-interpretation", finding.interpretation);
    card.appendChild(interpretation);

    if (finding.evidence && finding.evidence.length > 0) {
      const list = createNode("ul", "evidence-list");
      finding.evidence.forEach((item) => {
        list.appendChild(createNode("li", "", item));
      });
      card.appendChild(list);
    }

    if (finding.verification) {
      const verifyBox = createNode("div", "verification-box");
      verifyBox.appendChild(createNode("span", "verification-label", "DETERMINISTIC VERIFICATION:"));
      verifyBox.appendChild(document.createTextNode(finding.verification));
      card.appendChild(verifyBox);
    }

    findingsContainer.appendChild(card);
  });

  // Render Model Explanations
  const modelBadge = el("model-state");
  const modelResults = el("model-results");
  modelResults.replaceChildren();

  const statusMap = {
    completed: "Live Inference",
    not_configured: "Key Pending / Deterministic Only",
    not_requested: "Opted Out",
    no_supported_findings: "No Findings to Explain",
    rate_limited: "Budget / Rate Limit Reached",
    provider_error: "Provider Service Error",
  };
  modelBadge.textContent = statusMap[data.ai_status] || data.ai_status;

  if (data.ai_status === "completed") {
    const headerP = createNode("p", "", `Model: ${data.ai_model} via Nebius Token Factory. Explanations are AI-generated and strictly grounded in the verified finding IDs below.`);
    modelResults.appendChild(headerP);

    (data.ai_explanations || []).forEach((item) => {
      const aiItem = createNode("div", "ai-item");

      const head = createNode("div", "ai-item-head");
      head.appendChild(createNode("span", "ai-item-ruleid", `Finding: ${item.rule_id}`));
      aiItem.appendChild(head);

      aiItem.appendChild(createNode("p", "", item.explanation));

      if (item.suggested_check) {
        const cmdBox = createNode("div", "suggested-cmd-box");
        const info = createNode("div", "suggested-cmd-info");
        info.appendChild(createNode("span", "suggested-cmd-label", "PROPOSED VERIFICATION (Nemotron selected from allowlist):"));
        const codeElem = createNode("code", "", item.suggested_check);
        info.appendChild(codeElem);
        cmdBox.appendChild(info);

        const copyBtn = createNode("button", "copy-btn", "Copy Command");
        copyBtn.type = "button";
        copyBtn.addEventListener("click", () => copyToClipboard(item.suggested_check, copyBtn));
        cmdBox.appendChild(copyBtn);

        aiItem.appendChild(cmdBox);
      }

      modelResults.appendChild(aiItem);
    });
  } else if (data.ai_status === "not_configured") {
    modelResults.appendChild(createNode("p", "", "Nebius Token Factory API key is currently pending account approval. The server is operating in 100% offline deterministic mode. When NEBIUS_API_KEY is configured, NVIDIA Nemotron 120B explanations will appear here."));
    modelResults.appendChild(createNode("p", "", "To test real inference after your key arrives, configure NEBIUS_API_KEY in .env and run: python -m stacklens_ai.preflight --smoke-test"));
  } else if (data.ai_status === "rate_limited") {
    modelResults.appendChild(createNode("p", "", data.ai_error || "Inference rate limit or process call budget reached. Please try again shortly."));
  } else if (data.ai_status === "provider_error") {
    modelResults.appendChild(createNode("p", "", `Nebius Token Factory Error: ${data.ai_error || "Service unavailable."}`));
  } else {
    modelResults.appendChild(createNode("p", "", "Model explanation was not requested. Deterministic findings are shown above."));
  }

  el("limitations").textContent = data.note || "";
}

async function handleAnalyze() {
  const button = el("analyze");
  try {
    let parsedSnapshot;
    try {
      parsedSnapshot = JSON.parse(el("snapshot").value);
    } catch {
      throw new Error("Invalid JSON format. Please load a sample scenario or paste valid JSON.");
    }

    button.disabled = true;
    button.innerHTML = "<span>Analyzing...</span>";
    el("report-state").textContent = "RUNNING";
    el("report-state").className = "badge-status running";

    const response = await fetch("/api/analyze", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        snapshot: parsedSnapshot,
        use_ai: el("use-ai").checked,
      }),
    });

    const data = await response.json();
    if (!response.ok) {
      throw new Error(data.error || `HTTP ${response.status} from analysis endpoint`);
    }

    renderReport(data);
  } catch (err) {
    showError(err.message || String(err));
  } finally {
    button.disabled = false;
    button.innerHTML = '<span>Analyze Snapshot</span><span class="btn-arrow">→</span>';
  }
}

// Event Listeners
sampleButtons.forEach((btn) => {
  btn.addEventListener("click", () => loadExample(btn.dataset.example));
});

el("analyze").addEventListener("click", handleAnalyze);

el("file").addEventListener("change", async (event) => {
  const file = event.target.files[0];
  if (!file) return;
  if (file.size > 256 * 1024) {
    return showError("Snapshot file exceeds maximum allowed demo size of 256 KiB.");
  }
  try {
    el("snapshot").value = await file.text();
    sampleButtons.forEach((btn) => btn.classList.remove("active"));
  } catch (err) {
    showError("Could not read file: " + err.message);
  }
});

// Initial Setup
loadExample("driver");

fetch("/api/health")
  .then((res) => res.json())
  .then((data) => {
    const statusElem = el("live-status");
    const textElem = el("status-text");
    if (data.ai_configured) {
      textElem.textContent = "NEMOTRON ACTIVE";
      statusElem.className = "status";
      statusElem.title = `Model: ${data.nebius_model} on Nebius Token Factory`;
    } else {
      textElem.textContent = "OFFLINE (KEY PENDING)";
      statusElem.className = "status off";
      statusElem.title = "Nebius Token Factory key is pending approval. Deterministic diagnostics fully operational.";
    }
  })
  .catch(() => {
    const statusElem = el("live-status");
    const textElem = el("status-text");
    textElem.textContent = "BACKEND DISCONNECTED";
    statusElem.className = "status error";
  });
