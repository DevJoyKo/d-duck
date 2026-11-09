# D.Duck app

Claude Code 사용 통계와 로컬 작업 관제를 위한 Tauri 2 데스크톱 앱.
스펙: [`../openspec/specs/`](../openspec/specs/), 결정 요약: [`../docs/DECISIONS-SUMMARY.md`](../docs/DECISIONS-SUMMARY.md).

## 구조

| 경로 | 내용 |
|---|---|
| `crates/dduck-core/` | 수집·DB·집계·Claude CLI 연동 (Tauri 비의존 Rust 라이브러리) |
| `src-tauri/` | Tauri 셸: 창, 트레이, IPC 명령, 알림, 대화상자 |
| `src/` | React 19 + Vite + TypeScript 화면 |

## 요구 사항

- Rust 1.85 이상 (`rustc --version`, edition 2024)
- Node 22 이상, npm
- macOS: Xcode Command Line Tools

## 명령

```bash
npm install                 # 의존성 설치
npm run tauri dev           # 개발 모드 실행
npm run tauri build         # 릴리스 빌드 (macOS: .dmg)

cargo test --workspace      # Rust 테스트
cargo clippy --all-targets  # Rust 린트
npm test                    # 화면 테스트 (Vitest)
npm run lint                # 화면 린트 (ESLint)
npm run build               # 화면 빌드만
```
