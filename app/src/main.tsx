import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";
import { applyTheme, loadTheme } from "./lib/theme";
import { initLanguage } from "./i18n/language";

// Before the first paint, so a chosen theme never flashes the other one.
applyTheme(loadTheme());
// Same for the language: the cached choice sets the text and <html lang> before the DB value is read.
initLanguage();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
