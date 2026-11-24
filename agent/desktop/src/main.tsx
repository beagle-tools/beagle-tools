import React from "react";
import ReactDOM from "react-dom/client";

import App from "./App";
import "./styles/index.css";

async function boot(): Promise<void> {
  // Playwright runs the frontend in a plain browser (`vite --mode e2e`) with
  // every Tauri command answered by fixtures. Vite inlines MODE, so in any
  // other build this branch and the mock chunk are dropped entirely.
  if (import.meta.env.MODE === "e2e") {
    const { installIpcMocks } = await import("./e2e/mockIpc");
    installIpcMocks();
  }
  const root = document.getElementById("root");
  if (root) {
    ReactDOM.createRoot(root).render(
      <React.StrictMode>
        <App />
      </React.StrictMode>,
    );
  }
}

void boot();
