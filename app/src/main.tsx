import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";
import { applyTheme, loadTheme } from "./lib/theme";
import { initLanguage } from "./i18n/language";
import { loadPlatform } from "./lib/platform";

// Before the first paint, so a chosen theme never flashes the other one.
applyTheme(loadTheme());
// Same for the language: the cached choice sets the text and <html lang> before the DB value is read.
initLanguage();

// The OS (windows-support) decides the window chrome and path rules, so it is read before the first render. A slow or
// failed read renders as macOS after a short wait rather than holding the window blank.
void Promise.race([loadPlatform(), new Promise((r) => setTimeout(r, 500))]).then(() =>
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  ),
);
