import { applyLanguage } from "../i18n/language";

// Existing tests read Korean text: pin the app language (jsdom reports en-US). i18n tests switch it themselves.
applyLanguage("ko");

// jsdom lacks ResizeObserver, which Recharts' ResponsiveContainer needs.
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
globalThis.ResizeObserver ??= ResizeObserverStub as unknown as typeof ResizeObserver;
