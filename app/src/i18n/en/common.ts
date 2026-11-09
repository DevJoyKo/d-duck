import type ko from "../ko/common";
import type { Shape } from "../types";

export default {
  theme: {
    system: "System theme",
    light: "Light theme",
    dark: "Dark theme",
  },
  language: {
    system: "System language",
  },
  actions: {
    close: "Close",
    cancel: "Cancel",
    save: "Save",
    more: "More",
  },
  draft: {
    replace: "Replace the draft you're writing?",
  },
} satisfies Shape<typeof ko>;
