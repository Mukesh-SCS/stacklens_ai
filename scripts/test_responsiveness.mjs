import http from "node:http";
import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const CHROME_PATH = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
const PORT = 9222;
const TARGET_URL = "http://127.0.0.1:8000/";
const WIDTHS = [360, 375, 390, 430, 768, 1440];

async function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

async function getJson(url) {
  return new Promise((resolve, reject) => {
    http.get(url, (res) => {
      let data = "";
      res.on("data", (c) => (data += c));
      res.on("end", () => {
        try {
          resolve(JSON.parse(data));
        } catch (e) {
          reject(e);
        }
      });
    }).on("error", reject);
  });
}

class CDPClient {
  constructor(wsUrl) {
    this.ws = new WebSocket(wsUrl);
    this.id = 1;
    this.handlers = new Map();
  }

  async connect() {
    return new Promise((resolve, reject) => {
      this.ws.onopen = resolve;
      this.ws.onerror = reject;
      this.ws.onmessage = (event) => {
        const msg = JSON.parse(event.data);
        if (msg.id && this.handlers.has(msg.id)) {
          const { resolve, reject } = this.handlers.get(msg.id);
          this.handlers.delete(msg.id);
          if (msg.error) reject(msg.error);
          else resolve(msg.result);
        }
      };
    });
  }

  async send(method, params = {}) {
    const id = this.id++;
    return new Promise((resolve, reject) => {
      this.handlers.set(id, { resolve, reject });
      this.ws.send(JSON.stringify({ id, method, params }));
    });
  }

  close() {
    this.ws.close();
  }
}

async function run() {
  const tmpProfile = path.join(process.env.TEMP || "C:\\Temp", `chrome_resp_${Date.now()}`);
  const chromeProc = spawn(CHROME_PATH, [
    "--headless=new",
    `--remote-debugging-port=${PORT}`,
    `--user-data-dir=${tmpProfile}`,
    TARGET_URL,
  ]);

  let cdp;
  try {
    let targets = null;
    for (let i = 0; i < 30; i++) {
      await sleep(300);
      try {
        targets = await getJson(`http://127.0.0.1:${PORT}/json/list`);
        if (targets && targets.length > 0) break;
      } catch {}
    }

    if (!targets || targets.length === 0) {
      throw new Error("Could not connect to Chrome debugging target");
    }

    const pageTarget = targets.find((t) => t.type === "page") || targets[0];
    cdp = new CDPClient(pageTarget.webSocketDebuggerUrl);
    await cdp.connect();

    const consoleErrors = [];
    await cdp.send("Page.enable");
    await cdp.send("Runtime.enable");
    cdp.ws.addEventListener("message", (evt) => {
      const msg = JSON.parse(evt.data);
      if (msg.method === "Runtime.consoleAPICalled" && msg.params.type === "error") {
        consoleErrors.push(msg.params.args.map((a) => a.value || a.description).join(" "));
      } else if (msg.method === "Runtime.exceptionThrown") {
        consoleErrors.push(msg.params.exceptionDetails.text);
      }
    });

    // Wait for page to initialize
    await sleep(800);

    const checkOverflow = async (label) => {
      console.log(`\n--- Checking overflow for: ${label} ---`);
      for (const w of WIDTHS) {
        await cdp.send("Emulation.setDeviceMetricsOverride", {
          width: w,
          height: 844,
          deviceScaleFactor: 1,
          mobile: w <= 768,
        });
        await sleep(150);

        const evalRes = await cdp.send("Runtime.evaluate", {
          expression: `
            (() => {
              const clientWidth = document.documentElement.clientWidth;
              const scrollWidth = document.documentElement.scrollWidth;
              return {
                clientWidth,
                scrollWidth,
                overflow: scrollWidth > clientWidth
              };
            })()
          `,
          returnByValue: true,
        });

        const res = evalRes.result.value;
        const pass = !res.overflow;
        console.log(`  Viewport ${w}px: ${pass ? "PASS" : "FAIL"} (clientWidth: ${res.clientWidth}, scrollWidth: ${res.scrollWidth})`);
        if (!pass) {
          throw new Error(`Overflow detected at ${w}px for ${label}`);
        }
      }
    };

    // 1. Initial Empty State
    await checkOverflow("Initial Desktop & Mobile (Empty State)");

    // 2. Scenario 1 (Driver Outdated)
    console.log("\nTesting Scenario 1: Outdated Driver...");
    await cdp.send("Runtime.evaluate", {
      expression: `(async () => {
        document.getElementById('btn-example-driver').click();
        await new Promise(r => setTimeout(r, 400));
        document.getElementById('btn-analyze').click();
      })()`,
      awaitPromise: true,
    });
    await sleep(1200);
    await checkOverflow("Scenario 01 (Outdated Driver)");

    // Verify command labeling for Scenario 1
    const s1Labels = await cdp.send("Runtime.evaluate", {
      expression: `(() => {
        const btns = [...document.querySelectorAll('#findings-list .btn-copy')].map(b => b.textContent);
        const labels = [...document.querySelectorAll('#findings-list .verify-label')].map(l => l.textContent);
        return { btns, labels };
      })()`,
      returnByValue: true,
    });
    console.log("  Scenario 1 finding buttons:", s1Labels.result.value.btns);
    console.log("  Scenario 1 finding labels:", s1Labels.result.value.labels);

    // 3. Scenario 2 (Selectors Conflict)
    console.log("\nTesting Scenario 2: Conflicting Selectors...");
    await cdp.send("Runtime.evaluate", {
      expression: `(async () => {
        document.getElementById('btn-example-selectors').click();
        await new Promise(r => setTimeout(r, 400));
        document.getElementById('btn-analyze').click();
      })()`,
      awaitPromise: true,
    });
    await sleep(1200);
    await checkOverflow("Scenario 02 (Conflicting Selectors)");

    const s2Labels = await cdp.send("Runtime.evaluate", {
      expression: `(() => {
        const btns = [...document.querySelectorAll('#findings-list .btn-copy')].map(b => b.textContent);
        const labels = [...document.querySelectorAll('#findings-list .verify-label')].map(l => l.textContent);
        return { btns, labels };
      })()`,
      returnByValue: true,
    });
    console.log("  Scenario 2 finding buttons:", s2Labels.result.value.btns);
    console.log("  Scenario 2 finding labels:", s2Labels.result.value.labels);

    // 4. Scenario 3 (Component Mismatch)
    console.log("\nTesting Scenario 3: Driver Component Mismatch...");
    await cdp.send("Runtime.evaluate", {
      expression: `(async () => {
        document.getElementById('btn-example-components').click();
        await new Promise(r => setTimeout(r, 400));
        document.getElementById('btn-analyze').click();
      })()`,
      awaitPromise: true,
    });
    await sleep(1200);
    await checkOverflow("Scenario 03 (Driver Component Mismatch)");

    const s3Labels = await cdp.send("Runtime.evaluate", {
      expression: `(() => {
        const btns = [...document.querySelectorAll('#findings-list .btn-copy')].map(b => b.textContent);
        const labels = [...document.querySelectorAll('#findings-list .verify-label')].map(l => l.textContent);
        return { btns, labels };
      })()`,
      returnByValue: true,
    });
    console.log("  Scenario 3 finding buttons:", s3Labels.result.value.btns);
    console.log("  Scenario 3 finding labels:", s3Labels.result.value.labels);

    // 5. Test Copy Button Interaction
    console.log("\nTesting Copy Button Interaction...");
    const copyTestRes = await cdp.send("Runtime.evaluate", {
      expression: `(() => {
        const firstBtn = document.querySelector('#findings-list .btn-copy');
        if (!firstBtn) return { error: 'No copy button found' };
        firstBtn.click();
        return { text: firstBtn.textContent, hasClass: firstBtn.classList.contains('copied') };
      })()`,
      returnByValue: true,
    });
    console.log("  Copy button click result:", copyTestRes.result.value);

    // 6. Check console errors
    console.log("\nConsole errors count:", consoleErrors.length);
    if (consoleErrors.length > 0) {
      console.log("Console errors:", consoleErrors);
    }
  } finally {
    if (cdp) cdp.close();
    chromeProc.kill();
    await sleep(500);
    try {
      fs.rmSync(tmpProfile, { recursive: true, force: true });
    } catch {}
  }
}

run().catch((e) => {
  console.error(e);
  process.exit(1);
});
