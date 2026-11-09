// Claude Code install commands shown when the CLI is missing (onboarding welcome card). Kept out of tour.ts, whose
// content must hold no URLs: this is a command the user pastes into a terminal, never fetched by the app.
// Source: the official setup page (native installer), with Homebrew as the alternative.
export const INSTALL_COMMAND = "curl -fsSL https://claude.ai/install.sh | bash";
export const INSTALL_BREW = "brew install --cask claude-code";
