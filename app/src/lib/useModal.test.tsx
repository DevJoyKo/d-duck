// useModal (sheet-focus): focus enters the sheet, Tab stays inside, Escape closes through the sheet's own close path,
// and focus goes back where it came from. There is no user-event library: focus is set with focus() and keydown is
// dispatched on the active element, so these tests prove the hook's wrapping and guards, not native Tab movement.
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { StrictMode, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useModal, type ModalOptions } from "./useModal";

function Sheet({ onClose, label = "시트", options = {}, children }: { onClose: () => void; label?: string; options?: Omit<ModalOptions, "initial">; children?: ReactNode }) {
  const dialog = useRef<HTMLDivElement>(null);
  const closeBtn = useRef<HTMLButtonElement>(null);
  useModal(dialog, onClose, { initial: closeBtn, ...options });
  return (
    <div ref={dialog} role="dialog" aria-modal="true" aria-label={label}>
      <button ref={closeBtn} onClick={onClose}>
        {label} 닫기
      </button>
      {children ?? (
        <>
          <input aria-label={`${label} 이름`} />
          <button>{label} 저장</button>
        </>
      )}
    </div>
  );
}

function Page({ sheet }: { sheet: (close: () => void) => ReactNode }) {
  const [open, setOpen] = useState(false);
  return (
    <div>
      <button onClick={() => setOpen(true)}>열기</button>
      <input aria-label="뒤 페이지" />
      {open && sheet(() => setOpen(false))}
    </div>
  );
}

const tab = (shift = false) => fireEvent.keyDown(document.activeElement ?? document.body, { key: "Tab", shiftKey: shift });
const button = (name: string) => screen.getByRole("button", { name });

function openPage(sheet: (close: () => void) => ReactNode = (close) => <Sheet onClose={close} />) {
  render(<Page sheet={sheet} />);
  const opener = button("열기");
  opener.focus();
  fireEvent.click(opener);
  return opener;
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("useModal: focus in", () => {
  it("moves focus to the initial control on open", () => {
    openPage();
    expect(document.activeElement).toBe(button("시트 닫기"));
  });

  it("keeps an autoFocus field instead of taking focus away", () => {
    openPage((close) => (
      <Sheet onClose={close}>
        <input aria-label="링크" autoFocus />
      </Sheet>
    ));
    expect(document.activeElement).toBe(screen.getByLabelText("링크"));
  });
});

describe("useModal: Tab", () => {
  it("wraps Tab from the last control to the first and Shift+Tab from the first to the last", () => {
    openPage();
    act(() => button("시트 저장").focus());
    tab();
    expect(document.activeElement).toBe(button("시트 닫기"));
    tab(true);
    expect(document.activeElement).toBe(button("시트 저장"));
  });

  it("steps through the controls in order", () => {
    openPage();
    tab();
    expect(document.activeElement).toBe(screen.getByLabelText("시트 이름"));
    tab();
    expect(document.activeElement).toBe(button("시트 저장"));
  });

  it("sends Tab from the body to the first control and Shift+Tab to the last", () => {
    openPage();
    act(() => (document.activeElement as HTMLElement).blur());
    expect(document.activeElement).toBe(document.body);
    tab();
    expect(document.activeElement).toBe(button("시트 닫기"));
    act(() => (document.activeElement as HTMLElement).blur());
    tab(true);
    expect(document.activeElement).toBe(button("시트 저장"));
  });

  it("skips disabled, hidden, inert, aria-hidden and tabindex=-1 controls", () => {
    openPage((close) => (
      <Sheet onClose={close}>
        <button disabled>꺼짐</button>
        <div hidden>
          <button>숨김</button>
        </div>
        <div aria-hidden="true">
          <button>가림</button>
        </div>
        <button tabIndex={-1}>건너뜀</button>
        <button>마지막</button>
      </Sheet>
    ));
    tab();
    expect(document.activeElement).toBe(button("마지막"));
    tab();
    expect(document.activeElement).toBe(button("시트 닫기"));
  });

  it("pulls focus that lands outside the sheet back to the initial control", () => {
    openPage();
    act(() => screen.getByLabelText("뒤 페이지").focus());
    expect(document.activeElement).toBe(button("시트 닫기"));
  });
});

describe("useModal: Escape", () => {
  it("calls onClose and returns focus to the opener", () => {
    const opener = openPage();
    act(() => button("시트 저장").focus());
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(opener);
  });

  it("ignores Escape while an input method is composing", () => {
    openPage();
    const field = screen.getByLabelText("시트 이름");
    act(() => field.focus());
    fireEvent.keyDown(field, { key: "Escape", isComposing: true });
    fireEvent.keyDown(field, { key: "Escape", keyCode: 229 });
    expect(screen.queryByRole("dialog")).not.toBeNull();
  });

  it("ignores an Escape an inner control already handled (defaultPrevented)", () => {
    openPage((close) => (
      <Sheet onClose={close}>
        <input aria-label="고르기" onKeyDown={(e) => e.key === "Escape" && e.preventDefault()} />
      </Sheet>
    ));
    fireEvent.keyDown(screen.getByLabelText("고르기"), { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeNull();
  });

  it("keeps the sheet open when an inner panel stops the Escape", () => {
    function Panel() {
      const [open, setOpen] = useState(true);
      const onKeyDown = (e: KeyboardEvent) => {
        if (e.key !== "Escape") return;
        e.stopPropagation();
        setOpen(false);
      };
      return open ? (
        <div onKeyDown={onKeyDown}>
          <input aria-label="패널 검색" />
        </div>
      ) : null;
    }
    openPage((close) => (
      <Sheet onClose={close}>
        <Panel />
      </Sheet>
    ));
    fireEvent.keyDown(screen.getByLabelText("패널 검색"), { key: "Escape" });
    expect(screen.queryByLabelText("패널 검색")).toBeNull();
    expect(screen.queryByRole("dialog")).not.toBeNull();
  });

  it("uses the latest onClose and never re-runs focus when its identity changes while typing", () => {
    const seen: string[] = [];
    function Dirty({ close }: { close: () => void }) {
      const [text, setText] = useState("");
      // A new function on every render, as the dirty-guarded sheets have.
      const onClose = () => {
        seen.push(text);
        close();
      };
      return (
        <Sheet onClose={onClose}>
          <input aria-label="글" value={text} onChange={(e) => setText(e.target.value)} />
        </Sheet>
      );
    }
    openPage((close) => <Dirty close={close} />);
    const field = screen.getByLabelText("글") as HTMLInputElement;
    act(() => field.focus());
    fireEvent.change(field, { target: { value: "a" } });
    fireEvent.change(field, { target: { value: "ab" } });
    expect(document.activeElement).toBe(field);
    fireEvent.keyDown(field, { key: "Escape" });
    expect(seen).toEqual(["ab"]);
  });
});

describe("useModal: stack", () => {
  it("lets only the top sheet react to Escape and Tab", () => {
    const lower = vi.fn();
    function Two() {
      const [upper, setUpper] = useState(true);
      return (
        <>
          <Sheet onClose={lower} label="아래" />
          {upper && <Sheet onClose={() => setUpper(false)} label="위" />}
        </>
      );
    }
    render(<Two />);
    expect(document.activeElement).toBe(button("위 닫기"));
    act(() => button("위 저장").focus());
    tab();
    expect(document.activeElement).toBe(button("위 닫기"));
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    expect(screen.queryByRole("dialog", { name: "위" })).toBeNull();
    expect(lower).not.toHaveBeenCalled();
    // The lower sheet is on top again.
    act(() => button("아래 저장").focus());
    tab();
    expect(document.activeElement).toBe(button("아래 닫기"));
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    expect(lower).toHaveBeenCalledTimes(1);
  });
});

describe("useModal: focus return", () => {
  it("returns focus to returnTo() instead of the opener", () => {
    render(<Page sheet={(close) => <Sheet onClose={close} options={{ returnTo: () => screen.getByLabelText("뒤 페이지") }} />} />);
    act(() => button("열기").focus());
    fireEvent.click(button("열기"));
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    expect(document.activeElement).toBe(screen.getByLabelText("뒤 페이지"));
  });

  it("leaves focus alone when returnTo() gives nothing", () => {
    render(<Page sheet={(close) => <Sheet onClose={close} options={{ returnTo: () => null }} />} />);
    act(() => button("열기").focus());
    fireEvent.click(button("열기"));
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    expect(document.activeElement).toBe(document.body);
  });

  it("leaves focus alone when the opener is no longer in the page", () => {
    function Gone() {
      const [open, setOpen] = useState(false);
      return (
        <div>
          {!open && <button onClick={() => setOpen(true)}>사라지는 열기</button>}
          {open && <Sheet onClose={() => setOpen(false)} />}
        </div>
      );
    }
    render(<Gone />);
    const opener = button("사라지는 열기");
    act(() => opener.focus());
    fireEvent.click(opener);
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    expect(opener.isConnected).toBe(false);
    expect(document.activeElement).toBe(document.body);
  });
});

describe("useModal: StrictMode (dev)", () => {
  it("keeps an autoFocus field through the effect re-run and still returns focus on a real close", () => {
    render(
      <StrictMode>
        <Page
          sheet={(close) => (
            <Sheet onClose={close} options={{ returnTo: () => screen.getByLabelText("뒤 페이지") }}>
              <input aria-label="링크" autoFocus />
            </Sheet>
          )}
        />
      </StrictMode>,
    );
    act(() => button("열기").focus());
    fireEvent.click(button("열기"));
    expect(document.activeElement).toBe(screen.getByLabelText("링크"));
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(screen.getByLabelText("뒤 페이지"));
  });
});
