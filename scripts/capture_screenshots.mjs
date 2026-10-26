import http from "node:http";
import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const CHROME_PATH = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
const PORT = 9223;
const TARGET_URL = "http://127.0.0.1:8000/";
const OUTPUT_DIR = path.resolve("docs/screenshots");

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

async function capture(cdp, filename) {
  const res = await cdp.send("Page.captureScreenshot", {
    format: "png",
    captureBeyondViewport: false,
  });
  const filePath = path.join(OUTPUT_DIR, filename);
  fs.writeFileSync(filePath, Buffer.from(res.data, "base64"));
  console.log(`Saved screenshot: ${filePath}`);
}

async function run() {
  fs.mkdirSync(OUTPUT_DIR, { recursive: true });
  const tmpProfile = path.join(process.env.TEMP || "C:\\Temp", `chrome_shot_${Date.now()}`);
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

    await cdp.send("Page.enable");
    await cdp.send("Runtime.enable");
    await sleep(800);

    // 1. Desktop Home (1440x900)
    console.log("Capturing 1440x900 Desktop Home...");
    await cdp.send("Emulation.setDeviceMetricsOverride", {
      width: 1440,
      height: 900,
      deviceScaleFactor: 1,
      mobile: false,
    });
    await sleep(400);
    await capture(cdp, "01_desktop_home.png");

    // 2. Desktop Findings (1440x900)
    console.log("Analyzing and Capturing 1440x900 Desktop Findings...");
    await cdp.send("Runtime.evaluate", {
      expression: `(async () => {
        document.getElementById('btn-analyze').click();
      })()`,
      awaitPromise: true,
    });
    await sleep(1500);
    await capture(cdp, "02_desktop_findings.png");

    // 3. Mobile Home (390x844)
    console.log("Navigating to fresh home for 390x844 Mobile...");
    await cdp.send("Page.navigate", { url: TARGET_URL });
    await sleep(800);
    await cdp.send("Emulation.setDeviceMetricsOverride", {
      width: 390,
      height: 844,
      deviceScaleFactor: 1,
      mobile: true,
    });
    await sleep(400);
    await capture(cdp, "03_mobile_home.png");

    // 4. Mobile Findings (390x844)
    console.log("Analyzing and Capturing 390x844 Mobile Findings...");
    await cdp.send("Runtime.evaluate", {
      expression: `(async () => {
        document.getElementById('btn-analyze').click();
        await new Promise(r => setTimeout(r, 1200));
        document.getElementById('state-report').scrollIntoView({ behavior: 'instant', block: 'start' });
      })()`,
      awaitPromise: true,
    });
    await sleep(600);
    await capture(cdp, "04_mobile_findings.png");

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
