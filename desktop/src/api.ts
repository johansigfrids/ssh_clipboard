import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface Settings {
  target: string;
  port: number | null;
  identityFile: string;
  openShortcut: string;
  shortcutEnabled: boolean;
  showPreview: boolean;
  closeAfterCopy: boolean;
}

export interface PanelView {
  settings: Settings;
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
}

export const initialView: PanelView = {
  settings: { target: "", port: null, identityFile: "", openShortcut: "CommandOrControl+Shift+V", shortcutEnabled: true, showPreview: true, closeAfterCopy: false },
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
};

export async function command<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) throw new Error("Open the desktop app to connect and transfer. This browser view has no clipboard or SSH access.");
  return invoke<T>(name, args);
}

export async function subscribe(navigate: (page: string) => void, notice: (message: string) => void, settingsChanged: () => void) {
  if (!isTauri()) return () => {};
  const results = await Promise.allSettled([
    listen<string>("navigate", (event) => navigate(event.payload)),
    listen<string>("operation-notice", (event) => notice(event.payload)),
    listen("settings-changed", settingsChanged),
  ]);
  const failed = results.find((result) => result.status === "rejected");
  const stops = results.flatMap((result) => result.status === "fulfilled" ? [result.value] : []);
  if (failed?.status === "rejected") { stops.forEach((stop) => stop()); throw failed.reason; }
  return () => stops.forEach((stop) => stop());
}
