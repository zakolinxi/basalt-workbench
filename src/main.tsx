import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "@xterm/xterm/css/xterm.css";
import "./styles.css";
import { applyCornerStyle, loadCornerStyle } from "./lib/theme";
import { isDesktop } from "./lib/bridge";

applyCornerStyle(loadCornerStyle());

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

// The phone build is installable as a PWA; the desktop shell has no origin.
if (!isDesktop() && window.location.protocol.startsWith("http")) {
  window.addEventListener("load", () => {
    void navigator.serviceWorker?.register("/sw.js").catch(() => {
      // Registration is a progressive enhancement; ignore failures.
    });
  });
}
