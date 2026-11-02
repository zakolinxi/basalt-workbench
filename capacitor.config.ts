import type { CapacitorConfig } from "@capacitor/cli";

/**
 * Android shell for the Basalt phone client.
 *
 * The app ships the same web bundle the desktop gateway serves. It is served
 * from the device itself (`webDir`), and talks to whichever PC the user pairs
 * with over plain HTTP on the LAN — hence the http scheme and mixed-content
 * allowance, which would otherwise be blocked by the WebView.
 */
const config: CapacitorConfig = {
  appId: "com.basalt.remote",
  appName: "Basalt Remote",
  webDir: "dist",
  android: {
    allowMixedContent: true,
  },
  server: {
    androidScheme: "http",
    cleartext: true,
  },
};

export default config;
