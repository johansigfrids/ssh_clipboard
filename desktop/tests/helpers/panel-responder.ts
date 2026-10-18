import type { PanelView, Settings } from "../../src/api";

// A deliberately bounded UI fixture, not an implementation of the Rust backend.
// New commands need an explicit response kind instead of silently succeeding.
export function panelResponder(read: () => PanelView) {
  return async (name: string, args?: Record<string, unknown>) => {
    const state = read();
    switch (name) {
      case "get_operation": return { generation: "0", active: null };
      case "set_shortcut_recording":
      case "hide_panel":
      case "quit_app":
      case "install_update": return undefined;
      case "check_updates":
      case "download_update": return structuredClone(state.updateStatus);
      case "acknowledge_migration_summary":
        state.migrationSummary = null;
        state.settingsRevision = String(BigInt(state.settingsRevision ?? "0") + 1n);
        break;
      case "apply_settings": {
        if (!args?.settings) throw new Error("apply_settings requires settings");
        if (args.startAtLogin !== undefined) {
          state.startupStatus.enabled = args.startAtLogin as boolean;
          state.startupStatus.desired = args.startAtLogin as boolean;
        }
        const settings = args.settings as Settings;
        state.settings = structuredClone({ ...settings, profileId: settings.profileId ?? "new-profile" });
        const profile = { id: state.settings.profileId!, name: settings.profileName, target: settings.target, port: settings.port, identityFile: settings.identityFile, transport: structuredClone(settings.transport), endpoint: { ...settings.endpoint } };
        const index = state.profiles.findIndex(({ id }) => id === profile.id);
        if (index === -1) state.profiles.push(profile); else state.profiles[index] = profile;
        state.settingsSaved = true;
        state.settingsRevision = String(BigInt(state.settingsRevision ?? "0") + 1n);
        state.item = null;
        state.clipboardChecked = false;
        state.connected = false;
        state.connectionReport = null;
        break;
      }
      case "remove_profile": {
        state.profiles = state.profiles.filter(({ id }) => id !== args?.profileId);
        const profile = state.profiles[0];
        state.settings = { ...state.settings, profileId: profile.id, profileName: profile.name, target: profile.target, port: profile.port, identityFile: profile.identityFile, transport: structuredClone(profile.transport), endpoint: { ...profile.endpoint } };
        state.settingsRevision = String(BigInt(state.settingsRevision ?? "0") + 1n);
        state.item = null;
        state.connected = false;
        state.clipboardChecked = false;
        state.connectionReport = null;
        break;
      }
      case "get_panel":
      case "test_notification":
      case "test_connection":
      case "refresh_item":
      case "select_item":
      case "delete_item":
      case "clear_history":
      case "reconnect_history":
      case "share_clipboard":
      case "recover_share":
      case "discard_share":
      case "copy_item": break;
      case "cancel_operation": throw new Error("Set an explicit active-operation response for cancel_operation");
      default: throw new Error(`Unhandled IPC fixture command: ${name}`);
    }
    return structuredClone(state);
  };
}
