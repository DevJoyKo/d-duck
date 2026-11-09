import type ko from "../ko/auth";
import type { Shape } from "../types";

export default {
  banner: "Log in to Claude to hand off tasks",
  gate: "Log in to Claude to use this",
  cloud: "Cloud routines need a Claude account login",
  loginHint: "The Claude CLI logs you in through the browser. The app only checks whether you're logged in and never reads credentials",
  cloudHint: "A token, API key or cloud provider connection takes priority. Remove it and log in with a Claude account to use this",
  settingsOut: "Not logged in",
  waiting: "Closes on its own once you approve in the browser",
  cancelAsk: "Cancel the login?",
  doneBody: "You can hand off tasks now",
  connector: "Needs a Claude account login",
  progress: "Log in from your browser",
  link: "Open in browser",
  console: "Log in with a Console account",
  failed: "The login didn't finish",
  confirm: "Open Claude login in Terminal?",
  unknown: "Couldn't check the login",
  expiry: "Your Claude login expires soon",
  logout: "To log out, run `claude auth logout` in Terminal",
  done: "Logged in",
  actions: {
    login: "Log in",
    relogin: "Log in again",
    recheck: "Check again",
    copyCommand: "Copy command",
    keepWaiting: "Keep waiting",
    cancelLogin: "Cancel login",
    inTerminal: "Log in from Terminal",
    retry: "Try again",
  },
  terminal: {
    title: "Log in from Terminal",
    body: "Couldn't open Terminal. Paste this command into Terminal.",
  },
  fail: {
    timeout: "Stopped waiting after 5 minutes",
    exited: "The Claude CLI exited without logging in",
    needsTerminal: "A code has to be pasted, so continue in Terminal",
    spawn: "Couldn't run the Claude CLI",
  },
} satisfies Shape<typeof ko>;
