import React from "react";
import ReactDOM from "react-dom/client";

import App from "./App";
import "./styles.css";

// The macOS webview applies system "smart typing" to text fields, which
// silently rewrites identifiers like repo slugs and usernames. Policy:
// never auto-replace anywhere (autocorrect/autocapitalize off), but keep
// passive spellcheck squiggles on prose textareas (right-click offers
// suggestions; nothing changes unless you pick one). Monospace textareas
// hold code-ish content, so they opt out entirely.
function disableAutocorrect(root: ParentNode) {
  root.querySelectorAll("input, textarea").forEach((el) => {
    el.setAttribute("autocorrect", "off");
    el.setAttribute("autocapitalize", "off");
    const prose = el.tagName === "TEXTAREA" && !el.className.includes("font-mono");
    el.setAttribute("spellcheck", prose ? "true" : "false");
  });
}
const observer = new MutationObserver(() => {
  disableAutocorrect(document);
});
observer.observe(document.documentElement, { childList: true, subtree: true });
disableAutocorrect(document);

const root = document.getElementById("root");
if (root) {
  ReactDOM.createRoot(root).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}
