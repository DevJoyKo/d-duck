// Browser-only stand-in for @tauri-apps/api/event (see mock/core.ts). Mock handlers can `emit` to the listeners.
import { localize } from "./english";
import { DATA_LANG, backendLang } from "./lang";

type Handler =(e: { payload: unknown }) => void;
const handlers = new Map<string, Set<Handler>>();

export async function listen<T>(name?: string, cb?: (e: { payload: T }) => void): Promise<() => void> {
  if (!name || !cb) return () => {};
  const set = handlers.get(name) ?? new Set<Handler>();
  set.add(cb as Handler);
  handlers.set(name, set);
  return () => set.delete(cb as Handler);
}

export function emit(name: string, payload: unknown) {
  const p = localize(payload, backendLang(), DATA_LANG);
  handlers.get(name)?.forEach((h) => h({ payload: p }));
}
