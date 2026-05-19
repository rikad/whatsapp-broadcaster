import indexHtml from "./index.html" with { type: "text" };
import rendererJs from "./renderer.js" with { type: "text" };
import whatsapp from "./whatsapp.js";
import api from "./api.js";
import { spawn } from "node:child_process";

const PORT = parseInt(process.env.PORT, 10) || 1111;

api.setWhatsapp(whatsapp);
api.setStaticAssets({ indexHtml, rendererJs });
api.bindEvents(whatsapp.events);
api.start(PORT);

const url = `http://localhost:${PORT}`;
console.log(`WhatsApp Broadcaster — open ${url} in your browser`);

if (process.env.NO_OPEN !== "1") {
  openBrowser(url);
}

whatsapp.initialize().catch((err) => {
  console.error("WhatsApp init failed:", err);
});

process.on("unhandledRejection", (reason) => {
  console.error("Unhandled Rejection:", reason);
  whatsapp.events.emit("error", reason?.message || String(reason));
});

function openBrowser(target) {
  try {
    const platform = process.platform;
    let cmd, args;
    if (platform === "darwin") {
      cmd = "open";
      args = [target];
    } else if (platform === "win32") {
      cmd = "cmd";
      args = ["/c", "start", "", target];
    } else {
      cmd = "xdg-open";
      args = [target];
    }
    spawn(cmd, args, { stdio: "ignore", detached: true }).unref();
  } catch (err) {
    console.log(`Could not auto-open browser: ${err.message}`);
  }
}
