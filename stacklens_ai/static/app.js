"use strict";
const el = (id) => document.getElementById(id);
const sampleButtons = [...document.querySelectorAll("[data-example]")];
const severityOrder = { high: "HIGH", medium: "REVIEW", low: "INFO" };

function node(tag, cls, value) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (value !== undefined) e.textContent = value;
  return e;
}
function appendText(parent, tag, cls, value) {
  const child = node(tag, cls, value);
  parent.appendChild(child);
  return child;
}

async function loadExample(name) {
  try {
    const response = await fetch("/api/examples/" + encodeURIComponent(name));
    if (!response.ok) throw new Error("Example not found");
    el("snapshot").value = JSON.stringify(await response.json(), null, 2);
    sampleButtons.forEach((button) => button.classList.toggle("active", button.dataset.example === name));
  } catch (error) {
    setError("Unable to load the sample snapshot: " + error.message);
  }
}

function setError(message) {
  el("empty").hidden = true;
  el("report").hidden = false;
  el("report-state").textContent = "ERROR";
  el("summary").textContent = message;
  el("findings").replaceChildren();
  el("model-state").textContent = "Unavailable";
  el("model-results").replaceChildren();
  el("limitations").textContent = "";
}

function render(data) {
  el("empty").hidden = true;
  el("report").hidden = false;
  el("report-state").textContent = "COMPLETE";
  const n = data.finding_count;
  el("summary").textContent = n
    ? n + " evidence-based finding" + (n === 1 ? "" : "s") + " detected. Review each finding before acting."
    : "No supported diagnostic rules fired. This is not a clean bill of health.";
  const root = el("findings");
  root.replaceChildren();
  (data.findings || []).forEach((finding) => {
    const card = node("article", "result");
    const heading = node("div", "result-heading");
    appendText(heading, "h3", "", finding.title);
    appendText(heading, "span", "severity " + finding.severity, severityOrder[finding.severity] || finding.severity);
    card.appendChild(heading);
    appendText(card, "div", "ruleid", finding.rule_id);
    appendText(card, "p", "", finding.interpretation);
    const list = node("ul");
    (finding.evidence || []).forEach((fact) => appendText(list, "li", "", fact));
    card.appendChild(list);
    appendText(card, "p", "check", "VERIFY: " + finding.verification);
    root.appendChild(card);
  });
  el("model-state").textContent = {
    completed: "Live response",
    not_configured: "Key not configured",
    not_requested: "Disabled",
    no_supported_findings: "No supported findings",
    rate_limited: "Demo request limit",
    provider_error: "Provider unavailable"
  }[data.ai_status] || data.ai_status;
  const modelBox = el("model-results");
  modelBox.replaceChildren();
  if (data.ai_status === "completed") {
    appendText(modelBox, "p", "", "Model: " + data.ai_model + " on Nebius Token Factory. Explanations are AI-generated and have not been independently proven.");
    (data.ai_explanations || []).forEach((row) => {
      const item = node("div", "model-item");
      appendText(item, "strong", "", row.rule_id);
      appendText(item, "p", "", row.explanation);
      modelBox.appendChild(item);
    });
  } else if (data.ai_status === "not_configured") {
    appendText(modelBox, "p", "", "The server has no NEBIUS_API_KEY. Deterministic findings remain available, but no model call was made.");
  } else if (data.ai_status === "provider_error") {
    appendText(modelBox, "p", "", data.ai_error || "Live model call failed.");
  } else if (data.ai_status === "rate_limited") {
    appendText(modelBox, "p", "", "The public demo's inference budget or per-minute limit has been reached. Try later.");
  } else {
    appendText(modelBox, "p", "", "No model response requested. The deterministic diagnosis is shown above.");
  }
  el("limitations").textContent = data.note;
}

async function analyze() {
  const button = el("analyze");
  try {
    let snapshot;
    try {
      snapshot = JSON.parse(el("snapshot").value);
    } catch {
      throw new Error("Invalid JSON. Load a sample or paste a StackLens snapshot.");
    }
    button.disabled = true;
    button.textContent = "Analyzing…";
    el("report-state").textContent = "RUNNING";
    const response = await fetch("/api/analyze", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ snapshot, use_ai: el("use-ai").checked })
    });
    const data = await response.json();
    if (!response.ok) throw new Error(data.error || "Analysis failed.");
    render(data);
  } catch (error) {
    setError(error.message || String(error));
  } finally {
    button.disabled = false;
    button.replaceChildren(document.createTextNode("Analyze snapshot ↗"));
  }
}

sampleButtons.forEach((button) => button.addEventListener("click", () => loadExample(button.dataset.example)));
el("analyze").addEventListener("click", analyze);
el("file").addEventListener("change", async (event) => {
  const file = event.target.files[0];
  if (!file) return;
  if (file.size > 256 * 1024) return setError("This demo accepts JSON snapshots up to 256 KiB.");
  el("snapshot").value = await file.text();
  sampleButtons.forEach((button) => button.classList.remove("active"));
});
loadExample("driver");
fetch("/api/health").then((r) => r.json()).then((data) => {
  el("live-status").textContent = data.ai_configured ? "NEMOTRON AVAILABLE" : "MODEL KEY REQUIRED";
  el("live-status").classList.toggle("off", !data.ai_configured);
}).catch(() => {
  el("live-status").textContent = "SERVICE UNAVAILABLE";
  el("live-status").classList.add("off");
});
