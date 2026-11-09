import js from "@eslint/js";
import globals from "globals";
import react from "eslint-plugin-react";
import reactHooks from "eslint-plugin-react-hooks";
import tseslint from "typescript-eslint";

export default tseslint.config(
  { ignores: ["dist", "src-tauri", "target", "crates"] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["src/**/*.{ts,tsx}"],
    languageOptions: { globals: globals.browser },
    plugins: { react, "react-hooks": reactHooks },
    settings: { react: { version: "detect" } },
    rules: {
      ...reactHooks.configs.recommended.rules,
      // Log and CLI output must never be rendered as HTML (D-015, D7).
      "react/no-danger": "error",
      "no-restricted-properties": [
        "error",
        { property: "innerHTML", message: "Render untrusted text as text nodes only." },
        { property: "outerHTML", message: "Render untrusted text as text nodes only." },
        { property: "insertAdjacentHTML", message: "Render untrusted text as text nodes only." },
        { property: "createContextualFragment", message: "Render untrusted text as text nodes only." },
        { property: "parseFromString", message: "Do not parse untrusted text as markup." },
        { object: "document", property: "write", message: "Render untrusted text as text nodes only." },
        { object: "document", property: "writeln", message: "Render untrusted text as text nodes only." },
      ],
    },
  },
);
