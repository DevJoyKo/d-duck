// Service icons for the connector list (D-073). Bundled SVG paths from simple-icons (CC0): nothing is fetched,
// so no favicon request leaks which services are connected. Unknown services get a first-letter badge.
// Brand marks are trademarks of their owners; see THIRD_PARTY.md.
import {
  siAsana,
  siAtlassian,
  siCloudflare,
  siConfluence,
  siExpo,
  siFigma,
  siGithub,
  siGmail,
  siGooglecalendar,
  siGoogledrive,
  siJira,
  siLinear,
  siMermaid,
  siNotebooklm,
  siNotion,
  siPostgresql,
  siSentry,
  siStripe,
  siSupabase,
  siVercel,
  type SimpleIcon,
} from "simple-icons";

// First match wins; checked against "host name" in lower case, so specific products come before their vendor.
const RULES: [RegExp, SimpleIcon][] = [
  [/gmail/, siGmail],
  [/google.?calendar|calendarmcp\.googleapis|calendar\.google\./, siGooglecalendar],
  [/google.?drive|drivemcp\.googleapis|drive\.google\./, siGoogledrive],
  [/notebooklm/, siNotebooklm],
  [/notion/, siNotion],
  [/figma/, siFigma],
  [/jira/, siJira],
  [/confluence/, siConfluence],
  [/atlassian/, siAtlassian],
  [/cloudflare/, siCloudflare],
  [/(^|[\s:.])expo($|[\s:])|(^|\.)expo\.dev\b/, siExpo],
  [/mermaid/, siMermaid],
  [/github/, siGithub],
  [/(^|\.)linear\.app\b|(^|[\s:])linear($|[\s:])/, siLinear],
  [/sentry/, siSentry],
  [/vercel/, siVercel],
  [/supabase/, siSupabase],
  [/stripe/, siStripe],
  [/asana/, siAsana],
  [/postgres/, siPostgresql],
];

export function iconFor(name: string, host: string | null): SimpleIcon | null {
  const hay = `${host ?? ""} ${name}`.toLowerCase();
  return RULES.find(([re]) => re.test(hay))?.[1] ?? null;
}

/** Display name without the "claude.ai " / "plugin:x:" prefix, used for the badge letter. */
export function serviceName(name: string): string {
  return name.replace(/^claude\.ai\s+/i, "").replace(/^plugin:[^:]+:/i, "");
}

function dark(hex: string): boolean {
  const n = parseInt(hex, 16);
  const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  return 0.2126 * r + 0.7152 * g + 0.0722 * b < 60;
}

export function ConnectorIcon({ name, host }: { name: string; host: string | null }) {
  const icon = iconFor(name, host);
  const box = "flex size-6 shrink-0 items-center justify-center rounded-md border border-line bg-paper";
  if (!icon) {
    const letter = serviceName(name).trim().charAt(0).toUpperCase() || "?";
    return (
      <span aria-hidden className={`${box} text-[11px] font-semibold text-ink-2`}>
        {letter}
      </span>
    );
  }
  // Brand marks sit on a white chip in both themes so their colours keep enough contrast (dark-theme review MEDIUM).
  const fill = dark(icon.hex) ? "#111" : `#${icon.hex}`;
  return (
    <span aria-hidden className={`${box} bg-white`}>
      <svg viewBox="0 0 24 24" width="14" height="14" fill={fill}>
        <path d={icon.path} />
      </svg>
    </span>
  );
}
