import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../src/App.svelte";
import { panelResponder } from "./helpers/panel-responder";
import { ipc } from "./fixtures/ipc";
import requests from "./fixtures/ipc-requests.json";
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
  bridge.invoke.mockImplementation(panelResponder(() => state));
});
afterEach(() => { cleanup(); vi.useRealTimers(); });

describe("quick panel contracts", () => {
  it("unknown IPC commands fail instead of receiving a successful panel snapshot", async () => {
    await expect(panelResponder(() => state)("apply_setting", { settings: state.settings })).rejects.toThrow("Unhandled IPC fixture command");
  });

  it("renders backend-produced history/recovery fields and sends the exact string revision", async () => {
    state = structuredClone(ipc.shelf);
    render(App);
    await screen.findByText("Contract history item");
    expect(screen.getByText("Synthetic retained result")).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Check share status" }));
    expect(bridge.invoke).toHaveBeenCalledWith("recover_share", {
      operationId: state.pendingShare!.id, retry: false, expectedRevision: "18446744073709551615",
    });
    await fireEvent.click(screen.getByRole("button", { name: /Contract history item.*Contract laptop/ }));
    await waitFor(() => expect(bridge.invoke).toHaveBeenCalledWith("select_item", requests.select_item));
  });

  it("explicit failures focus their persistent message and Dismiss restores the settings tab", async () => {
    const normal=bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation((name,args)=>name==="test_connection"?Promise.reject(new Error("Synthetic connection failure")):normal(name,args));
    state.settings.target="hub";
    render(App);await screen.findByText("hub");
    await fireEvent.click(screen.getByRole("button",{name:"Open settings"}));
    await fireEvent.click(screen.getByRole("button",{name:"Test saved connection"}));
    await waitFor(()=>expect(document.activeElement).toBe(screen.getByRole("alert")));
    expect(screen.getByRole("alert").textContent).toContain("Synthetic connection failure");
    await fireEvent.click(screen.getByRole("button",{name:"Dismiss message"}));
    await waitFor(()=>expect(document.activeElement).toBe(screen.getByRole("tab",{name:"Connection",exact:true})));
  });

  it("a replaced update offer removes confirmation and recovers focus without installing", async () => {
    state.updateStatus={...state.updateStatus,available:true,currentVersion:"1.0.0",offeredVersion:"2.0.0",phase:"ready"};
    render(App);await waitFor(()=>expect(screen.getByRole("button",{name:"Open settings"}).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(screen.getByRole("button",{name:"Open settings"}));
    await fireEvent.click(screen.getByRole("tab",{name:"Updates"}));
    await fireEvent.click(screen.getByRole("button",{name:"Install update…"}));
    await waitFor(()=>expect(document.activeElement).toBe(screen.getByRole("button",{name:"Install and restart"})));
    bridge.listen.mock.calls.find(([name])=>name==="update-state")![1]({payload:{...state.updateStatus,offeredVersion:"3.0.0",phase:"offered"}});
    await waitFor(()=>expect(document.activeElement).toBe(screen.getByRole("tab",{name:"Updates"})));
    expect(screen.queryByRole("group",{name:"Install update confirmation"})).toBeNull();
    expect(bridge.invoke.mock.calls.map(([name])=>name)).toEqual(["get_operation","get_panel"]);
  });

  it("completion of a passive operation recovers a removed Cancel control without a command", async () => {
    render(App);await waitFor(()=>expect(screen.getByRole("button",{name:"Open settings"}).hasAttribute("disabled")).toBe(false));
    const changed=bridge.listen.mock.calls.find(([name])=>name==="operation-state")![1];
    changed({payload:{generation:"1",active:{id:"1",label:"Sharing",cancellable:true,cancelling:false}}});
    const cancel=await screen.findByRole("button",{name:"Cancel operation"});cancel.focus();
    changed({payload:{generation:"2",active:null}});
    await waitFor(()=>expect(document.activeElement).toBe(screen.getByRole("main",{name:"Clipboard panel"})));
    expect(bridge.invoke.mock.calls.map(([name])=>name)).toEqual(["get_operation","get_panel"]);
  });

  it("settings tabs expose one panel and one tab stop with arrows, Home and End", async () => {
    render(App);
    await waitFor(()=>expect(screen.getByRole("button",{name:"Open settings"}).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(screen.getByRole("button",{name:"Open settings"}));
    const connection=screen.getByRole("tab",{name:"Connection",exact:true});
    await waitFor(()=>expect(document.activeElement).toBe(connection));
    expect(screen.getByRole("tablist",{name:"Settings sections"})).toBeTruthy();
    for(const tab of screen.getAllByRole("tab")) {
      expect(tab.tabIndex).toBe(tab===connection?0:-1);
      expect(tab.getAttribute("aria-controls")).toBe(screen.getByRole("tabpanel").id);
    }
    await fireEvent.keyDown(connection,{key:"ArrowRight"});
    const shortcuts=screen.getByRole("tab",{name:"Shortcuts",exact:true});
    await waitFor(()=>expect(document.activeElement).toBe(shortcuts));
    expect(shortcuts.getAttribute("aria-selected")).toBe("true");
    expect(screen.getByRole("tabpanel").getAttribute("aria-labelledby")).toBe(shortcuts.id);
    await fireEvent.keyDown(shortcuts,{key:"End"});
    await waitFor(()=>expect(document.activeElement).toBe(screen.getByRole("tab",{name:"Updates",exact:true})));
    await fireEvent.keyDown(document.activeElement!,{key:"ArrowRight"});
    await waitFor(()=>expect(document.activeElement).toBe(connection));
    await fireEvent.keyDown(connection,{key:"ArrowLeft"});
    await waitFor(()=>expect(document.activeElement).toBe(screen.getByRole("tab",{name:"Updates",exact:true})));
    await fireEvent.keyDown(document.activeElement!,{key:"Home"});
    await fireEvent.keyDown(window,{key:"Escape"});
    await waitFor(()=>expect(document.activeElement).toBe(screen.getByRole("button",{name:"Open settings"})));
    expect(bridge.invoke.mock.calls.map(([name])=>name)).toEqual(["get_operation","get_panel"]);
  });

  it("relative ticks retain row identity, selection, disclosure state and clipboard command silence", async () => {
    const now=Date.UTC(2026,9,18,12);
    state.settings.target="hub";
    state.history={epoch:"epoch-1",selectedId:"one",items:[{id:"one",label:"Saved item",kind:"text",encodedBytes:1048576,source:"Laptop",receivedAt:String(now-90000),expiresAt:String(now+90000)}]};
    vi.useFakeTimers();vi.setSystemTime(now);
    const rendered=render(App);
    await vi.advanceTimersByTimeAsync(0);
    // Visibility refresh samples the clock without an IPC request.
    await fireEvent(document,new Event("visibilitychange"));
    await vi.advanceTimersByTimeAsync(30000);
    const row=screen.getByRole("button",{name:/Saved item.*Laptop/});
    const disclosure=screen.getByText("Exact times",{exact:false}).closest("details")!;
    disclosure.open=true;row.focus();
    expect(row.getAttribute("aria-pressed")).toBe("true");
    expect(row.textContent).toContain("1.0 MiB");
    await vi.advanceTimersByTimeAsync(60000);
    expect(screen.getByRole("button",{name:/Saved item.*Laptop/})).toBe(row);
    expect(document.activeElement).toBe(row);expect(disclosure.open).toBe(true);
    expect(screen.getByText(/Past expiry \(local clock\)/)).toBeTruthy();
    expect(row.closest("li")!.querySelector('[aria-live="off"]')).toBeTruthy();
    expect(screen.getByText(new Date(now-90000).toLocaleString())).toBeTruthy();
    expect(bridge.invoke.mock.calls.map(([name])=>name)).toEqual(["get_operation","get_panel"]);
    rendered.unmount();await vi.advanceTimersByTimeAsync(0);expect(vi.getTimerCount()).toBe(0);
  });

  it("hidden history clocks pause and resume without fetching or changing selection", async () => {
    const now=Date.UTC(2026,9,18,12);vi.useFakeTimers();vi.setSystemTime(now);
    let visibility: DocumentVisibilityState="visible";
    vi.spyOn(document,"visibilityState","get").mockImplementation(()=>visibility);
    state.settings.target="hub";state.history={epoch:"epoch",selectedId:"one",items:[{id:"one",label:"Clock item",kind:"text",encodedBytes:1,source:null,receivedAt:String(now),expiresAt:String(now+60000)}]};
    render(App);await vi.advanceTimersByTimeAsync(0);
    expect(screen.getByText(/Just now/)).toBeTruthy();
    visibility="hidden";await fireEvent(document,new Event("visibilitychange"));await vi.advanceTimersByTimeAsync(0);
    expect(vi.getTimerCount()).toBe(0);
    await vi.advanceTimersByTimeAsync(120000);expect(screen.getByText(/Just now/)).toBeTruthy();
    visibility="visible";await fireEvent(document,new Event("visibilitychange"));await vi.advanceTimersByTimeAsync(0);
    expect(screen.getByText(/2 minutes ago/)).toBeTruthy();
    expect(screen.getByRole("button",{name:/Clock item/}).getAttribute("aria-pressed")).toBe("true");
    expect(bridge.invoke.mock.calls.map(([name])=>name)).toEqual(["get_operation","get_panel"]);
  });

  it("passive history replies preserve existing focus and recover only when its row disappears", async () => {
    state.settings.target="hub";state.history={epoch:"epoch",selectedId:"one",items:[{id:"one",label:"Focus item",kind:"text",encodedBytes:1,source:null,receivedAt:"100",expiresAt:"200"}]};
    render(App);
    const row=await screen.findByRole("button",{name:/Focus item/});row.focus();
    const changed=bridge.listen.mock.calls.find(([name])=>name==="settings-changed")![1];
    changed();await waitFor(()=>expect(bridge.invoke).toHaveBeenCalledTimes(3));
    expect(document.activeElement).toBe(row);
    state.history.items=[];state.history.selectedId=null;changed();
    await waitFor(()=>expect(document.activeElement).toBe(screen.getByRole("heading",{name:"Temporary history 0"})));
    expect(screen.getByRole("status",{name:"History selection"}).textContent).toContain("no longer available");
    expect(bridge.invoke.mock.calls.every(([name])=>["get_operation","get_panel"].includes(name))).toBe(true);
  });

  it("history selection moves focus to the preview while preserving the selected ID and revision guard", async () => {
    state.settings.target="hub";state.settingsRevision="7";
    state.history={epoch:"epoch",selectedId:"one",items:[{id:"one",label:"Select me",kind:"text",encodedBytes:4,source:"Laptop",receivedAt:"100",expiresAt:"200"}]};
    const normal=bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation((name,args)=>{
      if(name==="select_item")state.item={kind:"text",label:"Select me",text:"Data",imageUrl:null,encodedBytes:4,createdAt:100};
      return normal(name,args);
    });
    render(App);const row=await screen.findByRole("button",{name:/Select me.*Laptop/});
    row.focus();await fireEvent.click(row);
    await waitFor(()=>expect(document.activeElement).toBe(screen.getByRole("heading",{name:"Select me"})));
    expect(bridge.invoke.mock.calls.find(([name])=>name==="select_item")![1]).toEqual({itemId:"one",epoch:"epoch",expectedRevision:"7"});
    expect(bridge.invoke.mock.calls.some(([name])=>["copy_item","share_clipboard"].includes(name))).toBe(false);
  });

  it("epoch/profile replacement recovers removed history focus without transferring", async () => {
    state.settings.target="hub";state.history={epoch:"old",selectedId:"one",items:[{id:"one",label:"Epoch item",kind:"text",encodedBytes:1,source:null,receivedAt:"100",expiresAt:"200"}]};
    render(App);const row=await screen.findByRole("button",{name:/Epoch item/});row.focus();
    const changed=bridge.listen.mock.calls.find(([name])=>name==="settings-changed")![1];
    state.history.epoch="new";changed();
    await waitFor(()=>expect(document.activeElement).toBe(screen.getByRole("heading",{name:"Temporary history 1"})));
    expect(screen.getByRole("button",{name:/Epoch item/})).not.toBe(row);
    state.history={epoch:null,selectedId:null,items:[]};state.settings.target="other";changed();
    await waitFor(()=>expect(document.activeElement).toBe(screen.getByRole("heading",{name:"Temporary history 0"})));
    expect(bridge.invoke.mock.calls.every(([name])=>["get_operation","get_panel"].includes(name))).toBe(true);
  });

  it("platform shortcut labels and field-help relationships remain local presentation", async () => {
    state.platform="macos";state.activeShortcut="CommandOrControl+Alt+Shift+KeyS";
    render(App);await waitFor(()=>expect(screen.getByRole("button",{name:"Open settings"}).hasAttribute("disabled")).toBe(false));
    expect(screen.getByText("Cmd + Option + Shift + S")).toBeTruthy();
    await fireEvent.click(screen.getByRole("button",{name:"Open settings"}));
    for(const section of ["Connection","Shortcuts","Behavior","Updates"]){
      await fireEvent.click(screen.getByRole("tab",{name:section,exact:true}));
      for(const element of document.querySelectorAll('[aria-describedby]'))for(const id of element.getAttribute("aria-describedby")!.split(/\s+/))expect(document.getElementById(id)).not.toBeNull();
    }
    await fireEvent.click(screen.getByRole("tab",{name:"Shortcuts",exact:true}));
    expect(screen.getByLabelText("Open panel").textContent).toBe("Cmd + Option + Shift + S");
    expect(state.settings.openShortcut).toBe("CommandOrControl+Alt+Shift+S");
    expect(bridge.invoke.mock.calls.map(([name])=>name)).toEqual(["get_operation","get_panel"]);
  });

  it("history confirmations restore focus on Escape and successful mutation", async () => {
    state.settings.target="hub";state.history={epoch:"epoch",selectedId:"one",items:[{id:"one",label:"Item",kind:"text",encodedBytes:1,source:null,receivedAt:"100",expiresAt:"200"}]};
    render(App);await screen.findByRole("button",{name:"Clear history…"});
    await fireEvent.click(screen.getByRole("button",{name:"Clear history…"}));
    await waitFor(()=>expect(document.activeElement).toBe(screen.getByRole("button",{name:"Clear history",exact:true})));
    await fireEvent.keyDown(window,{key:"Escape"});
    await waitFor(()=>expect(document.activeElement).toBe(screen.getByRole("button",{name:"Clear history…"})));
    expect(screen.queryByRole("group",{name:"History removal confirmation"})).toBeNull();
    await fireEvent.click(screen.getByRole("button",{name:"Clear history…"}));
    await fireEvent.click(screen.getByRole("button",{name:"Clear history",exact:true}));
    await waitFor(()=>expect(document.activeElement).toBe(screen.getByRole("heading",{name:"Temporary history 1"})));
    expect(bridge.invoke.mock.calls.find(([name])=>name==="clear_history")![1]).toEqual({epoch:"epoch",expectedRevision:null});
  });

  it("profile and install confirmation cancellation returns focus without committing", async () => {
    state.settingsSaved=true;state.settings.target="hub";state.profiles.push({...state.profiles[0],id:"two",name:"Other"});
    state.updateStatus={...state.updateStatus,available:true,currentVersion:"1.0.0",offeredVersion:"2.0.0",phase:"ready"};
    render(App);await screen.findByText("hub");await fireEvent.click(screen.getByRole("button",{name:"Open settings"}));
    await fireEvent.click(screen.getByRole("button",{name:"Remove…"}));
    await waitFor(()=>expect(document.activeElement).toBe(screen.getByRole("button",{name:"Remove connection",exact:true})));
    await fireEvent.keyDown(window,{key:"Escape"});
    await waitFor(()=>expect(document.activeElement).toBe(screen.getByRole("button",{name:"Remove…"})));
    await fireEvent.click(screen.getByRole("tab",{name:"Updates",exact:true}));
    await fireEvent.click(screen.getByRole("button",{name:"Install update…"}));
    await waitFor(()=>expect(document.activeElement).toBe(screen.getByRole("button",{name:"Install and restart"})));
    await fireEvent.keyDown(window,{key:"Escape"});
    await waitFor(()=>expect(document.activeElement).toBe(screen.getByRole("button",{name:"Install update…"})));
    expect(bridge.invoke.mock.calls.map(([name])=>name)).toEqual(["get_operation","get_panel"]);
  });

  it("routine confirmations expire, are scoped to their section, and clean up on unmount", async () => {
    const rendered=render(App);
    await waitFor(()=>expect(screen.getByRole("button",{name:"Open settings"}).hasAttribute("disabled")).toBe(false));
    vi.useFakeTimers();
    await fireEvent.click(screen.getByRole("button",{name:"Open settings"}));
    await fireEvent.input(screen.getByLabelText("Connection name"),{target:{value:"Renamed"}});
    await fireEvent.click(screen.getByRole("button",{name:"Apply settings"}));
    await vi.advanceTimersByTimeAsync(0);
    expect(screen.getByText("Settings applied.")).toBeTruthy();
    await vi.advanceTimersByTimeAsync(4999);
    expect(screen.getByText("Settings applied.")).toBeTruthy();
    await vi.advanceTimersByTimeAsync(1);
    expect(screen.queryByText("Settings applied.")).toBeNull();
    await fireEvent.click(screen.getByRole("button",{name:"Restore advanced defaults"}));
    expect(screen.getByText("Advanced defaults restored in your draft.")).toBeTruthy();
    await fireEvent.click(screen.getByRole("tab",{name:"Behavior",exact:true}));
    expect(screen.queryByText("Advanced defaults restored in your draft.")).toBeNull();
    await fireEvent.click(screen.getByRole("tab",{name:"Shortcuts",exact:true}));
    await fireEvent.click(screen.getByRole("button",{name:"Restore default shortcut"}));
    await vi.advanceTimersByTimeAsync(0);
    expect(vi.getTimerCount()).toBe(1);
    rendered.unmount();
    expect(vi.getTimerCount()).toBe(0);
    await vi.advanceTimersByTimeAsync(10000);
  });

  it("a late Test completion cannot restore success after leaving and returning to its section", async () => {
    let finish!: (view: PanelView) => void;
    state.notificationStatus={available:true,detail:"System settings control delivery."};
    const normal=bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation((name,args)=>name==="test_notification"?new Promise<PanelView>(resolve=>finish=resolve):normal(name,args));
    render(App);
    await waitFor(()=>expect(screen.getByRole("button",{name:"Open settings"}).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(screen.getByRole("button",{name:"Open settings"}));
    await fireEvent.click(screen.getByRole("tab",{name:"Behavior",exact:true}));
    await fireEvent.change(screen.getByLabelText("Feedback after shortcut actions"),{target:{value:"errors"}});
    await fireEvent.click(screen.getByRole("button",{name:"Test notification"}));
    await fireEvent.click(screen.getByRole("tab",{name:"Shortcuts",exact:true}));
    await fireEvent.click(screen.getByRole("tab",{name:"Behavior",exact:true}));
    finish(structuredClone(state));
    await waitFor(()=>expect(screen.getByRole("button",{name:"Back"}).hasAttribute("disabled")).toBe(false));
    expect(screen.queryByText("Test notification submitted to the system.")).toBeNull();
    expect(bridge.invoke.mock.calls.map(([name])=>name)).toEqual(["get_operation","get_panel","test_notification"]);
  });

  it("an old success timer cannot dismiss a newer confirmation or repair warning", async () => {
    render(App);
    await waitFor(()=>expect(screen.getByRole("button",{name:"Open settings"}).hasAttribute("disabled")).toBe(false));
    vi.useFakeTimers();
    await fireEvent.click(screen.getByRole("button",{name:"Open settings"}));
    await fireEvent.input(screen.getByLabelText("Connection name"),{target:{value:"Draft"}});
    await fireEvent.click(screen.getByRole("button",{name:"Discard changes"}));
    await vi.advanceTimersByTimeAsync(4000);
    await fireEvent.click(screen.getByRole("button",{name:"Restore advanced defaults"}));
    await vi.advanceTimersByTimeAsync(1000);
    expect(screen.getByText("Advanced defaults restored in your draft.")).toBeTruthy();
    bridge.listen.mock.calls.find(([name])=>name==="operation-notice")![1]({payload:"Shortcut cleanup failed. Quit and reopen."});
    await vi.advanceTimersByTimeAsync(10000);
    expect(screen.getByRole("alert").textContent).toContain("Shortcut cleanup failed");
    await fireEvent.click(screen.getByRole("tab",{name:"Behavior",exact:true}));
    await fireEvent.click(screen.getByRole("button",{name:"Back"}));
    expect(screen.getByRole("alert").textContent).toContain("Shortcut cleanup failed");
  });

  it("routine reloads and navigation retain quiet transfer results, errors and uncertain recovery", async () => {
    state.operationNotice={id:"1",failed:false,message:"Copied to this computer. Paste when ready."};
    state.pendingShare={id:"uncertain",canRetry:false};
    render(App);
    await screen.findByRole("region",{name:"Latest transfer result"});
    vi.useFakeTimers();
    await fireEvent.click(screen.getByRole("button",{name:"Open settings"}));
    await fireEvent.click(screen.getByRole("tab",{name:"Behavior",exact:true}));
    await fireEvent.change(screen.getByLabelText("Feedback after shortcut actions"),{target:{value:"off"}});
    await fireEvent.click(screen.getByRole("button",{name:"Apply settings"}));
    await vi.advanceTimersByTimeAsync(0);
    expect(screen.getByText("Settings applied.")).toBeTruthy();
    await fireEvent.click(screen.getByRole("button",{name:"Back"}));
    expect(screen.queryByText("Settings applied.")).toBeNull();
    bridge.listen.mock.calls.find(([name])=>name==="operation-notice")![1]({payload:"Operation cancelled. The share may have reached the server."});
    bridge.listen.mock.calls.find(([name])=>name==="settings-changed")![1]();
    await vi.advanceTimersByTimeAsync(60000);
    expect(screen.getByText("Copied to this computer.")).toBeTruthy();
    expect(screen.getByText("Paste when ready.").closest("details")).toBeTruthy();
    expect(screen.getByRole("alert").textContent).toContain("Operation cancelled");
    expect(screen.getByRole("region",{name:"Share recovery"})).toBeTruthy();
    expect(bridge.invoke.mock.calls.some(([name])=>["share_clipboard","copy_item","test_notification"].includes(name))).toBe(false);
  });

  it.each(["panel","all","errors","off"] as const)("notification help/test is conditional for %s without automatic test dispatch", async (mode) => {
    state.notificationStatus={available:false,detail:"This build shows results in the panel."};
    state.trayStatus="Unavailable: reopen through the launcher.";
    state.startupStatus={available:true,enabled:false,desired:true,detail:"Startup differs from the saved choice."};
    state.shortcutStatus="Copy shared clipboard could not be registered. Choose another shortcut.";
    render(App);
    await waitFor(()=>expect(screen.getByRole("button",{name:"Open settings"}).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(screen.getByRole("button",{name:"Open settings"}));
    await fireEvent.click(screen.getByRole("tab",{name:"Behavior",exact:true}));
    await fireEvent.change(screen.getByLabelText("Feedback after shortcut actions"),{target:{value:mode}});
    expect(screen.getByText("Startup differs from the saved choice.")).toBeTruthy();
    expect(screen.getByText("Unavailable: reopen through the launcher.")).toBeTruthy();
    const test=screen.queryByRole("button",{name:"Test notification"});
    if(mode==="all"||mode==="errors") {
      expect(test?.hasAttribute("disabled")).toBe(true);
      expect(screen.getByText("This build shows results in the panel.").closest("details")!.open).toBe(true);
      expect(screen.getByText(/Sends a test now, without applying your draft/)).toBeTruthy();
    } else expect(test).toBeNull();
    await fireEvent.click(screen.getByRole("tab",{name:"Shortcuts",exact:true}));
    expect(screen.getByText(/Copy shared clipboard could not be registered/)).toBeTruthy();
    expect(bridge.invoke.mock.calls.map(([name])=>name)).toEqual(["get_operation","get_panel"]);
  });

  it("healthy state is consolidated with details available and routine tray availability omitted", async () => {
    state.settings.target="hub";state.connected=true;state.trayStatus="Available";
    state.historyStatus={phase:"live",detail:"History is up to date.",retryInMs:null};
    render(App);
    await screen.findByText("History up to date");
    expect(screen.queryByText("Server reachable")).toBeNull();
    expect(screen.getByText("History is up to date.").closest("details")!.open).toBe(false);
    await fireEvent.click(screen.getByRole("button",{name:"Open settings"}));
    await fireEvent.click(screen.getByRole("tab",{name:"Behavior",exact:true}));
    expect(screen.queryByText("Tray menu")).toBeNull();
    expect(screen.getByRole("checkbox",{name:"Start at login"}).compareDocumentPosition(screen.getByRole("checkbox",{name:"Show content previews"}))&Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0);
  });

  it("history refresh stays available beside an empty history without issuing a transfer on navigation", async () => {
    state.settings.target="saved-hub";
    state.history={epoch:"epoch-1",selectedId:null,items:[]};
    state.clipboardChecked=true;
    render(App);
    await screen.findByText("saved-hub");
    const refresh=screen.getByRole("button",{name:"Refresh history"});
    expect(screen.getAllByRole("button",{name:/Refresh/})).toHaveLength(1);
    expect(refresh.closest("section")?.getAttribute("aria-label")).toBe("Temporary clipboard history");
    expect(screen.getByRole("button",{name:"Copy to this computer"}).hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("button",{name:"Share current clipboard"}).hasAttribute("disabled")).toBe(false);
    await fireEvent.click(refresh);
    expect(bridge.invoke.mock.calls.map(([name])=>name)).toEqual(["get_operation","get_panel","refresh_item"]);
    expect(bridge.invoke.mock.calls[2][1]).toEqual({expectedRevision:state.settingsRevision});
  });

  it("collapsed advanced fields retain port/key inheritance and edits remain drafts", async () => {
    state.settings.target="saved-hub";
    state.settings.port=2222;state.settings.identityFile="keys/saved key";
    render(App);
    await screen.findByText("saved-hub");
    await fireEvent.click(screen.getByRole("button",{name:"Open settings"}));
    const port=screen.getByLabelText(/Port/) as HTMLInputElement;
    const identity=screen.getByLabelText(/Identity file/) as HTMLInputElement;
    const details=port.closest("details")!;
    expect(details.open).toBe(false);
    expect(identity.closest("details")).toBe(details);
    expect(port.value).toBe("2222");expect(identity.value).toBe("keys/saved key");
    await fireEvent.input(port,{target:{value:""}});
    await fireEvent.input(identity,{target:{value:""}});
    await fireEvent.click(screen.getByRole("button",{name:"Restore advanced defaults"}));
    expect(state.settings.port).toBe(2222);
    expect(state.settings.identityFile).toBe("keys/saved key");
    await fireEvent.click(screen.getByRole("button",{name:"Apply settings"}));
    await screen.findByText("Settings applied.");
    const saved=bridge.invoke.mock.calls.find(([name])=>name==="apply_settings")![1].settings;
    expect(saved.port).toBeNull();expect(saved.identityFile).toBe("");
  });

  it("native field validation reveals a collapsed advanced field without sending settings", async () => {
    render(App);
    await waitFor(()=>expect(screen.getByRole("button",{name:"Open settings"}).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(screen.getByRole("button",{name:"Open settings"}));
    const port=screen.getByLabelText(/Port/) as HTMLInputElement;
    await fireEvent.input(port,{target:{value:"0"}});
    await fireEvent.invalid(port);
    expect(port.closest("details")!.open).toBe(true);
    expect(bridge.invoke.mock.calls.map(([name])=>name)).toEqual(["get_operation","get_panel"]);
  });

  it("backend advanced validation returns to the field while retaining the whole unsaved draft", async () => {
    const normal=bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation((name,args)=>name==="apply_settings"?Promise.reject("Timeout must be a positive whole number."):normal(name,args));
    render(App);
    await waitFor(()=>expect(screen.getByRole("button",{name:"Open settings"}).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(screen.getByRole("button",{name:"Open settings"}));
    await fireEvent.input(screen.getByLabelText("Timeout (milliseconds)"),{target:{value:"bad"}});
    await fireEvent.click(screen.getByRole("tab",{name:"Behavior",exact:true}));
    await fireEvent.click(screen.getByLabelText("Show content previews"));
    await fireEvent.click(screen.getByRole("button",{name:"Apply settings"}));
    await screen.findByRole("alert");
    const timeout=await screen.findByLabelText("Timeout (milliseconds)") as HTMLInputElement;
    await waitFor(()=>expect(document.activeElement).toBe(timeout));
    expect(timeout.closest("details")!.open).toBe(true);expect(timeout.value).toBe("bad");
    expect(state.settings.showPreview).toBe(true);
    await fireEvent.click(screen.getByRole("tab",{name:"Behavior",exact:true}));
    expect((screen.getByLabelText("Show content previews") as HTMLInputElement).checked).toBe(false);
    expect(bridge.invoke.mock.calls.map(([name])=>name)).toEqual(["get_operation","get_panel","apply_settings","get_panel"]);
  });

  it("curated notes are inert bullets after the primary update action", async () => {
    state.updateStatus={...state.updateStatus,available:true,currentVersion:"1.0.0",offeredVersion:"2.0.0",phase:"offered",detail:"An update is available.",notes:"# 2.0.0\n\n- Faster startup.\n- <script>window.hacked = true</script>\n"};
    render(App);
    await waitFor(()=>expect(screen.getByRole("button",{name:"Open settings"}).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(screen.getByRole("button",{name:"Open settings"}));
    await fireEvent.click(screen.getByRole("tab",{name:"Updates",exact:true}));
    const summary=screen.getByRole("region",{name:"What's new"});
    expect(summary.querySelectorAll("li")).toHaveLength(2);
    expect(screen.getByText("<script>window.hacked = true</script>").tagName).toBe("LI");
    expect(document.querySelector("main script")).toBeNull();
    expect(screen.getByText("Installed: 1.0.0")).toBeTruthy();
    expect(screen.getByText("Available: 2.0.0")).toBeTruthy();
    expect(screen.queryByRole("heading",{name:"Updates",exact:true})).toBeNull();
    expect(screen.getByRole("button",{name:"Download update"}).compareDocumentPosition(summary)&Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0);
    expect(bridge.invoke.mock.calls.map(([name])=>name)).toEqual(["get_operation","get_panel"]);
  });

  it.each(["", "# Changelog\n\n## [Unreleased]\n\n"+"Old release details.\n".repeat(80), "# 9.9.9\n\n- Notes for another version.\n"])("missing or legacy notes keep actions first and preserve full text (%#)", async (notes) => {
    state.updateStatus={...state.updateStatus,available:true,currentVersion:"1.0.0",offeredVersion:"2.0.0",phase:"offered",notes};
    render(App);
    await waitFor(()=>expect(screen.getByRole("button",{name:"Open settings"}).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(screen.getByRole("button",{name:"Open settings"}));
    await fireEvent.click(screen.getByRole("tab",{name:"Updates",exact:true}));
    const action=screen.getByRole("button",{name:"Download update"});
    const disclosure=document.querySelector("details.update-notes") as HTMLDetailsElement | null;
    if(notes) {
      expect(disclosure!.open).toBe(false);
      const full=disclosure!.querySelector("pre")!;
      expect(full.textContent).toBe(notes);
      expect(action.compareDocumentPosition(disclosure!)&Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0);
      bridge.listen.mock.calls.find(([name])=>name==="update-state")![1]({payload:{...state.updateStatus,phase:"checking",busy:true,detail:"Checking for updates…"}});
      await waitFor(()=>expect(action.hasAttribute("disabled")).toBe(true));
      expect(disclosure!.querySelector("pre")).toBe(full);
    } else {
      expect(disclosure).toBeNull();
      expect(screen.queryByText("What's new")).toBeNull();
    }
    expect(bridge.invoke.mock.calls.map(([name])=>name)).toEqual(["get_operation","get_panel"]);
  });

  it.each([
    ["checking","Download update",true], ["offered","Download update",false],
    ["downloading","Downloading…",true], ["verifying","Verifying…",true],
    ["ready","Install update…",false], ["failed","Download update",false],
  ] as const)("update phase %s presents the appropriate action without triggering it", async (phase,action,busy) => {
    state.updateStatus={...state.updateStatus,available:true,currentVersion:"1.0.0",offeredVersion:"2.0.0",phase,busy,detail:`Update phase: ${phase}`,downloadedBytes:512,totalBytes:1024};
    render(App);
    await waitFor(()=>expect(screen.getByRole("button",{name:"Open settings"}).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(screen.getByRole("button",{name:"Open settings"}));
    await fireEvent.click(screen.getByRole("tab",{name:"Updates",exact:true}));
    expect(screen.getByRole("button",{name:action,exact:true}).hasAttribute("disabled")).toBe(busy);
    expect(screen.getByRole("button",{name:"Check for updates"}).hasAttribute("disabled")).toBe(busy);
    if(phase==="downloading")expect(screen.getByText("512 B of 1.0 KiB")).toBeTruthy();
    if(phase==="failed")expect(screen.getByRole("alert").textContent).toBe("Update phase: failed");
    expect(bridge.invoke.mock.calls.map(([name])=>name)).toEqual(["get_operation","get_panel"]);
  });

  it("Back, Escape, hiding and reopening retain drafts without changing the active connection or previews", async () => {
    state.settings.target = "saved-hub";
    state.settingsSaved = true;
    state.settingsRevision = "7";
    state.item = { kind: "text", label: "Saved item", text: "Visible saved preview", imageUrl: null, encodedBytes: 21, createdAt: 1 };
    render(App);
    await screen.findByText("Visible saved preview");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    expect(screen.getByText("All changes saved")).toBeTruthy();
    await fireEvent.input(screen.getByLabelText("SSH alias or user@host"), { target: { value: "draft-hub" } });
    await fireEvent.click(screen.getByRole("tab", { name: "Behavior" }));
    await fireEvent.click(screen.getByLabelText("Show content previews"));
    expect(screen.getByText("Unsaved changes")).toBeTruthy();
    expect(screen.queryByText("All changes saved")).toBeNull();
    expect(screen.getByText("Changes take effect after Apply.")).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(screen.getByRole("region", { name: "Unsaved settings" })).toBeTruthy();
    expect(screen.getByText("saved-hub")).toBeTruthy();
    expect(screen.getByText("Visible saved preview")).toBeTruthy();
    await fireEvent.keyDown(window, { key: "Escape" });
    bridge.listen.mock.calls.find(([name]) => name === "navigate")![1]({ payload: "panel" });
    await fireEvent.click(screen.getByRole("button", { name: "Return to settings" }));
    expect((screen.getByLabelText("Show content previews") as HTMLInputElement).checked).toBe(false);
    await fireEvent.click(screen.getByRole("tab", { name: "Connection" }));
    expect((screen.getByLabelText("SSH alias or user@host") as HTMLInputElement).value).toBe("draft-hub");
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.getByRole("region", { name: "Unsaved settings" })).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Discard changes" }));
    expect(screen.queryByRole("region", { name: "Unsaved settings" })).toBeNull();
    expect(state.settings.target).toBe("saved-hub");
    expect(state.settings.showPreview).toBe(true);
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel", "hide_panel"]);
  });

  it("the persistent save actions apply the whole draft from Updates with its original revision", async () => {
    state.settings.target = "saved-hub";
    state.settingsRevision = "7";
    state.updateStatus = { ...state.updateStatus, available: true, offeredVersion: "2.0.0", phase: "ready" };
    render(App);
    await screen.findByText("saved-hub");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.input(screen.getByLabelText("Connection name"), { target: { value: "Work" } });
    await fireEvent.click(screen.getByRole("tab", { name: "Behavior" }));
    await fireEvent.click(screen.getByLabelText("Hide panel after copying"));
    await fireEvent.click(screen.getByRole("tab", { name: "Updates" }));
    const savebar = screen.getByRole("region", { name: "Settings changes" });
    expect(savebar.closest("main")).toBeNull();
    expect(screen.getByRole("button", { name: "Install update…" }).hasAttribute("disabled")).toBe(true);
    const apply = screen.getByRole("button", { name: "Apply settings" }) as HTMLButtonElement;
    expect(apply.form).toBe(document.getElementById("settings-form"));
    await fireEvent.click(apply);
    await screen.findByText("All changes saved");
    const saves = bridge.invoke.mock.calls.filter(([name]) => name === "apply_settings");
    expect(saves).toHaveLength(1);
    expect(saves[0][1]).toEqual({ settings: { ...initialView.settings, target: "saved-hub", profileName: "Work", closeAfterCopy: true }, expectedRevision: "7" });
    expect(screen.getByRole("button", { name: "Install update…" }).hasAttribute("disabled")).toBe(false);
    expect(apply.disabled).toBe(true);
  });

  it("form submission cannot bypass a read-only, recording, busy or stale draft guard", async () => {
    state.settings.target = "saved-hub";
    state.settingsRevision = "7";
    state.settingsWritable = false;
    render(App);
    await screen.findByText("saved-hub");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.input(screen.getByLabelText("Connection name"), { target: { value: "Work" } });
    const submit = () => fireEvent.submit(document.getElementById("settings-form")!);
    await submit();
    state.settingsWritable = true;
    const changed = bridge.listen.mock.calls.find(([name]) => name === "settings-changed")![1];
    changed();
    await waitFor(() => expect(screen.getByRole("button", { name: "Apply settings" }).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(screen.getByRole("tab", { name: "Shortcuts" }));
    await fireEvent.click(screen.getByLabelText("Open panel"));
    await screen.findByText("Press a shortcut · Esc to cancel");
    await submit();
    await fireEvent.keyDown(window, { key: "Escape" });
    const operation = bridge.listen.mock.calls.find(([name]) => name === "operation-state")![1];
    operation({ payload: { generation: "1", active: { id: "1", label: "Sharing", cancellable: true, cancelling: false } } });
    await waitFor(() => expect(screen.getByRole("button", { name: "Apply settings" }).hasAttribute("disabled")).toBe(true));
    await submit();
    operation({ payload: { generation: "2", active: null } });
    state.settingsRevision = "8";
    state.settings.target = "cli-hub";
    changed();
    await screen.findByText("Saved settings changed");
    await submit();
    expect(bridge.invoke.mock.calls.some(([name]) => name === "apply_settings")).toBe(false);
    await fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(screen.getByText("Saved settings changed")).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Discard changes" }));
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.click(screen.getByRole("tab", { name: "Connection" }));
    expect((screen.getByLabelText("SSH alias or user@host") as HTMLInputElement).value).toBe("cli-hub");
    expect(screen.queryByText("Unsaved changes")).toBeNull();
  });

  it("secondary form actions and navigation never apply a dirty settings draft", async () => {
    state.settings.openShortcut = "CommandOrControl+Shift+V";
    state.activeShortcut = state.settings.openShortcut;
    state.settings.target = "saved-hub";
    state.updateStatus = { ...state.updateStatus, available: true, offeredVersion: "2.0.0", phase: "offered" };
    const normal = bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation((name, args) => ["check_updates", "download_update"].includes(name) ? structuredClone(state.updateStatus) : normal(name, args));
    render(App);
    await screen.findByText("saved-hub");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.input(screen.getByLabelText("Connection name"), { target: { value: "Work" } });
    await fireEvent.click(screen.getByRole("tab", { name: "Shortcuts" }));
    await fireEvent.click(screen.getByRole("button", { name: "Restore default shortcut" }));
    expect(screen.getByLabelText("Open panel").textContent).toBe("Ctrl + Alt + Shift + S");
    expect(state.settings.openShortcut).toBe("CommandOrControl+Shift+V");
    await fireEvent.click(screen.getByRole("tab", { name: "Updates" }));
    await fireEvent.click(screen.getByRole("button", { name: "Check for updates" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Download update" }).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(screen.getByRole("button", { name: "Download update" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Back" }).hasAttribute("disabled")).toBe(false));
    expect(screen.getByText("Unsaved changes")).toBeTruthy();
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel", "check_updates", "download_update"]);
  });

  it("rechecking an update keeps its card and settings header mounted with progress confined to Updates", async () => {
    state.updateStatus={available:true,currentVersion:"0.3.2",offeredVersion:"0.3.3",notes:"Retained release notes",phase:"offered",detail:"An update is available.",busy:false,downloadedBytes:0,totalBytes:null};
    const normal=bridge.invoke.getMockImplementation()!;
    let finish!: (next: PanelView["updateStatus"]) => void;
    bridge.invoke.mockImplementation((name,args)=>name==="check_updates"?new Promise(resolve=>{finish=resolve;}):normal(name,args));
    render(App);
    await waitFor(()=>expect(screen.getByRole("button",{name:"Open settings"}).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(screen.getByRole("button",{name:"Open settings"}));
    await fireEvent.click(screen.getByRole("tab",{name:"Updates"}));
    const heading=document.querySelector("main")!.firstElementChild;
    const notes=screen.getByText("Retained release notes");
    const download=screen.getByRole("button",{name:"Download update"});
    await fireEvent.click(screen.getByRole("button",{name:"Check for updates"}));
    const update=bridge.listen.mock.calls.find(([name])=>name==="update-state")![1];
    const operation=bridge.listen.mock.calls.find(([name])=>name==="operation-state")![1];
    update({payload:{...state.updateStatus,phase:"checking",busy:true,detail:"Checking for updates…"}});
    operation({payload:{generation:"1",active:{id:"1",label:"Checking for updates",cancellable:true,cancelling:false}}});
    const cancel=await screen.findByRole("button",{name:"Cancel operation"});
    expect(cancel.closest(".updates-panel")).toBeTruthy();
    expect(document.querySelector("main")!.firstElementChild).toBe(heading);
    expect(screen.getByText("Retained release notes")).toBe(notes);
    expect(screen.getByRole("button",{name:"Download update"})).toBe(download);
    expect(download.hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("button",{name:"Check for updates"}).textContent).toBe("Checking…");
    operation({payload:{generation:"2",active:null}});
    finish(structuredClone(state.updateStatus));
    await waitFor(()=>expect(download.hasAttribute("disabled")).toBe(false));
    expect(screen.getByText("Retained release notes")).toBe(notes);
    expect(bridge.invoke.mock.calls.some(([name])=>["download_update","install_update"].includes(name))).toBe(false);
  });

  it("update notes are inert and a verified package still needs explicit install confirmation", async () => {
    state.updateStatus={available:true,currentVersion:"1.0.0",offeredVersion:"2.0.0",notes:"<script>never executed</script>",phase:"ready",detail:"Verified",busy:false,downloadedBytes:123,totalBytes:123};
    const normal=bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation((name,args)=>name==="install_update"?undefined:normal(name,args));
    render(App);
    await waitFor(()=>expect(screen.getByRole("button",{name:"Quit"}).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(screen.getByRole("button",{name:"Open settings"}));
    await fireEvent.click(screen.getByRole("tab",{name:"Updates"}));
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
    await fireEvent.click(screen.getByRole("tab",{name:"Updates"}));
    expect(screen.getByRole("button",{name:"Download update"})).toBeTruthy();
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
    await fireEvent.click(screen.getByRole("tab",{name:"Updates"}));
    await fireEvent.click(screen.getByRole("button",{name:"Install update…"}));
    bridge.listen.mock.calls.find(([name])=>name==="operation-state")![1]({payload:{generation:"1",active:{id:"1",label:"Sharing",cancellable:true,cancelling:false}}});
    await waitFor(()=>expect(screen.getByRole("button",{name:"Install and restart"}).hasAttribute("disabled")).toBe(true));
    expect(bridge.invoke.mock.calls.some(([name])=>name==="install_update")).toBe(false);
  });

  it("startup edits share Apply and Discard with other settings while showing actual imported state", async () => {
    state.settingsRevision = "7";
    state.startupStatus = { available: true, enabled: false, desired: true, detail: "Retained preference differs from this app's startup entry." };
    render(App);
    await waitFor(() => expect(screen.getByRole("button", { name: "Quit" }).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(await screen.findByRole("button", { name: "Open settings" }));
    await fireEvent.click(screen.getByRole("tab", { name: "Behavior" }));
    const checkbox = screen.getByRole("checkbox", { name: "Start at login" }) as HTMLInputElement;
    expect(checkbox.checked).toBe(false);
    expect(bridge.invoke.mock.calls.some(([name]) => name === "set_start_at_login")).toBe(false);
    await fireEvent.click(checkbox);
    expect(screen.getByText("Currently off.")).toBeTruthy();
    await fireEvent.click(screen.getByRole("checkbox", { name: "Show content previews" }));
    expect(checkbox.disabled).toBe(false);
    expect(screen.getByText("Unsaved changes")).toBeTruthy();
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel"]);
    await fireEvent.click(screen.getByRole("button", { name: "Back" }));
    await fireEvent.click(screen.getByRole("button", { name: "Return to settings" }));
    expect((screen.getByLabelText("Start at login") as HTMLInputElement).checked).toBe(true);
    await fireEvent.click(screen.getByRole("button", { name: "Discard changes" }));
    expect((screen.getByLabelText("Start at login") as HTMLInputElement).checked).toBe(false);
    expect((screen.getByLabelText("Show content previews") as HTMLInputElement).checked).toBe(true);
    await fireEvent.click(screen.getByLabelText("Start at login"));
    await fireEvent.click(screen.getByLabelText("Show content previews"));
    await fireEvent.click(screen.getByRole("button", { name: "Apply settings" }));
    await screen.findByText("All changes saved");
    const saves = bridge.invoke.mock.calls.filter(([name]) => name === "apply_settings");
    expect(saves).toHaveLength(1);
    expect(saves[0][1]).toEqual({ settings: { ...initialView.settings, showPreview: false }, startAtLogin: true, expectedRevision: "7" });
    expect(state.startupStatus.enabled).toBe(true);
    expect(state.settingsRevision).toBe("8");
    expect(bridge.invoke.mock.calls.some(([name]) => name === "set_start_at_login")).toBe(false);
  });

  it.each([false, true])("unknown or unavailable startup (available=%s) still allows unrelated settings saves", async (available) => {
    state.startupStatus = { available, enabled: null, desired: true, detail: "Startup state unavailable" };
    render(App);
    await waitFor(() => expect(screen.getByRole("button", { name: "Quit" }).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(await screen.findByRole("button", { name: "Open settings" }));
    await fireEvent.click(screen.getByRole("tab", { name: "Behavior" }));
    expect((screen.getByRole("checkbox", { name: "Start at login" }) as HTMLInputElement).disabled).toBe(true);
    await fireEvent.click(screen.getByLabelText("Show content previews"));
    await fireEvent.click(screen.getByRole("button", { name: "Apply settings" }));
    await screen.findByText("All changes saved");
    expect(bridge.invoke.mock.calls.find(([name]) => name === "apply_settings")![1]).not.toHaveProperty("startAtLogin");
    expect(state.startupStatus.enabled).toBeNull();
    expect(state.startupStatus.desired).toBe(true);
    expect(bridge.invoke.mock.calls.some(([name]) => name === "set_start_at_login")).toBe(false);
  });

  it("toggling startup back to actual state cancels the explicit intent without repairing a saved mismatch", async () => {
    state.startupStatus = { available: true, enabled: false, desired: true, detail: "Retained preference differs from this app's startup entry." };
    render(App);
    await waitFor(() => expect(screen.getByRole("button", { name: "Open settings" }).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.click(screen.getByRole("tab", { name: "Behavior" }));
    await fireEvent.click(screen.getByLabelText("Start at login"));
    expect(screen.getByText("Unsaved changes")).toBeTruthy();
    await fireEvent.click(screen.getByLabelText("Start at login"));
    expect(screen.queryByText("Unsaved changes")).toBeNull();
    await fireEvent.click(screen.getByLabelText("Hide panel after copying"));
    await fireEvent.click(screen.getByRole("button", { name: "Apply settings" }));
    await screen.findByText("All changes saved");
    expect(bridge.invoke.mock.calls.find(([name]) => name === "apply_settings")![1]).not.toHaveProperty("startAtLogin");
    expect(state.startupStatus.enabled).toBe(false);
    expect(state.startupStatus.desired).toBe(true);
  });

  it("an external revision retains a startup-only draft and requires Discard before saving", async () => {
    state.settingsRevision = "7";
    state.startupStatus = { available: true, enabled: false, desired: false, detail: "Actual startup is off" };
    render(App);
    await waitFor(() => expect(screen.getByRole("button", { name: "Open settings" }).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.click(screen.getByRole("tab", { name: "Behavior" }));
    await fireEvent.click(screen.getByLabelText("Start at login"));
    state.settingsRevision = "8";
    bridge.listen.mock.calls.find(([name]) => name === "settings-changed")![1]();
    await screen.findByText("Saved settings changed");
    expect((screen.getByLabelText("Start at login") as HTMLInputElement).checked).toBe(true);
    expect(screen.getByRole("button", { name: "Apply settings" }).hasAttribute("disabled")).toBe(true);
    await fireEvent.click(screen.getByRole("button", { name: "Discard changes" }));
    expect((screen.getByLabelText("Start at login") as HTMLInputElement).checked).toBe(false);
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel", "get_panel"]);
  });

  it("failed activation retains both startup and preference edits until an explicit Discard", async () => {
    state.startupStatus = { available: true, enabled: false, desired: false, detail: "Actual startup is off" };
    const normal = bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation((name, args) => name === "apply_settings" ? Promise.reject(new Error("Startup entry needs repair.")) : normal(name, args));
    render(App);
    await waitFor(() => expect(screen.getByRole("button", { name: "Open settings" }).hasAttribute("disabled")).toBe(false));
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.click(screen.getByRole("tab", { name: "Behavior" }));
    await fireEvent.click(screen.getByLabelText("Start at login"));
    await fireEvent.click(screen.getByLabelText("Show content previews"));
    await fireEvent.click(screen.getByRole("button", { name: "Apply settings" }));
    await screen.findByText("Startup entry needs repair.");
    expect(screen.queryByText("Settings applied.")).toBeNull();
    expect((screen.getByLabelText("Start at login") as HTMLInputElement).checked).toBe(true);
    expect((screen.getByLabelText("Show content previews") as HTMLInputElement).checked).toBe(false);
    await fireEvent.click(screen.getByRole("button", { name: "Discard changes" }));
    expect((screen.getByLabelText("Start at login") as HTMLInputElement).checked).toBe(false);
    expect((screen.getByLabelText("Show content previews") as HTMLInputElement).checked).toBe(true);
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
    await screen.findByText("The earlier share was accepted.");
    expect(screen.getByText("Your local clipboard was not read or changed.").closest("details")).toBeTruthy();
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
    state.settings.limits={max_item_bytes:1024,max_decoded_bytes:1024};
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
    await fireEvent.click(screen.getByRole("button",{name:"Refresh history"}));
    const changed = bridge.listen.mock.calls.find(([name])=>name==="operation-state")![1];
    changed({payload:{generation:"1",active:{id:"41",label:"Refreshing shared clipboard",cancellable:true,cancelling:false}}});
    await fireEvent.click(await screen.findByRole("button",{name:"Cancel operation"}));
    expect(bridge.invoke.mock.calls.find(([name])=>name==="cancel_operation")![1]).toEqual({id:"41"});
    await waitFor(()=>expect(screen.getByRole("button",{name:"Cancel operation"}).hasAttribute("disabled")).toBe(true));
    changed({payload:{generation:"3",active:null}});
    reject("Cancelled. This computer's clipboard was left unchanged.");
    await screen.findByRole("alert");
    expect(screen.queryByText("History refreshed.")).toBeNull();
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
    state.settings.limits = {max_item_bytes:67108864, max_decoded_bytes:268435456};
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
    state.settings.limits = {max_item_bytes:67108864,max_decoded_bytes:268435456};
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

  it("session limit edits remain a draft until Apply without network I/O", async () => {
    savedProfiles(); render(App); await screen.findByText("shared-hub");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.input(screen.getByLabelText("Item limit (bytes)"), { target: { value: "1024" } });
    expect(state.settings.limits.max_item_bytes).toBe(67108864);
    await fireEvent.click(screen.getByRole("button", { name: "Apply settings" }));
    await screen.findByText("Settings applied.");
    expect(state.settings.limits.max_item_bytes).toBe(1024);
    expect(bridge.invoke.mock.calls.map(([name])=>name)).toEqual(["get_operation","get_panel","apply_settings"]);
  });
  it("a history reload cancels an open removal confirmation instead of retargeting it", async () => {
    state.settings.target="hub";
    state.settings.limits={max_item_bytes:67108864,max_decoded_bytes:268435456};
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
    await fireEvent.click(screen.getByRole("tab", { name: "Behavior" }));
    expect(screen.queryByRole("button", { name: "Test notification" })).toBeNull();
    await fireEvent.change(screen.getByLabelText("Feedback after shortcut actions"), { target: { value: "all" } });
    expect(screen.getByRole("button", { name: "Test notification" }).hasAttribute("disabled")).toBe(true);
    await fireEvent.change(screen.getByLabelText("Feedback after shortcut actions"), { target: { value: "off" } });
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
    await fireEvent.click(screen.getByRole("tab", { name: "Behavior" }));
    await fireEvent.change(screen.getByLabelText("Feedback after shortcut actions"), { target: { value: "errors" } });
    await fireEvent.click(screen.getByRole("button", { name: "Test notification" }));
    await screen.findByText("Test notification submitted to the system.");
    expect((screen.getByLabelText("Feedback after shortcut actions") as HTMLSelectElement).value).toBe("errors");
    expect(state.settings.notifications).toBe("panel");
    expect(screen.getByText("Test submitted; delivery is not verified.")).toBeTruthy();
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel", "test_notification"]);
  });

  it("quiet-mode results survive reopening and repeated outcomes replace older UI feedback", async () => {
    state.operationNotice = { id: "1", message: "Shared clipboard copied to this computer.", failed: false };
    render(App);
    expect(await screen.findByRole("status", { name: "Transfer result" })).toHaveProperty("textContent", "Shared clipboard copied to this computer.");
    bridge.listen.mock.calls.find(([name]) => name === "operation-notice")![1]({ payload: "Older UI feedback" });
    await screen.findByText("Older UI feedback");
    state.operationNotice = { id: "2", message: "Shared clipboard copied to this computer.", failed: false };
    bridge.listen.mock.calls.find(([name]) => name === "settings-changed")![1]();
    await waitFor(() => expect(screen.getByRole("status", { name: "Transfer result" })).toHaveProperty("textContent", "Shared clipboard copied to this computer."));
    expect(screen.getByRole("alert").textContent).toContain("Older UI feedback");
    await fireEvent.click(screen.getByRole("button", { name: "Dismiss message" }));
    expect(screen.queryByText("Older UI feedback")).toBeNull();
    expect(bridge.invoke.mock.calls.map(([name]) => name)).toEqual(["get_operation", "get_panel", "get_panel"]);
  });

  it("direct shortcuts are opt-in, independently recorded drafts with no clipboard commands", async () => {
    state.settings.target = "hub";
    state.settings.directShareShortcut = "Control+Alt+KeyS";
    render(App);
    await screen.findByText("hub");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.click(screen.getByRole("tab", { name: "Shortcuts" }));
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
    await fireEvent.click(screen.getByRole("tab", { name: "Shortcuts" }));
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
    await fireEvent.click(screen.getByRole("tab", { name: "Shortcuts" }));
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
    expect(await screen.findByRole("status", { name: "Action result" })).toHaveProperty("textContent", "Direct shortcuts work while this app is in the background. Use the panel buttons here.");
    state.operationNotice = { id: "1", message: "Current clipboard shared.", failed: false };
    bridge.listen.mock.calls.find(([name]) => name === "settings-changed")![1]();
    await waitFor(() => expect(screen.getByRole("status", { name: "Transfer result" })).toHaveProperty("textContent", "Current clipboard shared."));
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
    await fireEvent.click(screen.getByRole("button", { name: "Back" }));
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
      { id: "default", name: "Default", target: "shared-hub", port: null, identityFile: "first/key", limits: { ...initialView.settings.limits }, transport: defaultTransport() },
      { id: "other", name: "Other account", target: "shared-hub", port: 2222, identityFile: "other/key", limits: { ...initialView.settings.limits }, transport: { ...defaultTransport(), timeoutMs: "15000", sshOptions: ["ServerAliveInterval=20"] } },
    ];
  }

  it("choosing a profile is a draft until Apply and does not fetch or copy an item", async () => {
    savedProfiles();
    state.item = { kind: "text", label: "Old item", text: "Old snapshot", imageUrl: null, encodedBytes: 12, createdAt: 1 };
    render(App);
    await screen.findByText("Old snapshot");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.change(screen.getByRole("combobox", { name: "Connection", exact: true }), { target: { value: "other" } });
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
    await fireEvent.click(screen.getByRole("button", { name: "Back" }));
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
    expect((screen.getByRole("combobox", { name: "Connection", exact: true }) as HTMLSelectElement).value).toBe("new-profile");
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
    await fireEvent.change(screen.getByRole("combobox", { name: "Connection", exact: true }), { target: { value: "other" } });
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

  it("advanced defaults remain a cancellable draft", async () => {
    savedProfiles(); state.settings.transport = { ...defaultTransport(), timeoutMs: "12000", sshOptions: ["ServerAliveInterval=30"] }; state.settings.limits.max_item_bytes=1024;
    render(App); await screen.findByText("shared-hub");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.click(screen.getByRole("button", { name: "Restore advanced defaults" }));
    expect((screen.getByLabelText("Timeout (milliseconds)") as HTMLInputElement).value).toBe("7000");
    expect((screen.getByLabelText("Item limit (bytes)") as HTMLInputElement).value).toBe("67108864");
    await fireEvent.click(screen.getByRole("button", { name: "Discard changes" }));
    expect((screen.getByLabelText("Timeout (milliseconds)") as HTMLInputElement).value).toBe("12000");
    expect((screen.getByLabelText("Item limit (bytes)") as HTMLInputElement).value).toBe("1024");
    expect(bridge.invoke.mock.calls.map(([name])=>name)).toEqual(["get_operation","get_panel"]);
  });
  it("a preference-only edit preserves the saved session policy", async () => {
    savedProfiles();
    state.settings.transport.timeoutMs = "300000";
    render(App);
    await screen.findByText("shared-hub");
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.click(screen.getByRole("tab", { name: "Behavior", exact: true }));
    await fireEvent.click(screen.getByLabelText("Show content previews"));
    await fireEvent.click(screen.getByRole("button", { name: "Apply settings" }));
    await screen.findByText("Settings applied.");
    const transport = bridge.invoke.mock.calls.find(([name]) => name === "apply_settings")![1].settings.transport;
    expect(transport.timeoutMs).toBe("300000");
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
    await fireEvent.click(screen.getByRole("button", { name: "Refresh history" }));
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
    expect(screen.getByText("All changes saved")).toBeTruthy();
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
    await fireEvent.click(screen.getByRole("button", { name: "Refresh history" }));
    await screen.findByText("Server clipboard is empty");
    expect(screen.queryByText("No shared item loaded")).toBeNull();
    expect(screen.getByRole("button", { name: "Copy to this computer" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("button", { name: "Share current clipboard" }).hasAttribute("disabled")).toBe(false);
    await fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await fireEvent.input(screen.getByLabelText("SSH alias or user@host"), { target: { value: "other-hub" } });
    await fireEvent.click(screen.getByRole("button", { name: "Apply settings" }));
    await screen.findByText("Settings applied.");
    await fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(screen.queryByText("Server clipboard is empty")).toBeNull();
    expect(screen.getByText("No shared item loaded")).toBeTruthy();
  });

  it("a failed refresh cannot claim the server clipboard is empty", async () => {
    state.settings.target = "hub";
    const normal = bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation((name, args) => name === "refresh_item" ? Promise.reject("Server unavailable") : normal(name, args));
    render(App);
    await screen.findByText("No shared item loaded");
    await fireEvent.click(screen.getByRole("button", { name: "Refresh history" }));
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
    await fireEvent.click(screen.getByRole("button", { name: "Back" }));
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
