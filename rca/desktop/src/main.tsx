import React from "react";
import ReactDOM from "react-dom/client";

import App from "./App";

async function boot(): Promise<void> {
  // Playwright runs the app in a plain browser (`vite --mode e2e`) with the
  // Tauri backend mocked. The MODE check is a build-time constant, so a
  // production build drops this branch and the fixtures never ship.
  if (import.meta.env.MODE === "e2e") {
    const { installMockIpc } = await import("./e2e/mockIpc");
    installMockIpc();
  }
  const root = document.getElementById("root");
  if (root === null) {
    throw new Error("index.html must contain #root");
  }
  ReactDOM.createRoot(root).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

void boot();
