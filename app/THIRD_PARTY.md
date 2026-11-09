# Third-party notices

- **simple-icons** (https://simpleicons.org, CC0 1.0): service icons in the connector list. All brand names and logos are trademarks of their respective owners and are used only to identify the connected service.
- **mattpocock/skills — `grilling`** (https://github.com/mattpocock/skills): the approach of "요구사항 다듬기" (rounds of the decisions that can be settled now, each question with a recommended answer, worded so that accepting means yes, facts left to the worker) is adapted from this skill. No text is copied; the prompt in `crates/dduck-core/src/brief_refine.rs` is our own Korean wording.
- **sys-locale** (https://crates.io/crates/sys-locale, MIT OR Apache-2.0): reads the OS language for "System language" (app-language). Only whether it starts with `ko` is used; the locale string is not stored or logged.
- **@xterm/addon-unicode11** (https://github.com/xtermjs/xterm.js, MIT, pinned 0.9.0): Unicode 11 character widths for the task log view, alongside `@xterm/xterm` (MIT).

## Licenses other than MIT / Apache-2.0

Most dependencies are MIT or Apache-2.0. The ones below carry other terms. Checked against `Cargo.lock` (`cargo metadata`) and `node_modules/*/package.json`.

- **Pretendard** (https://github.com/orioncactus/pretendard, SIL Open Font License 1.1): the bundled UI font (`pretendard` npm package, variable dynamic subset). The font files are bundled unmodified and are not sold by themselves. Copyright (c) 2021 Kil Hyung-jin, with Reserved Font Name Pretendard; full license text: `node_modules/pretendard/dist/LICENSE.txt`.
- **MPL-2.0** — used unmodified, source available from crates.io / npm:
  - Rust, shipped in the app: `cssparser`, `cssparser-macros`, `selectors`, `dtoa-short` (through `ammonia`, the HTML sanitizer for report attachments) and `option-ext`.
  - npm, build time only (not shipped): `lightningcss` (through Tailwind CSS v4).
- **Unicode-3.0** (Unicode License v3): ICU4X data and support crates used for URL and text normalization — `icu_collections`, `icu_locale_core`, `icu_normalizer`, `icu_normalizer_data`, `icu_properties`, `icu_properties_data`, `icu_provider`, `litemap`, `potential_utf`, `tinystr`, `writeable`, `yoke`, `yoke-derive`, `zerofrom`, `zerofrom-derive`, `zerotrie`, `zerovec`, `zerovec-derive`; also part of `unicode-ident` and `unicode-normalization` (`(MIT OR Apache-2.0) AND Unicode-3.0`).
- Other permissive licenses also appear (BSD-3-Clause, ISC, Zlib, CC0-1.0, for example `brotli`, `subtle`, `ring`, `rustls-webpki`, `zlib-rs`, `notify`). Run `cargo metadata --format-version 1` in `app/` for the full list.
