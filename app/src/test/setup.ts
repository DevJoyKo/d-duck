import { configure } from "@testing-library/react";
import { applyLanguage } from "../i18n/language";

// Slow CI runners (hosted Intel Macs) need longer findBy/waitFor waits than a local machine.
if (import.meta.env.CI) configure({ asyncUtilTimeout: 10_000 });

// Existing tests read Korean text: pin the app language (jsdom reports en-US). i18n tests switch it themselves.
applyLanguage("ko");

// jsdom lacks ResizeObserver, which Recharts' ResponsiveContainer needs.
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
globalThis.ResizeObserver ??= ResizeObserverStub as unknown as typeof ResizeObserver;
