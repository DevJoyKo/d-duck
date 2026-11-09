// The duck's yellow means one thing: "this is waiting for you" (palette-refresh, D-176). These are the only places app
// code may paint it; yellowRule.test.ts fails on any other `*-duck` class. The yellow inside the duck drawing (beak,
// feet, the waiting bubble, the tutor's body) is the other exception and lives in DuckAvatar (D-177). The agents page's
// "막힌 작업" button stays the ink primary: that page already rings its waiting cards, and one kind of yellow per screen.
export const YELLOW = {
  /** A card whose session is waiting for your answer: one step higher shadow and a 2px yellow ring outside the card.
   *  Selection is an inner ink outline and keyboard focus an outer ink outline, so the three never look alike. */
  waitingCard: "shadow-e2 ring-2 ring-duck",
  /** The sidebar count of sessions waiting for an answer. */
  badge: "bg-duck",
  /** The duck's own foot: the "you are here" footprint on the report's flow (part of the drawing, D-177). */
  duckFoot: "var(--color-duck)",
} as const;
