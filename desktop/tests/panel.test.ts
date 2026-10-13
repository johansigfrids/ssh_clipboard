import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../src/App.svelte";
import { initialView, type PanelView } from "../src/api";

const bridge = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn(), native: true }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: bridge.invoke, isTauri: () => bridge.native }));
vi.mock("@tauri-apps/api/event", () => ({ listen: bridge.listen }));

let state: PanelView;

beforeEach(() => {
  state = structuredClone(initialView);
  state.platform = "windows";
  state.shortcutStatus = "Registered";
  state.activeShortcut = state.settings.openShortcut;
  bridge.native = true;
  bridge.invoke.mockReset();
  bridge.listen.mockReset().mockResolvedValue(() => {});
  bridge.invoke.mockImplementation(async (name: string, args?: { settings: PanelView["settings"] }) => {
    if (name === "apply_settings") {
      state.settings = args!.settings;
      state.settingsSaved = true;
      state.item = null;
      state.clipboardChecked = false;
      state.connected = false;
    }
    return structuredClone(state);
  });
});
afterEach(cleanup);

describe("quick panel contracts", () => {
  it("corrupt settings show a recovery message and prevent overwriting the file", async () => {
    state.settingsWritable = false;
    state.settingsNotice = "Settings are invalid. Preserve the file and repair it before saving.";
    render(App);
    await screen.findByRole("alert");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.input(screen.getByLabelText("SSH alias or user@host"), { target: { value: "hub" } });
    expect(screen.getByRole("button", { name: "Apply settings" }).hasAttribute("disabled")).toBe(true);
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_panel"]);
  });

  it("startup restores saved settings without connecting or reading the clipboard", async () => {
    state.settings.target = "saved-hub";
    state.settings.showPreview = false;
    state.settingsSaved = true;
    render(App);
    await screen.findByText("saved-hub");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    expect((screen.getByLabelText("SSH alias or user@host") as HTMLInputElement).value).toBe("saved-hub");
    expect(screen.getByText("Saved on this computer")).toBeTruthy();
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_panel"]);
  });

  it("a failed save keeps the draft available without claiming it was applied", async () => {
    const normal = bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation((name, args) => name === "apply_settings" ? Promise.reject("Settings changed in another process. Quit and reopen before editing again.") : normal(name, args));
    render(App);
    await waitFor(() => expect(bridge.invoke).toHaveBeenCalledOnce());
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.input(screen.getByLabelText("SSH alias or user@host"), { target: { value: "new-hub" } });
    await fireEvent.click(screen.getByRole("button", { name: "Apply settings" }));
    await screen.findByRole("alert");
    expect(screen.queryByText("Settings applied.")).toBeNull();
    expect((screen.getByLabelText("SSH alias or user@host") as HTMLInputElement).value).toBe("new-hub");
    expect(state.settings.target).toBe("");
    await fireEvent.click(screen.getByRole("button", { name: "Discard changes" }));
    expect((screen.getByLabelText("SSH alias or user@host") as HTMLInputElement).value).toBe("");
  });

  it("opening the panel reads state without reading, sharing, or replacing the clipboard", async () => {
    render(App);
    await waitFor(() => expect(bridge.invoke).toHaveBeenCalledWith("get_panel", undefined));
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_panel"]);
    expect(screen.getByRole("button", { name: "Copy to this computer" }).hasAttribute("disabled")).toBe(true);
  });

  it("settings apply before the connection can be tested", async () => {
    render(App);
    await waitFor(() => expect(bridge.invoke).toHaveBeenCalledOnce());
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.input(screen.getByLabelText("SSH alias or user@host"), { target: { value: "clipboard-hub" } });
    expect(screen.getByRole("button", { name: "Test saved connection" }).hasAttribute("disabled")).toBe(true);
    await fireEvent.click(screen.getByRole("button", { name: "Apply settings" }));
    await screen.findByText("Settings applied.");
    await fireEvent.click(screen.getByRole("button", { name: "Test saved connection" }));
    await screen.findByText("SSH proxy and clipboard daemon responded.");
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_panel", "apply_settings", "test_connection"]);
  });

  it("an empty refresh explains the server state instead of asking to refresh again", async () => {
    state.settings.target = "hub";
    const normal = bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation((name, args) => {
      if (name === "refresh_item") { state.clipboardChecked = true; state.connected = true; }
      return normal(name, args);
    });
    render(App);
    await screen.findByText("No shared item loaded");
    await fireEvent.click(screen.getByRole("button", { name: "Refresh shared clipboard" }));
    await screen.findByText("Server clipboard is empty");
    expect(screen.queryByText("No shared item loaded")).toBeNull();
    expect(screen.getByRole("button", { name: "Copy to this computer" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("button", { name: "Share current clipboard" }).hasAttribute("disabled")).toBe(false);
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.input(screen.getByLabelText("SSH alias or user@host"), { target: { value: "other-hub" } });
    await fireEvent.click(screen.getByRole("button", { name: "Apply settings" }));
    await screen.findByText("Settings applied.");
    await fireEvent.click(screen.getByRole("button", { name: "Done" }));
    expect(screen.queryByText("Server clipboard is empty")).toBeNull();
    expect(screen.getByText("No shared item loaded")).toBeTruthy();
  });

  it("a failed refresh cannot claim the server clipboard is empty", async () => {
    state.settings.target = "hub";
    const normal = bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation((name, args) => name === "refresh_item" ? Promise.reject("Server unavailable") : normal(name, args));
    render(App);
    await screen.findByText("No shared item loaded");
    await fireEvent.click(screen.getByRole("button", { name: "Refresh shared clipboard" }));
    await screen.findByRole("alert");
    expect(screen.queryByText("Server clipboard is empty")).toBeNull();
    expect(screen.queryByText("Checked the server. No shared item is available.")).toBeNull();
  });

  it("switching the target drops previews from the previous connection", async () => {
    state.settings.target = "old-hub";
    state.item = { kind: "text", label: "Old item", text: "Previous connection contents", imageUrl: null, encodedBytes: 28, createdAt: 1 };
    render(App);
    await screen.findByText("Previous connection contents");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.input(screen.getByLabelText("SSH alias or user@host"), { target: { value: "new-hub" } });
    await fireEvent.click(screen.getByRole("button", { name: "Apply settings" }));
    await screen.findByText("Settings applied.");
    await fireEvent.click(screen.getByRole("button", { name: "Done" }));
    expect(screen.queryByText("Previous connection contents")).toBeNull();
    expect(screen.getByRole("button", { name: "Copy to this computer" }).hasAttribute("disabled")).toBe(true);
  });

  it("a pending share prevents duplicate transfers", async () => {
    state.settings.target = "hub";
    let complete: (value: PanelView) => void = () => {};
    const normal = bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation((name, args) => name === "share_clipboard" ? new Promise<PanelView>((resolve) => { complete = resolve; }) : normal(name, args));
    render(App);
    await waitFor(() => expect(bridge.invoke).toHaveBeenCalledOnce());
    const share = screen.getByRole("button", { name: "Share current clipboard" });
    await fireEvent.click(share);
    expect(share.hasAttribute("disabled")).toBe(true);
    await fireEvent.click(share);
    expect(bridge.invoke.mock.calls.filter(([name]) => name === "share_clipboard")).toHaveLength(1);
    complete(state);
    await screen.findByText("Shared your clipboard with the server.");
  });

  it("Copy targets the displayed snapshot without triggering a remote refresh", async () => {
    state.settings.target = "hub";
    state.item = { kind: "text", label: "Original", text: "Original bytes", imageUrl: null, encodedBytes: 14, createdAt: 1 };
    render(App);
    await screen.findByText("Original bytes");
    await fireEvent.click(screen.getByRole("button", { name: "Copy to this computer" }));
    await screen.findByText("Copied to this computer. You can paste it now.");
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_panel", "copy_item"]);
  });

  it("clipboard write failure never displays a success message", async () => {
    state.settings.target = "hub";
    state.item = { kind: "text", label: "Original", text: "Original bytes", imageUrl: null, encodedBytes: 14, createdAt: 1 };
    const normal = bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation((name, args) => name === "copy_item" ? Promise.reject("Clipboard unavailable") : normal(name, args));
    render(App);
    await screen.findByText("Original bytes");
    await fireEvent.click(screen.getByRole("button", { name: "Copy to this computer" }));
    await screen.findByRole("alert");
    expect(screen.queryByText("Copied to this computer. You can paste it now.")).toBeNull();
  });

  it("an oversized share keeps the previous snapshot available and shows no success", async () => {
    state.settings.target = "hub";
    state.item = { kind: "text", label: "Original", text: "Original bytes", imageUrl: null, encodedBytes: 14, createdAt: 1 };
    const normal = bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation((name, args) => name === "share_clipboard" ? Promise.reject("Image is 12.00 MiB as PNG. Copy a smaller image and try again.") : normal(name, args));
    render(App);
    await screen.findByText("Original bytes");
    await fireEvent.click(screen.getByRole("button", { name: "Share current clipboard" }));
    await screen.findByRole("alert");
    expect(screen.getByText("Original bytes")).toBeTruthy();
    expect(screen.queryByText("Shared your clipboard with the server.")).toBeNull();
    await waitFor(() => expect(screen.getByRole("button", { name: "Copy to this computer" }).hasAttribute("disabled")).toBe(false));
  });

  it("browser-only viewing cannot invoke native clipboard or SSH commands", async () => {
    bridge.native = false;
    render(App);
    await screen.findByRole("alert");
    expect(bridge.invoke).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Share current clipboard" }).hasAttribute("disabled")).toBe(true);
  });
});
