import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../src/App.svelte";
import { cloneTransport, defaultTransport, initialView, type PanelView } from "../src/api";

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
  bridge.invoke.mockImplementation(async (name: string, args?: { settings?: PanelView["settings"]; profileId?: string }) => {
    if (name === "get_operation") return { generation: "0", active: null };
    if (name === "apply_settings") {
      state.settings = { ...args!.settings!, profileId: args!.settings!.profileId ?? "new-profile" };
      const profile = { id: state.settings.profileId!, name: state.settings.profileName, target: state.settings.target, port: state.settings.port, identityFile: state.settings.identityFile, transport: cloneTransport(state.settings.transport), endpoint: { ...state.settings.endpoint } };
      const index = state.profiles.findIndex(({ id }) => id === profile.id);
      if (index === -1) state.profiles.push(profile); else state.profiles[index] = profile;
      state.settingsSaved = true;
      state.settingsRevision = String(Number(state.settingsRevision ?? "0") + 1);
      state.item = null;
      state.clipboardChecked = false;
      state.connected = false;
      state.connectionReport = null;
    }
    if (name === "remove_profile") {
      state.profiles = state.profiles.filter(({ id }) => id !== args!.profileId);
      const profile = state.profiles[0];
      state.settings = { ...state.settings, profileId: profile.id, profileName: profile.name, target: profile.target, port: profile.port, identityFile: profile.identityFile, transport: cloneTransport(profile.transport), endpoint: { ...profile.endpoint } };
      state.settingsRevision = String(Number(state.settingsRevision ?? "0") + 1);
      state.item = null;
      state.connected = false;
      state.clipboardChecked = false;
      state.connectionReport = null;
    }
    return structuredClone(state);
  });
});
afterEach(cleanup);

describe("quick panel contracts", () => {
  it("update notes are inert and a verified package still needs explicit install confirmation", async () => {
    state.updateStatus={available:true,currentVersion:"1.0.0",offeredVersion:"2.0.0",notes:"<script>never executed</script>",phase:"ready",detail:"Verified",busy:false,downloadedBytes:123,totalBytes:123};
    const normal=bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation((name,args)=>name==="install_update"?undefined:normal(name,args));
    render(App);
    await waitFor(()=>expect(screen.getByRole("button",{name:"Quit"}).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(screen.getByRole("button",{name:"Open settings"}));
    await fireEvent.click(screen.getByRole("button",{name:"Updates"}));
    expect(screen.getByText("<script>never executed</script>").tagName).toBe("PRE");
    expect(document.querySelector("main script")).toBeNull();
    expect(bridge.invoke.mock.calls.some(([name])=>name==="install_update")).toBe(false);
    await fireEvent.click(screen.getByRole("button",{name:"Install update…"}));
    await fireEvent.click(screen.getByRole("button",{name:"Cancel",exact:true}));
    expect(bridge.invoke.mock.calls.some(([name])=>name==="install_update")).toBe(false);
    await fireEvent.click(screen.getByRole("button",{name:"Install update…"}));
    await fireEvent.click(screen.getByRole("button",{name:"Install and restart"}));
    expect(bridge.invoke.mock.calls.find(([name])=>name==="install_update")![1]).toEqual({version:"2.0.0"});
  });

  it("quiet update discovery never downloads or installs and uncertainty blocks installation", async () => {
    render(App);
    await waitFor(()=>expect(screen.getByRole("button",{name:"Quit"}).hasAttribute("disabled")).toBe(false));
    const next={available:true,currentVersion:"1.0.0",offeredVersion:"2.0.0",notes:"Notes",phase:"offered",detail:"Available",busy:false,downloadedBytes:0,totalBytes:null};
    bridge.listen.mock.calls.find(([name])=>name==="update-state")![1]({payload:next});
    await fireEvent.click(screen.getByRole("button",{name:"Open settings"}));
    await fireEvent.click(screen.getByRole("button",{name:"Updates"}));
    expect(screen.getByRole("button",{name:"Download and verify"})).toBeTruthy();
    expect(bridge.invoke.mock.calls.map(([name])=>name)).toEqual(["get_operation","get_panel"]);
    state.pendingShare={id:"uncertain",canRetry:true};
    state.updateStatus={...next,phase:"ready"};
    bridge.listen.mock.calls.find(([name])=>name==="settings-changed")![1]();
    await waitFor(()=>expect(screen.getByRole("button",{name:"Install update…"}).hasAttribute("disabled")).toBe(true));
    expect(screen.getByText("Resolve or discard the uncertain share before installing.")).toBeTruthy();
  });

  it("a current transfer blocks update installation even after confirmation is visible", async () => {
    state.updateStatus={...state.updateStatus,available:true,currentVersion:"1.0.0",offeredVersion:"2.0.0",phase:"ready"};
    render(App);
    await waitFor(()=>expect(screen.getByRole("button",{name:"Quit"}).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(screen.getByRole("button",{name:"Open settings"}));
    await fireEvent.click(screen.getByRole("button",{name:"Updates"}));
    await fireEvent.click(screen.getByRole("button",{name:"Install update…"}));
    bridge.listen.mock.calls.find(([name])=>name==="operation-state")![1]({payload:{generation:"1",active:{id:"1",label:"Sharing",cancellable:true,cancelling:false}}});
    await waitFor(()=>expect(screen.getByRole("button",{name:"Install and restart"}).hasAttribute("disabled")).toBe(true));
    expect(bridge.invoke.mock.calls.some(([name])=>name==="install_update")).toBe(false);
  });

  it("startup shows actual state without activating an imported preference and saves only on an explicit click", async () => {
    state.settingsRevision = "7";
    state.startupStatus = { available: true, enabled: false, desired: true, detail: "Retained preference differs from this app's startup entry." };
    const normal = bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation((name, args) => {
      if (name === "set_start_at_login") {
        state.startupStatus.enabled = args.enabled;
        state.settingsRevision = "8";
      }
      return normal(name, args);
    });
    render(App);
    await waitFor(() => expect(screen.getByRole("button", { name: "Quit" }).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(await screen.findByRole("button", { name: "Open settings" }));
    await fireEvent.click(screen.getByRole("button", { name: "Behavior" }));
    const checkbox = screen.getByRole("checkbox", { name: "Start at login" }) as HTMLInputElement;
    expect(checkbox.checked).toBe(false);
    expect(bridge.invoke.mock.calls.some(([name]) => name === "set_start_at_login")).toBe(false);
    await fireEvent.click(checkbox);
    await screen.findByText("Startup choice saved.");
    expect(bridge.invoke.mock.calls.find(([name]) => name === "set_start_at_login")![1]).toEqual({ enabled: true, expectedRevision: "7" });
    await fireEvent.click(screen.getByRole("checkbox", { name: "Show content previews" }));
    expect(checkbox.disabled).toBe(true);
    await fireEvent.click(screen.getByRole("button", { name: "Apply settings" }));
    expect(bridge.invoke.mock.calls.find(([name]) => name === "apply_settings")![1].expectedRevision).toBe("8");
  });

  it("portable builds cannot request startup registration", async () => {
    render(App);
    await waitFor(() => expect(screen.getByRole("button", { name: "Quit" }).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(await screen.findByRole("button", { name: "Open settings" }));
    await fireEvent.click(screen.getByRole("button", { name: "Behavior" }));
    expect((screen.getByRole("checkbox", { name: "Start at login" }) as HTMLInputElement).disabled).toBe(true);
    expect(bridge.invoke.mock.calls.some(([name]) => name === "set_start_at_login")).toBe(false);
  });

  it("uncertain share recovery checks status without recapturing and displays acceptance", async () => {
    state.settings.target="hub";
    state.settingsRevision="5";
    state.pendingShare={id:"original-operation",canRetry:true};
    const normal=bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation((name,args)=>{
      if(name==="recover_share") {state.pendingShare=null;state.operationNotice={id:"1",failed:false,message:"The earlier share was accepted. Your local clipboard was not read or changed."};}
      return normal(name,args);
    });
    render(App);
    await screen.findByRole("region",{name:"Share recovery"});
    expect(screen.getByRole("button",{name:"Share current clipboard"}).hasAttribute("disabled")).toBe(true);
    expect(bridge.invoke.mock.calls.map(([name])=>name)).toEqual(["get_operation","get_panel"]);
    await fireEvent.click(screen.getByRole("button",{name:"Check share status"}));
    await screen.findByText("The earlier share was accepted. Your local clipboard was not read or changed.");
    expect(bridge.invoke.mock.calls.find(([name])=>name==="recover_share")![1]).toEqual({operationId:"original-operation",retry:false,expectedRevision:"5"});
    expect(screen.queryByRole("region",{name:"Share recovery"})).toBeNull();
    expect(bridge.invoke.mock.calls.some(([name])=>["share_clipboard","copy_item"].includes(name))).toBe(false);
  });

  it("captured retries require an explicit click and expired captures can only be checked or discarded", async () => {
    state.settings.target="hub";
    state.pendingShare={id:"original-operation",canRetry:true};
    render(App);
    await fireEvent.click(await screen.findByRole("button",{name:"Retry captured share"}));
    expect(bridge.invoke.mock.calls.find(([name])=>name==="recover_share")![1]).toEqual({operationId:"original-operation",retry:true,expectedRevision:null});
    state.pendingShare.canRetry=false;
    bridge.listen.mock.calls.find(([name])=>name==="settings-changed")![1]();
    await waitFor(()=>expect(screen.getByRole("button",{name:"Retry captured share"}).hasAttribute("disabled")).toBe(true));
    expect(screen.getByRole("button",{name:"Check share status"}).hasAttribute("disabled")).toBe(false);
    await fireEvent.click(screen.getByRole("button",{name:"Discard recovery data"}));
    expect(bridge.invoke.mock.calls.find(([name])=>name==="discard_share")![1]).toEqual({operationId:"original-operation",expectedRevision:null});
    expect(bridge.invoke.mock.calls.some(([name])=>["share_clipboard","copy_item"].includes(name))).toBe(false);
  });

  it("background history changes update metadata without fetching, sharing, or copying", async () => {
    state.settings.target="hub";
    state.settings.endpoint={protocol:"shelf",max_item_bytes:1024,max_decoded_bytes:1024};
    state.history={epoch:"one",selectedId:null,items:[]};
    state.historyStatus={phase:"live",detail:"History is up to date.",retryInMs:null};
    render(App);
    await screen.findByText("History is up to date.");
    state.history={epoch:"one",selectedId:"new",items:[{id:"new",label:"Other computer's text",kind:"text",encodedBytes:12,source:"Laptop",receivedAt:"100",expiresAt:"200"}]};
    bridge.listen.mock.calls.find(([name])=>name==="settings-changed")![1]();
    await screen.findByRole("button",{name:/Other computer's text.*Laptop/});
    expect(screen.getByRole("button",{name:"Copy to this computer"}).hasAttribute("disabled")).toBe(false);
    expect(bridge.invoke.mock.calls.map(([name])=>name)).toEqual(["get_operation","get_panel","get_panel"]);
  });

  it("history authentication errors offer an explicit reconnect guarded by saved revision", async () => {
    state.settings.target="hub";
    state.settingsRevision="7";
    state.historyStatus={phase:"needs_action",detail:"Unlock your SSH key, then reconnect history.",retryInMs:null};
    render(App);
    await fireEvent.click(await screen.findByRole("button",{name:"Reconnect history"}));
    expect(bridge.invoke.mock.calls.find(([name])=>name==="reconnect_history")![1]).toEqual({expectedRevision:"7"});
    expect(bridge.invoke.mock.calls.some(([name])=>["share_clipboard","copy_item","select_item"].includes(name))).toBe(false);
  });

  it("Cancel remains available during a pending transfer and targets its operation ID", async () => {
    state.settings.target = "hub";
    const normal = bridge.invoke.getMockImplementation()!;
    let reject!: (message: string) => void;
    bridge.invoke.mockImplementation((name, args) => {
      if (name === "refresh_item") return new Promise((_resolve, fail) => { reject = fail; });
      if (name === "cancel_operation") return {generation:"2",active:{id:"41",label:"Refreshing shared clipboard",cancellable:false,cancelling:true}};
      return normal(name,args);
    });
    render(App);
    await screen.findByText("hub");
    await fireEvent.click(screen.getByRole("button",{name:"Refresh shared clipboard"}));
    const changed = bridge.listen.mock.calls.find(([name])=>name==="operation-state")![1];
    changed({payload:{generation:"1",active:{id:"41",label:"Refreshing shared clipboard",cancellable:true,cancelling:false}}});
    await fireEvent.click(await screen.findByRole("button",{name:"Cancel operation"}));
    expect(bridge.invoke.mock.calls.find(([name])=>name==="cancel_operation")![1]).toEqual({id:"41"});
    await waitFor(()=>expect(screen.getByRole("button",{name:"Cancel operation"}).hasAttribute("disabled")).toBe(true));
    changed({payload:{generation:"3",active:null}});
    reject("Cancelled. This computer's clipboard was left unchanged.");
    await screen.findByRole("alert");
    expect(screen.queryByText("Shared clipboard refreshed.")).toBeNull();
    expect(bridge.invoke.mock.calls.some(([name])=>["share_clipboard","copy_item"].includes(name))).toBe(false);
  });

  it("startup exposes a background operation before the panel snapshot is available", async () => {
    const normal = bridge.invoke.getMockImplementation()!;
    let finish!: (view: PanelView) => void;
    bridge.invoke.mockImplementation((name,args)=> {
      if(name==="get_operation") return {generation:"9007199254740994",active:{id:"9007199254740993",label:"Sharing current clipboard",cancellable:true,cancelling:false}};
      if(name==="get_panel") return new Promise<PanelView>((resolve)=>{finish=resolve;});
      if(name==="cancel_operation") return {generation:"9007199254740995",active:{id:"9007199254740993",label:"Sharing current clipboard",cancellable:false,cancelling:true}};
      return normal(name,args);
    });
    render(App);
    const cancel = await screen.findByRole("button",{name:"Cancel operation"});
    expect(cancel.hasAttribute("disabled")).toBe(false);
    await fireEvent.click(cancel);
    expect(bridge.invoke.mock.calls.find(([name])=>name==="cancel_operation")![1]).toEqual({id:"9007199254740993"});
    const changed = bridge.listen.mock.calls.find(([name])=>name==="operation-state")![1];
    changed({payload:{generation:"9007199254740996",active:null}});
    changed({payload:{generation:"9007199254740994",active:{id:"old",label:"Old operation",cancellable:true,cancelling:false}}});
    finish(structuredClone(state));
    await waitFor(()=>expect(screen.queryByRole("button",{name:"Cancel operation"})).toBeNull());
  });

  it("a clipboard write already finishing cannot be cancelled from the panel", async () => {
    render(App);
    await waitFor(()=>expect(screen.getByRole("button",{name:"Open settings"}).hasAttribute("disabled")).toBe(false));
    const changed = bridge.listen.mock.calls.find(([name])=>name==="operation-state")![1];
    changed({payload:{generation:"1",active:{id:"1",label:"Copying shared clipboard",cancellable:false,cancelling:false}}});
    expect((await screen.findByRole("button",{name:"Cancel operation"})).hasAttribute("disabled")).toBe(true);
    expect(bridge.invoke.mock.calls.some(([name])=>name==="cancel_operation")).toBe(false);
  });

  it("history selection previews by stable ID and revision without copying or sharing", async () => {
    state.settings.target = "hub";
    state.settings.endpoint = {protocol:"shelf", max_item_bytes:67108864, max_decoded_bytes:268435456};
    state.settingsRevision = "19";
    state.history = {epoch:"server-epoch", selectedId:"second", items:[{id:"first",label:"Earlier snippet",kind:"text",encodedBytes:12,source:"Laptop",receivedAt:"100",expiresAt:"200"}, {id:"second",label:"Recent snippet",kind:"text",encodedBytes:14,source:"Desktop",receivedAt:"101",expiresAt:"201"}]};
    render(App);
    await screen.findByRole("region", {name:"Temporary clipboard history"});
    await fireEvent.click(screen.getByRole("button", {name:/Earlier snippet/}));
    await screen.findByText("Item selected. Your clipboard is unchanged.");
    expect(bridge.invoke.mock.calls.map(([name])=>name)).toEqual(["get_operation", "get_panel","select_item"]);
    expect(bridge.invoke.mock.calls[2][1]).toEqual({itemId:"first",epoch:"server-epoch",expectedRevision:"19"});
  });

  it("clearing server history requires confirmation and carries the displayed epoch", async () => {
    state.settings.target = "hub";
    state.settings.endpoint = {protocol:"shelf",max_item_bytes:67108864,max_decoded_bytes:268435456};
    state.settingsRevision = "4";
    state.history = {epoch:"old-epoch",selectedId:"item",items:[{id:"item",label:"Text",kind:"text",encodedBytes:1,source:null,receivedAt:"1",expiresAt:"2"}]};
    render(App);
    await screen.findByRole("button", {name:"Clear history…"});
    await fireEvent.click(screen.getByRole("button", {name:"Clear history…"}));
    expect(bridge.invoke.mock.calls.map(([name])=>name)).toEqual(["get_operation", "get_panel"]);
    await fireEvent.click(screen.getByRole("button", {name:"Cancel"}));
    expect(screen.queryByRole("group", {name:"History removal confirmation"})).toBeNull();
    await fireEvent.click(screen.getByRole("button", {name:"Clear history…"}));
    await fireEvent.click(screen.getByRole("button", {name:"Clear history",exact:true}));
    await screen.findByText("History cleared.");
    expect(bridge.invoke.mock.calls[2]).toEqual(["clear_history",{epoch:"old-epoch",expectedRevision:"4"}]);
  });

  it("endpoint changes stay in a draft until Apply and do not probe or transfer", async () => {
    render(App);
    await waitFor(()=>expect(screen.getByRole("button",{name:"Open settings"}).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(screen.getByRole("button",{name:"Open settings"}));
    await fireEvent.change(screen.getByLabelText("Server features"),{target:{value:"shelf"}});
    expect(state.settings.endpoint.protocol).toBe("legacy");
    expect(bridge.invoke.mock.calls.map(([name])=>name)).toEqual(["get_operation", "get_panel"]);
    await fireEvent.click(screen.getByRole("button",{name:"Apply settings"}));
    await screen.findByText("Settings applied.");
    expect(state.settings.endpoint).toEqual({protocol:"shelf",max_item_bytes:67108864,max_decoded_bytes:268435456});
  });

  it("a history reload cancels an open removal confirmation instead of retargeting it", async () => {
    state.settings.target="hub";
    state.settings.endpoint={protocol:"shelf",max_item_bytes:67108864,max_decoded_bytes:268435456};
    state.history={epoch:"old",selectedId:"item",items:[{id:"item",label:"Text",kind:"text",encodedBytes:1,source:null,receivedAt:"1",expiresAt:"2"}]};
    render(App);
    await screen.findByRole("button",{name:"Clear history…"});
    await fireEvent.click(screen.getByRole("button",{name:"Clear history…"}));
    expect(screen.getByRole("group",{name:"History removal confirmation"})).toBeTruthy();
    state.history.epoch="new";
    bridge.listen.mock.calls.find(([name])=>name==="settings-changed")![1]({payload:null});
    await waitFor(()=>expect(screen.queryByRole("group",{name:"History removal confirmation"})).toBeNull());
    expect(bridge.invoke.mock.calls.some(([name])=>name==="clear_history")).toBe(false);
  });
  it("notification feedback is a saved draft and never sends a notification while editing", async () => {
    state.notificationStatus = { available: false, detail: "This build shows results in the panel." };
    render(App);
    await waitFor(() => expect(screen.getByRole("button", { name: "Open settings" }).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.click(screen.getByRole("button", { name: "Behavior" }));
    expect(screen.getByRole("button", { name: "Test notification" }).hasAttribute("disabled")).toBe(true);
    await fireEvent.change(screen.getByLabelText("Direct action feedback"), { target: { value: "off" } });
    expect(state.settings.notifications).toBe("panel");
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel"]);
    await fireEvent.click(screen.getByRole("button", { name: "Apply settings" }));
    await screen.findByText("Settings applied.");
    expect(state.settings.notifications).toBe("off");
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel", "apply_settings"]);
  });

  it("testing notifications is explicit, has no clipboard commands, and retains the feedback draft", async () => {
    state.notificationStatus = { available: true, detail: "System settings control notification delivery." };
    const normal = bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation((name, args) => {
      if (name === "test_notification") state.notificationStatus.detail = "Test submitted; delivery is not verified.";
      return normal(name, args);
    });
    render(App);
    await waitFor(() => expect(screen.getByRole("button", { name: "Open settings" }).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.click(screen.getByRole("button", { name: "Behavior" }));
    await fireEvent.change(screen.getByLabelText("Direct action feedback"), { target: { value: "errors" } });
    await fireEvent.click(screen.getByRole("button", { name: "Test notification" }));
    await screen.findByText("Test notification submitted to the system.");
    expect((screen.getByLabelText("Direct action feedback") as HTMLSelectElement).value).toBe("errors");
    expect(state.settings.notifications).toBe("panel");
    expect(screen.getByText("Test submitted; delivery is not verified.")).toBeTruthy();
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel", "test_notification"]);
  });

  it("quiet-mode results survive reopening and repeated outcomes replace older UI feedback", async () => {
    state.operationNotice = { id: "1", message: "Shared clipboard copied to this computer.", failed: false };
    render(App);
    expect(await screen.findByRole("status")).toHaveProperty("textContent", "Shared clipboard copied to this computer.");
    bridge.listen.mock.calls.find(([name]) => name === "operation-notice")![1]({ payload: "Older UI feedback" });
    await screen.findByText("Older UI feedback");
    state.operationNotice = { id: "2", message: "Shared clipboard copied to this computer.", failed: false };
    bridge.listen.mock.calls.find(([name]) => name === "settings-changed")![1]();
    await waitFor(() => expect(screen.getByRole("status")).toHaveProperty("textContent", "Shared clipboard copied to this computer."));
    expect(screen.queryByText("Older UI feedback")).toBeNull();
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel", "get_panel"]);
  });

  it("direct shortcuts are opt-in, independently recorded drafts with no clipboard commands", async () => {
    state.settings.target = "hub";
    state.settings.directShareShortcut = "Control+Alt+KeyS";
    render(App);
    await screen.findByText("hub");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.click(screen.getByRole("button", { name: "Shortcuts" }));
    const share = screen.getByLabelText("Share current clipboard") as HTMLButtonElement;
    expect(share.disabled).toBe(true);
    expect((screen.getByLabelText("Enable direct shortcuts") as HTMLInputElement).checked).toBe(false);
    await fireEvent.click(screen.getByLabelText("Enable direct shortcuts"));
    await fireEvent.click(share);
    await screen.findByText("Press a shortcut · Esc to cancel");
    // Registered bindings may be consumed by the OS: the native event also feeds the recorder.
    bridge.listen.mock.calls.find(([name]) => name === "shortcut-pressed")![1]({ payload: { binding: "control+alt+KeyC", openPanel: false } });
    await waitFor(() => expect(share.textContent).toContain("Ctrl + Alt + C"));
    await fireEvent.click(screen.getByLabelText("Copy shared clipboard"));
    await screen.findByText("Press a shortcut · Esc to cancel");
    await fireEvent.keyDown(window, { key: "v", code: "KeyV", ctrlKey: true, altKey: true });
    await fireEvent.click(screen.getByRole("button", { name: "Apply settings" }));
    await screen.findByText("Settings applied.");
    const args = bridge.invoke.mock.calls.find(([name]) => name === "apply_settings")![1];
    expect(args.settings.directShortcutsEnabled).toBe(true);
    expect(args.settings.directShareShortcut).toBe("control+alt+KeyC");
    expect(args.settings.directCopyShortcut).toBe("Control+Alt+KeyV");
    expect(args.settings.openShortcut).toBe(initialView.settings.openShortcut);
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel", "set_shortcut_recording", "set_shortcut_recording", "set_shortcut_recording", "set_shortcut_recording", "apply_settings"]);
    expect(bridge.invoke.mock.calls.filter(([name]) => name === "set_shortcut_recording").map(([, args]) => args.active)).toEqual([true, false, true, false]);
  });

  it("recording can be cancelled and retained direct bindings can be disabled without forgetting them", async () => {
    state.settings.directShortcutsEnabled = true;
    state.settings.directCopyShortcut = "Control+Alt+KeyV";
    render(App);
    await waitFor(() => expect(screen.getByRole("button", { name: "Open settings" }).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.click(screen.getByRole("button", { name: "Shortcuts" }));
    await fireEvent.click(screen.getByLabelText("Copy shared clipboard"));
    await screen.findByText("Press a shortcut · Esc to cancel");
    await fireEvent.keyDown(window, { key: "x", code: "KeyX", ctrlKey: true, repeat: true });
    expect(screen.getByText("Press a shortcut · Esc to cancel")).toBeTruthy();
    await fireEvent.keyDown(window, { key: "Escape" });
    await fireEvent.click(screen.getByLabelText("Enable direct shortcuts"));
    await fireEvent.click(screen.getByRole("button", { name: "Apply settings" }));
    await screen.findByText("Settings applied.");
    const args = bridge.invoke.mock.calls.find(([name]) => name === "apply_settings")![1];
    expect(args.settings.directShortcutsEnabled).toBe(false);
    expect(args.settings.directCopyShortcut).toBe("Control+Alt+KeyV");
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel", "set_shortcut_recording", "set_shortcut_recording", "apply_settings"]);
  });

  it("losing focus cancels a pending recorder and clears native suppression in order", async () => {
    const normal = bridge.invoke.getMockImplementation()!;
    let acknowledge!: () => void;
    bridge.invoke.mockImplementation((name, args) => name === "set_shortcut_recording" && args.active
      ? new Promise<void>((resolve) => { acknowledge = resolve; }) : normal(name, args));
    render(App);
    await waitFor(() => expect(screen.getByRole("button", { name: "Open settings" }).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.click(screen.getByRole("button", { name: "Shortcuts" }));
    await fireEvent.click(screen.getByLabelText("Open panel"));
    await waitFor(() => expect(acknowledge).toBeTypeOf("function"));
    await fireEvent.blur(window);
    acknowledge();
    await waitFor(() => expect(bridge.invoke.mock.calls.filter(([name]) => name === "set_shortcut_recording").map(([, args]) => args.active)).toEqual([true, false]));
    expect(screen.queryByText("Press a shortcut · Esc to cancel")).toBeNull();
    expect(bridge.invoke.mock.calls.every(([name]) => ["get_operation", "get_panel", "set_shortcut_recording"].includes(name))).toBe(true);
  });

  it("direct action completion is shown as success and foreground shortcuts do not transfer", async () => {
    render(App);
    await waitFor(() => expect(screen.getByRole("button", { name: "Open settings" }).hasAttribute("disabled")).toBe(false));
    const press = bridge.listen.mock.calls.find(([name]) => name === "shortcut-pressed")![1];
    press({ payload: { binding: "Control+Alt+KeyC", openPanel: false } });
    expect(await screen.findByRole("status")).toHaveProperty("textContent", "Direct shortcuts work while this app is in the background. Use the panel buttons here.");
    state.operationNotice = { id: "1", message: "Current clipboard shared.", failed: false };
    bridge.listen.mock.calls.find(([name]) => name === "settings-changed")![1]();
    await waitFor(() => expect(screen.getByRole("status")).toHaveProperty("textContent", "Current clipboard shared."));
    expect(screen.queryByRole("alert")).toBeNull();
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel", "get_panel"]);
  });

  it("connection diagnostics distinguish verified SSH/proxy from a missing daemon", async () => {
    state.settings.target = "hub";
    const normal = bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation((name, args) => {
      if (name === "test_connection") {
        state.connectionReport = {
          healthy: false, summary: "The clipboard daemon is unavailable.", hint: "Check the daemon service on the server.",
          checks: [
            { stage: "ssh_connection", label: "SSH connection", status: "passed", detail: "A clipboard reply arrived over SSH." },
            { stage: "proxy", label: "Remote clipboard proxy", status: "passed", detail: "A clipboard frame was received." },
            { stage: "daemon", label: "Clipboard daemon", status: "failed", detail: "The proxy could not reach the daemon." },
            { stage: "request", label: "Metadata check", status: "not_checked", detail: "Not verified by this check." },
          ],
        };
        return Promise.reject("The clipboard daemon is unavailable.");
      }
      return normal(name, args);
    });
    render(App);
    await screen.findByText("hub");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.click(screen.getByRole("button", { name: "Test saved connection" }));
    await screen.findByRole("alert");
    expect(await screen.findByRole("region", { name: "Saved connection diagnostics" })).toBeTruthy();
    expect(screen.getByText("SSH connection · Verified")).toBeTruthy();
    expect(screen.getByText("Clipboard daemon · Failed")).toBeTruthy();
    expect(screen.getByText("Metadata check · Not verified")).toBeTruthy();
    expect(screen.getByText("Check the daemon service on the server.")).toBeTruthy();
    expect(screen.queryByText("SSH proxy and clipboard daemon responded.")).toBeNull();
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel", "test_connection", "get_panel"]);
    await fireEvent.input(screen.getByLabelText("SSH alias or user@host"), { target: { value: "other-hub" } });
    expect(screen.queryByRole("region", { name: "Saved connection diagnostics" })).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "Apply settings" }));
    await screen.findByText("Settings applied.");
    expect(screen.queryByRole("region", { name: "Saved connection diagnostics" })).toBeNull();
  });

  it("a successful diagnostic remains metadata-only and never enables Copy", async () => {
    state.settings.target = "hub";
    const normal = bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation((name, args) => {
      if (name === "test_connection") {
        state.connected = true;
        state.connectionReport = { healthy: true, summary: "Verified", hint: null, checks: [
          { stage: "protocol", label: "Clipboard protocol", status: "passed", detail: "A correlated reply was received." },
        ] };
      }
      return normal(name, args);
    });
    render(App);
    await screen.findByText("hub");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.click(screen.getByRole("button", { name: "Test saved connection" }));
    await screen.findByText("Connection verified");
    await fireEvent.click(screen.getByRole("button", { name: "Done" }));
    expect(screen.getByRole("button", { name: "Copy to this computer" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByText("No shared item loaded")).toBeTruthy();
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel", "test_connection"]);
  });

  function savedProfiles() {
    state.settings.target = "shared-hub";
    state.settings.identityFile = "first/key";
    state.settingsSaved = true;
    state.settingsRevision = "4";
    state.profiles = [
      { id: "default", name: "Default", target: "shared-hub", port: null, identityFile: "first/key", transport: defaultTransport() },
      { id: "other", name: "Other account", target: "shared-hub", port: 2222, identityFile: "other/key", transport: { ...defaultTransport(), timeoutMs: "15000", sshOptions: ["ServerAliveInterval=20"] } },
    ];
  }

  it("choosing a profile is a draft until Apply and does not fetch or copy an item", async () => {
    savedProfiles();
    state.item = { kind: "text", label: "Old item", text: "Old snapshot", imageUrl: null, encodedBytes: 12, createdAt: 1 };
    render(App);
    await screen.findByText("Old snapshot");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.change(screen.getByLabelText("Connection"), { target: { value: "other" } });
    expect((screen.getByLabelText("Connection name") as HTMLInputElement).value).toBe("Other account");
    expect((screen.getByLabelText(/Identity file/) as HTMLInputElement).value).toBe("other/key");
    expect(screen.getByRole("button", { name: "Test saved connection" }).hasAttribute("disabled")).toBe(true);
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel"]);
    await fireEvent.click(screen.getByRole("button", { name: "Apply settings" }));
    await screen.findByText("Settings applied.");
    const args = bridge.invoke.mock.calls.find(([name]) => name === "apply_settings")![1];
    expect(args.settings.profileId).toBe("other");
    expect(args.settings.port).toBe(2222);
    expect(args.settings.transport.timeoutMs).toBe("15000");
    expect(args.settings.transport.sshOptions).toEqual(["ServerAliveInterval=20"]);
    expect(args.expectedRevision).toBe("4");
    await fireEvent.click(screen.getByRole("button", { name: "Done" }));
    expect(screen.getByText("Other account")).toBeTruthy();
    expect(screen.queryByText("Old snapshot")).toBeNull();
    expect(screen.getByRole("button", { name: "Copy to this computer" }).hasAttribute("disabled")).toBe(true);
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel", "apply_settings"]);
  });

  it("a new connection starts without another connection's key and can be cancelled before saving", async () => {
    savedProfiles();
    state.settings.transport = { ...defaultTransport(), sshOptions: ["ProxyJump=first-gateway"], timeoutMs: "9000", sshBinary: "custom/ssh" };
    render(App);
    await screen.findByText("shared-hub");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.click(screen.getByRole("button", { name: "Add connection" }));
    expect((screen.getByLabelText("SSH alias or user@host") as HTMLInputElement).value).toBe("");
    expect((screen.getByLabelText(/Identity file/) as HTMLInputElement).value).toBe("");
    expect((screen.getByLabelText("Timeout (milliseconds)") as HTMLInputElement).value).toBe("7000");
    expect((screen.getByLabelText(/SSH executable/) as HTMLInputElement).value).toBe("");
    expect((screen.getByLabelText(/SSH options/) as HTMLTextAreaElement).value).toBe("");
    await fireEvent.click(screen.getByRole("button", { name: "Discard changes" }));
    expect((screen.getByLabelText(/Identity file/) as HTMLInputElement).value).toBe("first/key");
    expect((screen.getByLabelText("Timeout (milliseconds)") as HTMLInputElement).value).toBe("9000");
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel"]);
    await fireEvent.click(screen.getByRole("button", { name: "Add connection" }));
    await fireEvent.input(screen.getByLabelText("Connection name"), { target: { value: "New server" } });
    await fireEvent.input(screen.getByLabelText("SSH alias or user@host"), { target: { value: "new-hub" } });
    await fireEvent.click(screen.getByRole("button", { name: "Apply settings" }));
    await screen.findByText("Settings applied.");
    expect(bridge.invoke.mock.calls.find(([name]) => name === "apply_settings")![1].settings.profileId).toBeNull();
    expect(state.profiles).toHaveLength(3);
    expect((screen.getByLabelText("Connection") as HTMLSelectElement).value).toBe("new-profile");
  });

  it("removal requires confirmation, carries the revision, and clears the old preview", async () => {
    savedProfiles();
    render(App);
    await screen.findByText("shared-hub");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.click(screen.getByRole("button", { name: "Remove…" }));
    expect(screen.getByRole("group", { name: "Remove connection confirmation" })).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Cancel", exact: true }));
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel"]);
    await fireEvent.click(screen.getByRole("button", { name: "Remove…" }));
    await fireEvent.click(screen.getByRole("button", { name: "Remove connection", exact: true }));
    await screen.findByText("Connection removed. Another saved connection is now active.");
    expect(bridge.invoke.mock.calls.find(([name]) => name === "remove_profile")![1]).toEqual({ profileId: "default", expectedRevision: "4" });
    expect((screen.getByLabelText("Connection name") as HTMLInputElement).value).toBe("Other account");
    expect(screen.getByRole("button", { name: "Remove…" }).hasAttribute("disabled")).toBe(true);
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel", "remove_profile"]);
  });

  it("a CLI removal invalidates a profile draft and Discard returns to the surviving profile", async () => {
    savedProfiles();
    render(App);
    await screen.findByText("shared-hub");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.change(screen.getByLabelText("Connection"), { target: { value: "other" } });
    state.profiles.pop();
    state.settingsRevision = "5";
    bridge.listen.mock.calls.find(([name]) => name === "settings-changed")![1]();
    await screen.findByText("Saved settings changed. Discard your draft to load them before editing again.");
    expect((screen.getByLabelText("Connection name") as HTMLInputElement).value).toBe("Other account");
    expect(screen.getByRole("button", { name: "Apply settings" }).hasAttribute("disabled")).toBe(true);
    await fireEvent.click(screen.getByRole("button", { name: "Discard changes" }));
    expect((screen.getByLabelText("Connection name") as HTMLInputElement).value).toBe("Default");
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel", "get_panel"]);
  });

  it("a failed removal keeps the existing connection and reports no success", async () => {
    savedProfiles();
    const normal = bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation((name, args) => name === "remove_profile" ? Promise.reject("Settings changed in another process.") : normal(name, args));
    render(App);
    await screen.findByText("shared-hub");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.click(screen.getByRole("button", { name: "Remove…" }));
    await fireEvent.click(screen.getByRole("button", { name: "Remove connection", exact: true }));
    await screen.findByRole("alert");
    expect(screen.queryByText("Connection removed. Another saved connection is now active.")).toBeNull();
    expect(state.profiles).toHaveLength(2);
    expect((screen.getByLabelText("Connection name") as HTMLInputElement).value).toBe("Default");
  });

  it("advanced fields edit a separate draft and preserve option order while ignoring blank lines", async () => {
    savedProfiles();
    render(App);
    await screen.findByText("shared-hub");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    const options = screen.getByLabelText(/SSH options/) as HTMLTextAreaElement;
    const text = "BatchMode=no\n\nServerAliveInterval=20\n";
    await fireEvent.input(options, { target: { value: text } });
    await fireEvent.input(screen.getByLabelText("Timeout (milliseconds)"), { target: { value: "9000" } });
    expect(options.value).toBe(text);
    expect(state.settings.transport.timeoutMs).toBe("7000");
    expect(state.settings.transport.sshOptions).toEqual([]);
    expect(screen.getByRole("button", { name: "Apply settings" }).hasAttribute("disabled")).toBe(false);
    expect(screen.getByRole("button", { name: "Test saved connection" }).hasAttribute("disabled")).toBe(true);
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel"]);
    await fireEvent.click(screen.getByRole("button", { name: "Apply settings" }));
    await screen.findByText("Settings applied.");
    const transport = bridge.invoke.mock.calls.find(([name]) => name === "apply_settings")![1].settings.transport;
    expect(transport.timeoutMs).toBe("9000");
    expect(transport.sshOptions).toEqual(["BatchMode=no", "ServerAliveInterval=20"]);
  });

  it("advanced defaults remain a cancellable draft and strict framing retains its budget", async () => {
    savedProfiles();
    state.settings.transport = { ...defaultTransport(), timeoutMs: "12000", resyncFrames: false, resyncMaxBytes: "4096", sshOptions: ["ServerAliveInterval=30"] };
    render(App);
    await screen.findByText("shared-hub");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    const budget = screen.getByLabelText("Framing recovery budget (bytes)") as HTMLInputElement;
    expect(budget.value).toBe("4096");
    expect(budget.disabled).toBe(true);
    await fireEvent.click(screen.getByLabelText("Recover from text before protocol frames"));
    expect(budget.disabled).toBe(false);
    expect(budget.value).toBe("4096");
    await fireEvent.click(screen.getByRole("button", { name: "Restore advanced defaults" }));
    expect((screen.getByLabelText("Timeout (milliseconds)") as HTMLInputElement).value).toBe("7000");
    expect(budget.value).toBe("8192");
    await fireEvent.click(screen.getByRole("button", { name: "Discard changes" }));
    expect((screen.getByLabelText("Timeout (milliseconds)") as HTMLInputElement).value).toBe("12000");
    expect(budget.value).toBe("4096");
    expect((screen.getByLabelText(/SSH options/) as HTMLTextAreaElement).value).toBe("ServerAliveInterval=30");
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel"]);
  });

  it("a preference-only edit preserves advanced integers larger than JavaScript's exact range", async () => {
    savedProfiles();
    state.settings.transport.timeoutMs = "18446744073709551615";
    state.settings.transport.maxSize = "9007199254740993";
    render(App);
    await screen.findByText("shared-hub");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.click(screen.getByRole("button", { name: "Behavior", exact: true }));
    await fireEvent.click(screen.getByLabelText("Show content previews"));
    await fireEvent.click(screen.getByRole("button", { name: "Apply settings" }));
    await screen.findByText("Settings applied.");
    const transport = bridge.invoke.mock.calls.find(([name]) => name === "apply_settings")![1].settings.transport;
    expect(transport.timeoutMs).toBe("18446744073709551615");
    expect(transport.maxSize).toBe("9007199254740993");
  });

  it("external edits preserve an advanced draft until Discard loads the latest policy", async () => {
    savedProfiles();
    render(App);
    await screen.findByText("shared-hub");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.input(screen.getByLabelText("Timeout (milliseconds)"), { target: { value: "12000" } });
    await fireEvent.input(screen.getByLabelText(/SSH options/), { target: { value: "ServerAliveInterval=10\n" } });
    state.settings.transport.timeoutMs = "15000";
    state.settings.transport.sshOptions = ["ServerAliveInterval=30"];
    state.settingsRevision = "5";
    bridge.listen.mock.calls.find(([name]) => name === "settings-changed")![1]();
    await screen.findByText("Saved settings changed. Discard your draft to load them before editing again.");
    expect((screen.getByLabelText("Timeout (milliseconds)") as HTMLInputElement).value).toBe("12000");
    expect((screen.getByLabelText(/SSH options/) as HTMLTextAreaElement).value).toBe("ServerAliveInterval=10\n");
    expect(screen.getByRole("button", { name: "Apply settings" }).hasAttribute("disabled")).toBe(true);
    await fireEvent.click(screen.getByRole("button", { name: "Discard changes" }));
    expect((screen.getByLabelText("Timeout (milliseconds)") as HTMLInputElement).value).toBe("15000");
    expect((screen.getByLabelText(/SSH options/) as HTMLTextAreaElement).value).toBe("ServerAliveInterval=30");
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel", "get_panel"]);
  });

  it("an imported configuration explains preserved setup choices while remaining editable", async () => {
    state.settings.target = "legacy-hub";
    state.settingsSaved = true;
    state.settingsRevision = "1";
    state.migrationSummary = "Imported your existing connection and preferences and saved a backup.";
    render(App);
    await screen.findByText("legacy-hub");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    expect(screen.getByText(state.migrationSummary)).toBeTruthy();
    await fireEvent.input(screen.getByLabelText("SSH alias or user@host"), { target: { value: "edited-hub" } });
    expect(screen.getByRole("button", { name: "Apply settings" }).hasAttribute("disabled")).toBe(false);
  });

  it("a failed subscription removes the listeners already installed and still loads the panel", async () => {
    const stop = vi.fn();
    bridge.listen.mockImplementation((name) => name === "settings-changed" ? Promise.reject("Unavailable") : Promise.resolve(stop));
    render(App);
    await screen.findByText("Could not listen for tray or settings changes. Reopen through the launcher.");
    await waitFor(() => expect(screen.getByRole("button", { name: "Quit" }).hasAttribute("disabled")).toBe(false));
    expect(stop).toHaveBeenCalledTimes(5);
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel"]);
  });

  it("a settings change during the initial snapshot is delivered after startup", async () => {
    state.settings.target = "first-hub";
    state.settingsRevision = "1";
    const firstReply = structuredClone(state);
    let finish!: (view: PanelView) => void;
    const normal = bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementationOnce(() => ({generation:"0",active:null}));
    bridge.invoke.mockImplementationOnce(() => new Promise<PanelView>((resolve) => { finish = resolve; }));
    render(App);
    await waitFor(() => expect(bridge.invoke).toHaveBeenCalledTimes(2));
    state.settings.target = "second-hub";
    state.settingsRevision = "2";
    bridge.listen.mock.calls.find(([name]) => name === "settings-changed")![1]();
    expect(bridge.invoke).toHaveBeenCalledTimes(2);
    bridge.invoke.mockImplementation(normal);
    finish(firstReply);
    await screen.findByText("second-hub");
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel", "get_panel"]);
  });

  it("settings notifications during a transfer reload after its reply without starting another transfer", async () => {
    state.settings.target = "first-hub";
    state.settingsRevision = "1";
    const reply = structuredClone(state);
    let finish!: (view: PanelView) => void;
    const normal = bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation((name, args) => name === "refresh_item" ? new Promise<PanelView>((resolve) => { finish = resolve; }) : normal(name, args));
    render(App);
    await screen.findByText("first-hub");
    await fireEvent.click(screen.getByRole("button", { name: "Refresh shared clipboard" }));
    state.settings.target = "second-hub";
    state.settingsRevision = "2";
    bridge.listen.mock.calls.find(([name]) => name === "settings-changed")![1]();
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel", "refresh_item"]);
    finish(reply);
    await screen.findByText("second-hub");
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel", "refresh_item", "get_panel"]);
  });

  it("transfer clicks carry the displayed settings revision and recover a stale clean form", async () => {
    state.settings.target = "first-hub";
    state.settingsRevision = "1";
    const normal = bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation((name, args) => name === "share_clipboard" ? Promise.reject("Saved settings changed. Reload the panel before continuing.") : normal(name, args));
    render(App);
    await screen.findByText("first-hub");
    state.settings.target = "second-hub";
    state.settingsRevision = "2";
    await fireEvent.click(screen.getByRole("button", { name: "Share current clipboard" }));
    await screen.findByRole("alert");
    await screen.findByText("second-hub");
    expect(bridge.invoke.mock.calls.find(([name]) => name === "share_clipboard")![1].expectedRevision).toBe("1");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    expect((screen.getByLabelText("SSH alias or user@host") as HTMLInputElement).value).toBe("second-hub");
    expect(screen.getByRole("button", { name: "Discard changes" }).hasAttribute("disabled")).toBe(true);
  });

  it("external settings reload without a transfer and preserve an edited draft until discarded", async () => {
    state.settings.target = "first-hub";
    state.settingsRevision = "1";
    render(App);
    await screen.findByText("first-hub");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.input(screen.getByLabelText("SSH alias or user@host"), { target: { value: "draft-hub" } });
    state.settings.target = "cli-hub";
    state.settingsRevision = "2";
    const changed = bridge.listen.mock.calls.find(([name]) => name === "settings-changed")![1];
    changed();
    await screen.findByText("Saved settings changed. Discard your draft to load them before editing again.");
    expect((screen.getByLabelText("SSH alias or user@host") as HTMLInputElement).value).toBe("draft-hub");
    expect(screen.getByRole("button", { name: "Apply settings" }).hasAttribute("disabled")).toBe(true);
    await fireEvent.click(screen.getByRole("button", { name: "Discard changes" }));
    expect((screen.getByLabelText("SSH alias or user@host") as HTMLInputElement).value).toBe("cli-hub");
    await fireEvent.input(screen.getByLabelText("SSH alias or user@host"), { target: { value: "edited-hub" } });
    await fireEvent.click(screen.getByRole("button", { name: "Apply settings" }));
    await screen.findByText("Settings applied.");
    expect(bridge.invoke.mock.calls.find(([name]) => name === "apply_settings")![1].expectedRevision).toBe("2");
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel", "get_panel", "apply_settings"]);
  });

  it("external settings update a clean form while leaving clipboard actions explicit", async () => {
    render(App);
    await waitFor(() => expect(screen.getByRole("button", { name: "Quit" }).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    state.settings.target = "cli-hub";
    state.settingsRevision = "1";
    bridge.listen.mock.calls.find(([name]) => name === "settings-changed")![1]();
    await waitFor(() => expect((screen.getByLabelText("SSH alias or user@host") as HTMLInputElement).value).toBe("cli-hub"));
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel", "get_panel"]);
  });

  it("corrupt settings show a recovery message and prevent overwriting the file", async () => {
    state.settingsWritable = false;
    state.settingsNotice = "Settings are invalid. Preserve the file and repair it before saving.";
    render(App);
    await screen.findByRole("alert");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.input(screen.getByLabelText("SSH alias or user@host"), { target: { value: "hub" } });
    expect(screen.getByRole("button", { name: "Apply settings" }).hasAttribute("disabled")).toBe(true);
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel"]);
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
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel"]);
  });

  it("a failed save keeps the draft available without claiming it was applied", async () => {
    const normal = bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation((name, args) => name === "apply_settings" ? Promise.reject("Settings changed in another process. Quit and reopen before editing again.") : normal(name, args));
    render(App);
    await waitFor(() => expect(bridge.invoke).toHaveBeenCalledTimes(2));
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
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel"]);
    expect(screen.getByRole("button", { name: "Copy to this computer" }).hasAttribute("disabled")).toBe(true);
  });

  it("settings apply before the connection can be tested", async () => {
    render(App);
    await waitFor(() => expect(bridge.invoke).toHaveBeenCalledTimes(2));
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.input(screen.getByLabelText("SSH alias or user@host"), { target: { value: "clipboard-hub" } });
    expect(screen.getByRole("button", { name: "Test saved connection" }).hasAttribute("disabled")).toBe(true);
    await fireEvent.click(screen.getByRole("button", { name: "Apply settings" }));
    await screen.findByText("Settings applied.");
    await fireEvent.click(screen.getByRole("button", { name: "Test saved connection" }));
    await screen.findByText("SSH proxy and clipboard daemon responded.");
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel", "apply_settings", "test_connection"]);
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
    await waitFor(() => expect(bridge.invoke).toHaveBeenCalledTimes(2));
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
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel", "copy_item"]);
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
