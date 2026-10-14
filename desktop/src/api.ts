import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface Settings {
  profileId: string | null;
  profileName: string;
  target: string;
  port: number | null;
  identityFile: string;
  transport: TransportSettings;
  openShortcut: string;
  shortcutEnabled: boolean;
  directShortcutsEnabled: boolean;
  directShareShortcut: string | null;
  directCopyShortcut: string | null;
  showPreview: boolean;
  closeAfterCopy: boolean;
}

export interface TransportSettings {
  sshOptions: string[];
  sshBinary: string;
  maxSize: string;
  timeoutMs: string;
  resyncFrames: boolean;
  resyncMaxBytes: string;
}

export function defaultTransport(): TransportSettings {
  return { sshOptions: [], sshBinary: "", maxSize: "10485760", timeoutMs: "7000", resyncFrames: true, resyncMaxBytes: "8192" };
}

export function cloneTransport(value: TransportSettings): TransportSettings {
  return { ...value, sshOptions: [...value.sshOptions] };
}

export function cloneSettings(value: Settings): Settings {
  return { ...value, transport: cloneTransport(value.transport) };
}

export interface ConnectionProfile {
  id: string;
  name: string;
  target: string;
  port: number | null;
  identityFile: string;
  transport: TransportSettings;
}

export interface PanelView {
  settings: Settings;
  profiles: ConnectionProfile[];
  item: null | {
    kind: "text" | "image";
    label: string;
    text: string | null;
    imageUrl: string | null;
    encodedBytes: number;
    createdAt: number;
  };
  connected: boolean;
  clipboardChecked: boolean;
  shortcutStatus: string;
  trayStatus: string;
  shortcutAvailable: boolean;
  platform: string;
  settingsSaved: boolean;
  settingsWritable: boolean;
  settingsNotice: string | null;
  activeShortcut: string | null;
  settingsRevision: string | null;
  migrationSummary: string | null;
  connectionReport: ConnectionReport | null;
}

export interface ConnectionReport {
  healthy: boolean;
  summary: string;
  hint: string | null;
  checks: Array<{ stage: string; label: string; status: "passed" | "failed" | "not_checked"; detail: string }>;
}

export const initialView: PanelView = {
  settings: { profileId: "default", profileName: "Default", target: "", port: null, identityFile: "", transport: defaultTransport(), openShortcut: "CommandOrControl+Shift+V", shortcutEnabled: true, directShortcutsEnabled: false, directShareShortcut: null, directCopyShortcut: null, showPreview: true, closeAfterCopy: false },
  profiles: [{ id: "default", name: "Default", target: "", port: null, identityFile: "", transport: defaultTransport() }],
  item: null,
  connected: false,
  clipboardChecked: false,
  shortcutStatus: "Waiting for the desktop app",
  trayStatus: "Waiting for the desktop app",
  shortcutAvailable: true,
  platform: "unknown",
  settingsSaved: false,
  settingsWritable: true,
  settingsNotice: null,
  activeShortcut: null,
  settingsRevision: null,
  migrationSummary: null,
  connectionReport: null,
};

export async function command<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) throw new Error("Open the desktop app to connect and transfer. This browser view has no clipboard or SSH access.");
  return invoke<T>(name, args);
}

export async function subscribe(navigate: (page: string) => void, notice: (message: string, failed: boolean) => void, settingsChanged: () => void, shortcutPressed: (binding: string, openPanel: boolean) => void) {
  if (!isTauri()) return () => {};
  const results = await Promise.allSettled([
    listen<string>("navigate", (event) => navigate(event.payload)),
    listen<string>("operation-notice", (event) => notice(event.payload, true)),
    listen<string>("operation-complete", (event) => notice(event.payload, false)),
    listen<{ binding: string; openPanel: boolean }>("shortcut-pressed", (event) => shortcutPressed(event.payload.binding, event.payload.openPanel)),
    listen("settings-changed", settingsChanged),
  ]);
  const failed = results.find((result) => result.status === "rejected");
  const stops = results.flatMap((result) => result.status === "fulfilled" ? [result.value] : []);
  if (failed?.status === "rejected") { stops.forEach((stop) => stop()); throw failed.reason; }
  return () => stops.forEach((stop) => stop());
}
