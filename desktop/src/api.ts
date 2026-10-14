import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Endpoint = { protocol: "legacy" } | { protocol: "shelf"; max_item_bytes: number; max_decoded_bytes: number };

export interface Settings {
  profileId: string | null;
  profileName: string;
  target: string;
  port: number | null;
  identityFile: string;
  transport: TransportSettings;
  endpoint: Endpoint;
  openShortcut: string;
  shortcutEnabled: boolean;
  directShortcutsEnabled: boolean;
  directShareShortcut: string | null;
  directCopyShortcut: string | null;
  showPreview: boolean;
  closeAfterCopy: boolean;
  notifications: "panel" | "all" | "errors" | "off";
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
  return { ...value, transport: cloneTransport(value.transport), endpoint: { ...value.endpoint } };
}

export interface ConnectionProfile {
  id: string;
  name: string;
  target: string;
  port: number | null;
  identityFile: string;
  transport: TransportSettings;
  endpoint: Endpoint;
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
  history: null | {
    epoch: string | null;
    selectedId: string | null;
    items: Array<{ id: string; label: string; kind: "text" | "image"; encodedBytes: number; source: string | null; receivedAt: string; expiresAt: string }>;
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
  operationNotice: { id: string; message: string; failed: boolean } | null;
  notificationStatus: { available: boolean; detail: string };
  historyStatus: null | { phase: "connecting" | "live" | "reconnecting" | "needs_action"; detail: string; retryInMs: number | null };
  pendingShare: null | {id:string;canRetry:boolean};
  startupStatus: { available: boolean; enabled: boolean | null; desired: boolean; detail: string };
  updateStatus: UpdateView;
}

export interface UpdateView {
  available: boolean; currentVersion: string; offeredVersion: string | null; notes: string;
  phase: string; detail: string; busy: boolean; downloadedBytes: number; totalBytes: number | null;
}

export interface OperationView {
  generation: string;
  active: null | { id: string; label: string; cancellable: boolean; cancelling: boolean };
}

export interface ConnectionReport {
  healthy: boolean;
  summary: string;
  hint: string | null;
  checks: Array<{ stage: string; label: string; status: "passed" | "failed" | "not_checked"; detail: string }>;
}

export const initialView: PanelView = {
  settings: { profileId: "default", profileName: "Default", target: "", port: null, identityFile: "", transport: defaultTransport(), endpoint: { protocol: "legacy" }, openShortcut: "CommandOrControl+Shift+V", shortcutEnabled: true, directShortcutsEnabled: false, directShareShortcut: null, directCopyShortcut: null, showPreview: true, closeAfterCopy: false, notifications: "panel" },
  profiles: [{ id: "default", name: "Default", target: "", port: null, identityFile: "", transport: defaultTransport(), endpoint: { protocol: "legacy" } }],
  item: null,
  history: null,
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
  operationNotice: null,
  notificationStatus: { available: false, detail: "Waiting for the desktop app" },
  historyStatus: null,
  pendingShare: null,
  startupStatus: { available: false, enabled: null, desired: false, detail: "Waiting for the desktop app" },
  updateStatus: { available: false, currentVersion: "0.0.0", offeredVersion: null, notes: "", phase: "unavailable", detail: "Waiting for the desktop app", busy: false, downloadedBytes: 0, totalBytes: null },
};

export async function command<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) throw new Error("Open the desktop app to connect and transfer. This browser view has no clipboard or SSH access.");
  return invoke<T>(name, args);
}

export async function subscribe(navigate: (page: string) => void, notice: (message: string, failed: boolean) => void, settingsChanged: () => void, shortcutPressed: (binding: string, openPanel: boolean) => void, operationChanged: (state: OperationView) => void, updateChanged: (state: UpdateView) => void) {
  if (!isTauri()) return () => {};
  const results = await Promise.allSettled([
    listen<string>("navigate", (event) => navigate(event.payload)),
    listen<string>("operation-notice", (event) => notice(event.payload, true)),
    listen<{ binding: string; openPanel: boolean }>("shortcut-pressed", (event) => shortcutPressed(event.payload.binding, event.payload.openPanel)),
    listen("settings-changed", settingsChanged),
    listen<OperationView>("operation-state", (event) => operationChanged(event.payload)),
    listen<UpdateView>("update-state", (event) => updateChanged(event.payload)),
  ]);
  const failed = results.find((result) => result.status === "rejected");
  const stops = results.flatMap((result) => result.status === "fulfilled" ? [result.value] : []);
  if (failed?.status === "rejected") { stops.forEach((stop) => stop()); throw failed.reason; }
  return () => stops.forEach((stop) => stop());
}
