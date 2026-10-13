import React from "react";
import ReactDOM from "react-dom/client";
import { MemoryRouter } from "react-router-dom";
import App from "./App";
import ErrorBoundary from "./components/ErrorBoundary";
import "./styles.css";

window.addEventListener("error", (event) => {
  const root = document.getElementById("root");
  if (root && root.innerHTML.trim() === "") {
    root.innerHTML = `
      <div style="padding:40px;color:#e6e9ee;background:#14171c;font-family:Segoe UI,sans-serif;height:100vh;box-sizing:border-box">
        <h2 style="color:#e5534b">Application Error</h2>
        <p style="color:#9aa4b2">An error occurred during startup:</p>
        <pre style="background:#1c2027;padding:16px;border-radius:8px;border:1px solid #323945;color:#f06a63;overflow:auto">${event.message || event.error}</pre>
        <button onclick="window.location.reload()" style="margin-top:20px;padding:10px 20px;background:#4f8cff;color:#fff;border:none;border-radius:6px;cursor:pointer;font-weight:600">Reload</button>
      </div>
    `;
  }
});

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ErrorBoundary>
      <MemoryRouter>
        <App />
      </MemoryRouter>
    </ErrorBoundary>
  </React.StrictMode>
);
