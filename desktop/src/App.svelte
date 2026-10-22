<script lang="ts">
  import { onMount, tick } from "svelte";
  import { cloneSettings, cloneTransport, command, defaultTransport, initialView, subscribe, type OperationView, type PanelView, type Settings, type UpdateView } from "./api";
  import { displayShortcut, shortcutFromEvent } from "./shortcut.js";
  import { conciseUpdateNotes } from "./update-notes";
  import { exactTime, expiryTime, formatSize, isoTime, receivedTime } from "./history-format";

  let view = $state<PanelView>(initialView);
  let draft = $state<Settings>(cloneSettings(initialView.settings));
  let startupDraft = $state<boolean | null>(null);
  let sshOptionsText = $state("");
  let page = $state("panel");
  let section = $state("connection");
  let localBusy = $state(false);
  let operation = $state<OperationView>({ generation: "0", active: null });
  const busy = $derived(localBusy || operation.active !== null);
  let routine = $state<{ text: string; scope: string } | null>(null);
  let failure = $state<{ text: string; origin: string } | null>(null);
  let routineTimer: ReturnType<typeof setTimeout> | undefined;
  let routineVersion = 0;
  let navigationVersion = 0;
  type ShortcutField = "openShortcut" | "directShareShortcut" | "directCopyShortcut";
  let recording = $state<ShortcutField | null>(null);
  let recordingRequested = $state(false);
  let recordingEpoch = 0;
  let recordingSync = Promise.resolve();
  let removingProfile = $state(false);
  let confirmingHistory = $state<{ kind: "delete" | "clear"; epoch: string; itemId: string | null } | null>(null);
  let confirmingInstall = $state<string | null>(null);
  let ready = $state(false);
  let content = $state<HTMLElement>();
  let advancedOpen = $state(false);
  let draftRevision = $state<string | null>(null);
  let reloadPending = false;
  let reloading = false;
  let disposed = false;
  let operationEpoch = 0;
  let focusVersion = 0;
  let clock = $state(Date.now());
  let documentVisible = $state(true);
  let historyAnnouncement = $state("");
  const settingsSections = [["connection", "Connection"], ["shortcuts", "Shortcuts"], ["behavior", "Behavior"], ["updates", "Updates"]];

  $effect(() => {
    if (!ready || page !== "panel" || !documentVisible || !view.history?.items.length) return;
    clock = Date.now();
    const timer = setInterval(() => clock = Date.now(), 30000);
    return () => clearInterval(timer);
  });

  async function focusAfterRender(selector: string, onlyIfBody = false) {
    const version = ++focusVersion;
    await tick();
    if (disposed || version !== focusVersion || (onlyIfBody && document.activeElement !== document.body)) return;
    const element = document.querySelector<HTMLElement>(selector);
    if (element && !element.matches(":disabled")) element.focus();
    else content?.focus();
  }

  function captureRemovableFocus() {
    const element = document.activeElement;
    if (!(element instanceof HTMLElement) || !content?.contains(element)) return null;
    const row = element.closest<HTMLElement>("[data-history-id]");
    return row || element.closest(".history, [data-confirmation], [data-operation-progress]") ? { element, history: !!row } : null;
  }

  async function restoreRemovedFocus(previous: ReturnType<typeof captureRemovableFocus>) {
    if (!previous) return;
    await tick();
    if (disposed || previous.element.isConnected || (document.activeElement !== document.body && document.activeElement?.isConnected)) return;
    if (previous.history) historyAnnouncement = "The focused history item is no longer available.";
    await focusAfterRender(page === "settings" ? `#settings-tab-${section}` : view.history ? "#history-heading" : "#main-content", true);
  }

  function acceptUpdate(next: UpdateView) {
    if (disposed) return;
    const previousFocus = captureRemovableFocus();
    view = { ...view, updateStatus: next };
    if (next.phase !== "ready" || confirmingInstall !== next.offeredVersion) confirmingInstall = null;
    void restoreRemovedFocus(previousFocus);
  }

  const configured = $derived(view.settings.target.trim().length > 0);
  const dirty = $derived(startupDraft !== null || JSON.stringify(draft) !== JSON.stringify(view.settings));
  const staleDraft = $derived(dirty && draftRevision !== view.settingsRevision);
  const applyDisabled = $derived(!ready || !view.settingsWritable || !dirty || staleDraft || busy || recordingRequested || (startupDraft !== null && (!view.startupStatus.available || view.startupStatus.enabled === null)));
  const selectedHistory = $derived(view.history?.items.find((item) => item.id === view.history?.selectedId));
  const canCopy = $derived(!!view.item || !!selectedHistory);
  const updateNotes = $derived(conciseUpdateNotes(view.updateStatus.notes, view.updateStatus.offeredVersion));
  const feedbackScope = $derived(page === "settings" ? section : "panel");
  const notificationMode = $derived(draft.notifications === "all" || draft.notifications === "errors");
  const startupHelp = $derived(startupDraft !== null && view.startupStatus.enabled !== null ? `Currently ${view.startupStatus.enabled ? "on" : "off"}.` : !view.startupStatus.available || view.startupStatus.enabled === null || view.startupStatus.enabled !== view.startupStatus.desired ? view.startupStatus.detail : view.startupStatus.enabled ? "Starts in the background." : "");
  const historyHealthy = $derived(view.historyStatus?.phase === "live");
  const connectionLabel = $derived(busy ? "Working…" : view.historyStatus ? ({live:"History up to date",connecting:"Connecting…",reconnecting:"Reconnecting…",needs_action:"Connection needs attention"}[view.historyStatus.phase]) : view.connected ? "Server reachable" : configured ? "Not connected" : "Setup needed");
  const resultParts = $derived(splitResult(view.operationNotice?.message ?? ""));

  function requestHistoryRemoval(kind: "delete" | "clear") {
    if (!view.history?.epoch) return;
    confirmingHistory = { kind, epoch: view.history.epoch, itemId: view.history.selectedId };
    void focusAfterRender("#history-confirmation button");
  }

  function cancelHistoryRemoval() {
    const kind = confirmingHistory?.kind;
    confirmingHistory = null;
    void focusAfterRender(kind === "clear" ? "#clear-history" : "#delete-selected");
  }

  function toggleProfileRemoval() {
    removingProfile = !removingProfile;
    void focusAfterRender(removingProfile ? "#profile-confirmation button" : "#remove-profile");
  }

  function installConfirmation(open: boolean) {
    confirmingInstall = open ? view.updateStatus.offeredVersion : null;
    void focusAfterRender(open ? "#install-confirmation button" : "#install-update");
  }

  function confirmHistoryRemoval() {
    const confirmation = confirmingHistory;
    if (!confirmation) return;
    void perform(confirmation.kind === "clear" ? "clear_history" : "delete_item", confirmation.kind === "clear" ? "History cleared." : "Item deleted.", { epoch: confirmation.epoch, ...(confirmation.kind === "delete" ? { itemId: confirmation.itemId } : {}) });
  }

  function clearRoutine() {
    routineVersion++;
    clearTimeout(routineTimer);
    routineTimer = undefined;
    routine = null;
  }

  function inform(text: string, failed = false, origin = "", scope = feedbackScope) {
    if (disposed) return;
    clearRoutine();
    if (!text) return;
    if (failed) { failure = { text, origin }; return; }
    routine = { text, scope };
    const version = routineVersion;
    routineTimer = setTimeout(() => {
      if (!disposed && version === routineVersion) clearRoutine();
    }, 5000);
  }

  function splitResult(text: string) {
    const match = text.match(/^(.+?[.!?])\s+([\s\S]+)$/);
    return { summary: match?.[1] ?? text, detail: match?.[2] ?? "" };
  }

  function acceptOperation(next: OperationView) {
    if (disposed || !next || !/^(0|[1-9][0-9]*)$/.test(next.generation)) return;
    if (BigInt(next.generation) < BigInt(operation.generation)) return;
    const previousFocus = captureRemovableFocus();
    operation = next;
    void restoreRemovedFocus(previousFocus);
    if (!next.active && reloadPending && !localBusy) void reloadSavedSettings();
  }

  async function cancelOperation() {
    const active = operation.active;
    if (!active?.cancellable) return;
    try { acceptOperation(await command<OperationView>("cancel_operation", { id: active.id })); }
    catch (cause) { inform(String(cause), true, "cancel_operation"); }
  }

  function discardDraft() { draft = cloneSettings(view.settings); startupDraft = null; sshOptionsText = draft.transport.sshOptions.join("\n"); draftRevision = view.settingsRevision; stopRecording(); removingProfile = false; }

  function applyDraft() {
    if (applyDisabled) return;
    void perform("apply_settings", "Settings applied.", { settings: cloneSettings(draft), expectedRevision: draftRevision, ...(startupDraft === null ? {} : { startAtLogin: startupDraft }) });
  }

  function revealInvalidField(event: Event) {
    const details = (event.target as HTMLElement).closest("details");
    if (details) { advancedOpen = true; details.open = true; }
  }

  async function revealAdvancedError(detail: string) {
    // Backend validation remains authoritative, including decimal-string limits.
    const fields: [RegExp, string][] = [
      [/SSH port/i, "port"], [/identity file/i, "identity"], [/SSH option/i, "ssh-options"],
      [/timeout/i, "timeout"], [/Transfer limit/i, "transfer-limit"], [/Framing recovery budget/i, "resync-budget"],
      [/item.*limit/i, "shelf-item-limit"], [/decoded.*limit/i, "shelf-decode-limit"],
    ];
    const field = fields.find(([pattern]) => pattern.test(detail))?.[1];
    if (!field) return false;
    section = "connection";
    advancedOpen = true;
    await tick();
    const element = content?.querySelector<HTMLElement>(`#${field}`);
    element?.focus();
    return !!element && document.activeElement === element;
  }

  function discardChanges() { discardDraft(); inform("Changes discarded."); void focusAfterRender(page === "settings" ? `#settings-tab-${section}` : "#open-settings"); }

  function syncRecording(active: boolean) {
    recordingSync = recordingSync.catch(() => {}).then(() => command<void>("set_shortcut_recording", { active }));
    return recordingSync;
  }

  function stopRecording() {
    recordingEpoch++;
    recording = null;
    if (!recordingRequested) return;
    recordingRequested = false;
    void syncRecording(false).catch(() => inform("Shortcut recording could not be reset. Quit and reopen before using direct shortcuts.", true));
  }

  async function startRecording(field: ShortcutField) {
    if (!ready || busy) return;
    if (recording === field) { stopRecording(); return; }
    const epoch = ++recordingEpoch;
    recording = null;
    recordingRequested = true;
    try {
      await syncRecording(true);
      if (epoch === recordingEpoch && !disposed) recording = field;
    } catch (cause) {
      if (epoch === recordingEpoch) { stopRecording(); inform(String(cause), true); }
    }
  }

  function chooseProfile(id: string) {
    if (dirty || busy || !ready) return;
    const profile = view.profiles.find((profile) => profile.id === id);
    if (!profile) return;
    draft = { ...draft, profileId: profile.id, profileName: profile.name, target: profile.target, port: profile.port, identityFile: profile.identityFile, transport: cloneTransport(profile.transport), endpoint: { ...profile.endpoint } };
    sshOptionsText = draft.transport.sshOptions.join("\n");
    removingProfile = false;
    inform("");
  }

  function addProfile() {
    if (dirty || busy || !ready) return;
    draft = { ...draft, profileId: null, profileName: "New connection", target: "", port: null, identityFile: "", transport: defaultTransport(), endpoint: { protocol: "legacy" } };
    sshOptionsText = "";
    removingProfile = false;
    inform("");
  }

  function editSshOptions(text: string) {
    sshOptionsText = text;
    draft.transport.sshOptions = text.split(/\r?\n/).filter((option) => option.trim().length > 0);
  }

  function resetTransport() {
    draft.transport = defaultTransport();
    sshOptionsText = "";
    inform("Advanced defaults restored in your draft.");
  }

  async function reloadSavedSettings() {
    if (disposed) return;
    if (busy || reloading) { reloadPending = true; return; }
    reloadPending = false;
    reloading = true;
    const epoch = operationEpoch;
    try {
      const next = await command<PanelView>("get_panel");
      if (disposed || epoch !== operationEpoch) return;
      const retainDraft = dirty && (startupDraft !== null || JSON.stringify(draft) !== JSON.stringify(next.settings));
      if (JSON.stringify(next.operationNotice) !== JSON.stringify(view.operationNotice)) clearRoutine();
      const previousFocus = captureRemovableFocus();
      view = next;
      removingProfile = false;
      confirmingHistory = null;
      if (retainDraft && draftRevision !== next.settingsRevision) inform("Saved settings changed. Discard your draft to load them before editing again.", true);
      else if (!retainDraft) discardDraft();
      await restoreRemovedFocus(previousFocus);
    } catch (cause) { if (!disposed) inform(String(cause), true); }
    finally {
      reloading = false;
      if (reloadPending && !busy && !disposed) void reloadSavedSettings();
    }
  }

  async function perform(name: string, success: string, args?: Record<string, unknown>) {
    if (busy || !ready) return;
    const hadDraft = dirty;
    const scope = feedbackScope;
    const navigation = navigationVersion;
    const invoker = document.activeElement;
    let failed = false;
    let validationDetail: string | null = null;
    localBusy = true;
    operationEpoch++;
    inform("");
    try {
      const guardedArgs = ["test_connection", "refresh_item", "share_clipboard", "copy_item", "remove_profile", "select_item", "delete_item", "clear_history", "reconnect_history", "recover_share", "discard_share", "acknowledge_migration_summary"].includes(name) ? { ...args, expectedRevision: view.settingsRevision } : args;
      confirmingHistory = null;
      view = await command<PanelView>(name, guardedArgs);
      if (name === "apply_settings" || name === "remove_profile" || name === "acknowledge_migration_summary") discardDraft();
      if (failure?.origin === name) failure = null;
      if (navigation === navigationVersion && !(["copy_item", "share_clipboard", "recover_share"].includes(name) && view.operationNotice)) {
        inform(name === "refresh_item" && !view.item ? "Checked the server. No shared item is available." : success, false, name, scope);
      }
      if (name === "select_item" && content) content.scrollTop = 0;
    } catch (cause) {
      failed = true;
      const detail = cause instanceof Error ? cause.message : String(cause);
      inform(detail, true, name);
      if (name === "apply_settings") validationDetail = detail;
      // Reflect connection/registration state even after a failed operation.
      try { view = await command<PanelView>("get_panel"); if (!hadDraft) discardDraft(); } catch { /* Keep the last view. */ }
    } finally {
      localBusy = false;
      if (navigation === navigationVersion) {
        const fieldFocused = validationDetail ? await revealAdvancedError(validationDetail) : false;
        if (failed && !fieldFocused) await focusAfterRender("#action-error");
        else if (!failed && name === "select_item") await focusAfterRender("#shared-item-heading");
        else if (!failed && name === "apply_settings") await focusAfterRender(`#settings-tab-${section}`);
        else if (!failed && ["delete_item", "clear_history"].includes(name)) await focusAfterRender("#history-heading");
        else if (!failed && name === "remove_profile") await focusAfterRender("#profile");
        else if (!failed) {
          await tick();
          if (document.activeElement === document.body) {
            if (invoker instanceof HTMLElement && invoker.isConnected && !invoker.matches(":disabled")) invoker.focus();
            else await focusAfterRender(page === "settings" ? `#settings-tab-${section}` : "#main-content");
          }
        }
      }
      if (reloadPending) void reloadSavedSettings();
    }
  }

  async function updateAction(name: "check_updates" | "download_update" | "install_update", version?: string) {
    if (!ready || busy || view.updateStatus.busy) return;
    if (name === "install_update" && (dirty || view.pendingShare)) return;
    const navigation = navigationVersion;
    const invoker = document.activeElement;
    let failed = false;
    localBusy = true;
    operationEpoch++;
    confirmingInstall = null;
    inform("");
    try {
      const next = await command<UpdateView | void>(name, version ? { version } : undefined);
      if (next) view = { ...view, updateStatus: next };
    } catch (cause) { failed = true; inform(String(cause), true); }
    finally {
      localBusy = false;
      if (navigation === navigationVersion) {
        if (failed) await focusAfterRender("#action-error");
        else if (name === "download_update" && view.updateStatus.phase === "ready") await focusAfterRender("#install-update");
        else { await tick(); if (invoker instanceof HTMLElement && invoker.isConnected && !invoker.matches(":disabled") && document.activeElement === document.body) invoker.focus(); }
      }
      if (reloadPending) void reloadSavedSettings();
    }
  }

  function navigate(next: string) {
    if (busy) return;
    const returning = page === "settings";
    navigationVersion++;
    clearRoutine();
    stopRecording();
    removingProfile = false;
    confirmingHistory = null;
    confirmingInstall = null;
    page = next;
    if (content) content.scrollTop = 0;
    void focusAfterRender(next === "settings" ? `#settings-tab-${section}` : returning ? "#open-settings" : "#main-content");
  }

  function selectSection(next: string) {
    navigationVersion++;
    clearRoutine();
    section = next;
    confirmingInstall = null;
    stopRecording();
    removingProfile = false;
    if (content) content.scrollTop = 0;
    void focusAfterRender(`#settings-tab-${section}`);
  }

  function tabKey(event: KeyboardEvent, id: string) {
    if (recordingRequested || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
    const index = settingsSections.findIndex(([key]) => key === id);
    const next = event.key === "ArrowRight" ? (index + 1) % settingsSections.length : event.key === "ArrowLeft" ? (index + settingsSections.length - 1) % settingsSections.length : event.key === "Home" ? 0 : event.key === "End" ? settingsSections.length - 1 : -1;
    if (next < 0) return;
    event.preventDefault();
    selectSection(settingsSections[next][0]);
  }

  function handleKey(event: KeyboardEvent) {
    if (recordingRequested) {
      event.preventDefault();
      if (event.repeat) return;
      if (event.key === "Escape") { stopRecording(); return; }
      const shortcut = shortcutFromEvent(event);
      if (shortcut) recordShortcut(shortcut);
      return;
    }
    if (event.key === "Escape" && !busy) {
      if (confirmingHistory) cancelHistoryRemoval();
      else if (removingProfile) toggleProfileRemoval();
      else if (confirmingInstall) installConfirmation(false);
      else if (page === "settings") navigate("panel");
      else void command("hide_panel").catch((cause) => inform(String(cause), true));
    }
  }

  function recordShortcut(binding: string) {
    if (!recording || busy || !ready) return;
    draft[recording] = binding;
    stopRecording();
  }

  onMount(() => {
    const visibility = () => documentVisible = document.visibilityState !== "hidden";
    visibility();
    document.addEventListener("visibilitychange", visibility);
    let stop = () => {};
    void (async () => {
      try {
        // Subscribe before taking the initial snapshot so startup changes cannot be missed.
        const unsubscribe = await subscribe(navigate, inform, () => {
          if (!ready) reloadPending = true; else void reloadSavedSettings();
        }, (binding, openPanel) => {
          if (recording) recordShortcut(binding);
          else if (openPanel) navigate("panel");
          else inform("Direct shortcuts work while this app is in the background. Use the panel buttons here.");
        }, acceptOperation, acceptUpdate);
        if (disposed) unsubscribe(); else stop = unsubscribe;
      } catch { inform("Could not listen for tray or settings changes. Reopen through the launcher.", true); }
      if (disposed) return;
      try {
        acceptOperation(await command<OperationView>("get_operation"));
        const next = await command<PanelView>("get_panel");
        if (disposed) return;
        view = next; discardDraft(); ready = true;
        void focusAfterRender(page === "settings" ? `#settings-tab-${section}` : "#main-content", true);
        if (reloadPending) void reloadSavedSettings();
      } catch (cause) { inform(cause instanceof Error ? cause.message : String(cause), true); }
    })();
    return () => { disposed = true; focusVersion++; document.removeEventListener("visibilitychange", visibility); clearRoutine(); stopRecording(); stop(); };
  });
</script>

<svelte:window onkeydown={handleKey} onblur={stopRecording} />

{#snippet feedback()}
  {#if failure}<div id="action-error" tabindex="-1" class="feedback error" role="alert"><span>{failure.text}</span><button type="button" class="text-button" aria-label="Dismiss message" onclick={() => { failure = null; void focusAfterRender(page === "settings" ? `#settings-tab-${section}` : "#main-content"); }}>Dismiss</button></div>{/if}
{/snippet}

{#snippet routineFeedback()}
  {#if routine && routine.scope === feedbackScope}<p class="action-feedback" role="status" aria-label="Action result">{routine.text}</p>{/if}
{/snippet}

{#snippet operationProgress()}
  {#if operation.active}
    <div data-operation-progress class="feedback operation-progress" role="status">
      <span>{operation.active.label}… {operation.active.cancelling ? "Stopping…" : !operation.active.cancellable ? "Finishing…" : ""}</span>
      <button type="button" disabled={!operation.active.cancellable} onclick={cancelOperation}>Cancel operation</button>
    </div>
  {/if}
{/snippet}

<div class="app-shell" class:settings-open={page === "settings"}>
  <header>
    <div class="brand-mark" aria-hidden="true">↔</div>
    <div class="brand"><h1>SSH Clipboard</h1><span>{configured ? view.settings.target : "Connect your machines"}</span></div>
    {#if page === "panel"}
      <button id="open-settings" class="icon-button" aria-label="Open settings" onclick={() => navigate("settings")} disabled={!ready || busy}>⚙</button>
    {:else}
      <button type="button" class="text-button" onclick={() => navigate("panel")} disabled={!ready || busy}>Back</button>
    {/if}
  </header>

  {#if page === "settings"}
    <section class="settings-navigation" aria-label="Settings navigation">
      <h2>Settings</h2>
      <div class="settings-tabs" role="tablist" aria-label="Settings sections">
        {#each settingsSections as [id, label]}
          <button type="button" role="tab" id={`settings-tab-${id}`} class:active={section === id} aria-selected={section === id} aria-controls="settings-panel" tabindex={section === id ? 0 : -1} onkeydown={(event) => tabKey(event, id)} onclick={() => selectSection(id)}>{label}</button>
        {/each}
      </div>
    </section>
  {/if}

  <main id="main-content" tabindex="-1" aria-label={page === "panel" ? "Clipboard panel" : "Settings content"} bind:this={content}>
    {#if page !== "settings" || section !== "updates"}{@render operationProgress()}{/if}
    {#if view.settingsNotice}<div class="feedback error" role="alert">{view.settingsNotice}</div>{/if}
    {#if view.migrationSummary}
      <section class="migration-summary" aria-labelledby="migration-heading">
        <h2 id="migration-heading">Review imported settings</h2>
        <p>Your existing settings were imported and a recovery backup was saved. These are your current saved choices.</p>
        <dl>
          <dt>Connection</dt><dd>{view.settings.profileName} — {view.settings.target || "Setup needed"}</dd>
          <dt>This computer</dt><dd>{view.migrationSummary.deviceName}</dd>
          <dt>Open panel</dt><dd>{displayShortcut(view.settings.openShortcut, view.platform)}{!view.settings.shortcutEnabled ? " (disabled)" : ""}</dd>
          <dt>Saved start-at-login choice</dt><dd>{view.migrationSummary.startAtLogin ? "On" : "Off"}. Review Behavior to change this app's startup entry.</dd>
          <dt>Direct shortcuts</dt><dd>{view.settings.directShortcutsEnabled ? "Enabled" : "Disabled"}. Share: {view.settings.directShareShortcut ? displayShortcut(view.settings.directShareShortcut, view.platform) : "Not set"}; Copy: {view.settings.directCopyShortcut ? displayShortcut(view.settings.directCopyShortcut, view.platform) : "Not set"}.</dd>
        </dl>
        <p>Your old agent and its startup entry were left in place. Quit it before enabling direct shortcuts here; keep only one app enabled at login when you switch.</p>
        <details><summary>Recovery backup</summary><p>{view.migrationSummary.backupPath}</p></details>
        <button type="button" disabled={!ready || busy || dirty || !view.settingsWritable} onclick={() => perform("acknowledge_migration_summary", "Import summary dismissed.")}>Got it</button>
        {#if dirty}<p class="field-help">Apply or discard edits before dismissing this summary.</p>{/if}
      </section>
    {/if}
    {#if page === "panel"}
      {#if dirty}
        <section class="draft-notice" aria-label="Unsaved settings">
          <strong>{staleDraft ? "Saved settings changed" : "Unsaved settings"}</strong>
          <p>{staleDraft ? "Discard your draft to load the saved settings." : "Changes take effect after Apply."}</p>
          <div class="draft-actions">
            <button type="button" class="text-button" disabled={!ready || busy} onclick={() => navigate("settings")}>Return to settings</button>
            <button type="button" class="text-button" disabled={busy} onclick={discardChanges}>Discard changes</button>
          </div>
        </section>
      {/if}
      <div class="status-line" role="status" aria-label="Connection status"><span aria-hidden="true" class:connected={view.connected && (!view.historyStatus || historyHealthy)} class="status-dot"></span>{connectionLabel}<span class="mode">{view.settings.profileName}</span></div>
      {#if view.historyStatus}
        {#if historyHealthy}
          <details class="connection-details"><summary>Connection details</summary><p>{view.historyStatus.detail}</p></details>
        {:else}
          <div class="history-connection" aria-label="History connection">
            <p>{view.historyStatus.detail}{view.historyStatus.retryInMs !== null ? ` Retrying in ${Math.ceil(view.historyStatus.retryInMs / 1000)} seconds.` : ""}</p>
            {#if view.historyStatus.phase === "needs_action" || view.historyStatus.phase === "reconnecting"}<button type="button" class="text-button" disabled={!ready || busy} onclick={() => perform("reconnect_history", "Reconnecting history…")}>Reconnect history</button>{/if}
          </div>
        {/if}
      {/if}
      {#if !configured}
        <section class="empty-state">
          <h2>Your clipboard, across machines</h2>
          <p>Connect to your SSH clipboard server. Then share from one computer and copy on another.</p>
          <button class="primary" onclick={() => navigate("settings")}>Set up connection</button>
        </section>
      {:else if view.item}
        <section class="item-card" aria-label="Shared item">
          <div class="item-top"><span class="kind">{view.item.kind === "image" ? "Image" : "Text"}</span><span>{formatSize(view.item.encodedBytes)}</span></div>
          <h2 id="shared-item-heading" tabindex="-1">{view.item.label}</h2>
          {#if view.item.imageUrl}<img class="preview-image" src={view.item.imageUrl} alt="Shared clipboard preview" />
          {:else if view.item.text !== null}<pre>{view.item.text || "(Empty text)"}</pre>
          {:else}<p class="muted">{view.settings.showPreview ? "Refresh or select this item to load its preview." : "Content preview is hidden."}</p>{/if}
          <div class="item-meta"><time datetime={isoTime(view.item.createdAt)}>{exactTime(view.item.createdAt)}</time><span>{view.history ? selectedHistory?.source ?? "Unknown source device" : "Source device unavailable on this server"}</span></div>
        </section>
      {:else if selectedHistory}
        <section class="item-card" aria-label="Selected history item"><h2 id="shared-item-heading" tabindex="-1">{selectedHistory.label}</h2><p class="muted">Select this item in history to load its preview, or copy it to this computer.</p></section>
      {:else if view.clipboardChecked}
        <section class="empty-state compact"><h2>Server clipboard is empty</h2><p>Copy text or an image, then choose Share current clipboard.</p></section>
      {:else}
        <section class="empty-state compact"><h2>No shared item loaded</h2><p>Refresh to load a shared item, or share your current clipboard.</p></section>
      {/if}

      <div class="actions">
        <button class:primary={canCopy} disabled={!ready || !canCopy || busy} onclick={() => perform("copy_item", "Copied to this computer. You can paste it now.")}>Copy to this computer</button>
        <button class:primary={!canCopy} disabled={!ready || !configured || busy || view.pendingShare !== null} onclick={() => perform("share_clipboard", "Shared your clipboard with the server.")}>Share current clipboard</button>
        {#if !view.history}<button class="text-button" disabled={!ready || !configured || busy} onclick={() => perform("refresh_item", "Shared clipboard refreshed.")}>Refresh shared clipboard</button>{/if}
      </div>
      {@render feedback()}
      {@render routineFeedback()}
      {#if view.operationNotice && view.operationNotice.message !== failure?.text}
        <section class:error={view.operationNotice.failed} class="transfer-result" aria-label="Latest transfer result">
          <p role={view.operationNotice.failed ? "alert" : "status"} aria-label="Transfer result">{resultParts.summary}</p>
          {#if resultParts.detail}<details open={view.operationNotice.failed}><summary>Details</summary><p>{resultParts.detail}</p></details>{/if}
        </section>
      {/if}
      {#if view.pendingShare}
        <section class="share-recovery" aria-label="Share recovery">
          <h2>Check an earlier share</h2>
          <p>It may already have reached the server. Check its status first. Retry sends the saved capture without reading your current clipboard.</p>
          <div class="recovery-actions">
            <button type="button" disabled={!ready || busy} onclick={() => perform("recover_share", "Share status checked.", {operationId:view.pendingShare?.id,retry:false})}>Check share status</button>
            <button type="button" disabled={!ready || busy || !view.pendingShare.canRetry} onclick={() => perform("recover_share", "Captured share checked.", {operationId:view.pendingShare?.id,retry:true})}>Retry captured share</button>
            <button type="button" disabled={!ready || busy} onclick={() => perform("discard_share", "Recovery data discarded. The server's clipboard was not changed.", {operationId:view.pendingShare?.id})}>Discard recovery data</button>
          </div>
          {#if !view.pendingShare.canRetry}<p>The safe retry window has expired or the server changed. Status checks remain available.</p>{/if}
          <p>Discarding does not undo a server share. Leaving this profile or quitting clears recovery data.</p>
        </section>
      {/if}
      {#if configured && view.history}
        <section class="history" aria-label="Temporary clipboard history">
          <div class="history-heading"><h2 id="history-heading" tabindex="-1">Temporary history <span>{view.history.items.length}</span></h2><div class="history-tools"><button type="button" class="text-button" disabled={!ready || busy} onclick={() => perform("refresh_item", "History refreshed.")}>Refresh history</button><button id="clear-history" type="button" class="text-button" disabled={!ready || busy || !view.history.items.length} onclick={() => requestHistoryRemoval("clear")}>Clear history…</button></div></div>
          {#if historyAnnouncement}<p class="sr-only" role="status" aria-label="History selection">{historyAnnouncement}</p>{/if}
          {#if view.history.items.length}
            <ul class="history-list">
              {#each view.history.items as item (`${view.history.epoch}:${item.id}`)}
                <li data-history-id={item.id} class:selected={item.id === view.history.selectedId}>
                  <button type="button" aria-pressed={item.id === view.history.selectedId} aria-describedby={`history-time-${item.id}`} disabled={!ready || busy} onclick={() => { historyAnnouncement = ""; void perform("select_item", "Item selected. Your clipboard is unchanged.", { itemId: item.id, epoch: view.history?.epoch }); }}>
                    <span class="history-label">{item.label}</span><span class="history-detail">{item.source ?? "Unknown source"} · {formatSize(item.encodedBytes)}</span>
                  </button>
                  <div class="history-times">
                    <span id={`history-time-${item.id}`} aria-live="off">{receivedTime(item.receivedAt, clock)} · {expiryTime(item.expiresAt, clock)}</span>
                    <details><summary>Exact times<span class="sr-only"> for {item.label}</span></summary><dl><dt>Received</dt><dd><time datetime={isoTime(item.receivedAt)}>{exactTime(item.receivedAt)}</time></dd><dt>Expires</dt><dd><time datetime={isoTime(item.expiresAt)}>{exactTime(item.expiresAt)}</time></dd></dl></details>
                  </div>
                </li>
              {/each}
            </ul>
            <button id="delete-selected" class="text-button" disabled={!ready || busy || !selectedHistory} onclick={() => requestHistoryRemoval("delete")}>Delete selected item…</button>
          {:else}<p class="field-help">Items expire automatically and are cleared when the server restarts.</p>{/if}
          {#if confirmingHistory}
            <div id="history-confirmation" data-confirmation class="remove-confirmation" role="group" aria-label="History removal confirmation">
              <p>{confirmingHistory.kind === "clear" ? "Clear all items from this server's temporary history?" : "Delete the selected item from the server?"} Copies already saved on other computers remain available.</p>
              <div class="profile-actions"><button type="button" disabled={busy} onclick={confirmHistoryRemoval}>{confirmingHistory.kind === "clear" ? "Clear history" : "Delete item"}</button><button type="button" disabled={busy} onclick={cancelHistoryRemoval}>Cancel</button></div>
            </div>
          {/if}
        </section>
      {/if}
    {:else}
      <div id="settings-panel" role="tabpanel" aria-labelledby={`settings-tab-${section}`}>
      <form id="settings-form" oninvalidcapture={revealInvalidField} onsubmit={(event) => { event.preventDefault(); applyDraft(); }}>
        {#if section === "connection"}
          <label for="profile">Connection</label>
          <select id="profile" aria-describedby="profile-help" value={draft.profileId ?? ""} onchange={(event) => chooseProfile(event.currentTarget.value)} disabled={!ready || !view.settingsWritable || dirty || busy}>
            {#each view.profiles as profile (profile.id)}<option value={profile.id}>{profile.name} — {profile.target || "Setup needed"}</option>{/each}
            {#if draft.profileId === null}<option value="">New connection</option>{/if}
            {#if draft.profileId !== null && !view.profiles.some((profile) => profile.id === draft.profileId)}<option value={draft.profileId}>{draft.profileName} (removed)</option>{/if}
          </select>
          <p id="profile-help" class="field-help">Choose a connection, then Apply to use it. Apply or discard your draft before choosing another.</p>
          <div class="profile-actions">
            <button type="button" disabled={!ready || !view.settingsWritable || !view.settingsSaved || dirty || busy} onclick={addProfile}>Add connection</button>
            <button id="remove-profile" type="button" disabled={!ready || !view.settingsWritable || !view.settingsSaved || view.profiles.length < 2 || dirty || busy} onclick={toggleProfileRemoval}>Remove…</button>
          </div>
          {#if removingProfile}
            <div id="profile-confirmation" data-confirmation class="remove-confirmation" role="group" aria-label="Remove connection confirmation">
              <p>Remove “{view.settings.profileName}” from this computer? Another saved connection will become active. The server and its clipboard are kept.</p>
              <div class="profile-actions"><button type="button" disabled={!view.settingsWritable || dirty || busy} onclick={() => perform("remove_profile", "Connection removed. Another saved connection is now active.", { profileId: view.settings.profileId })}>Remove connection</button><button type="button" disabled={busy} onclick={toggleProfileRemoval}>Cancel</button></div>
            </div>
          {/if}
          <label for="profile-name">Connection name</label><input id="profile-name" bind:value={draft.profileName} autocomplete="off" disabled={!ready || busy} />
          <label for="target">SSH alias or user@host</label><input id="target" aria-describedby="target-help" bind:value={draft.target} placeholder="clipboard-hub" autocomplete="off" disabled={!ready || busy} />
          <p id="target-help" class="field-help">Uses your existing SSH configuration and unlocked key. The server must already be installed.</p>
          <label for="server-features">Server features</label>
          <select id="server-features" aria-describedby="server-features-help" value={draft.endpoint.protocol} disabled={!ready || busy} onchange={(event) => draft.endpoint = event.currentTarget.value === "shelf" ? { protocol: "shelf", max_item_bytes: 67108864, max_decoded_bytes: 268435456 } : { protocol: "legacy" }}>
            <option value="legacy">Single shared item (legacy server)</option><option value="shelf">Temporary history (new server)</option>
          </select>
          <p id="server-features-help" class="field-help">Choose the features installed on this server. Temporary history requires the new server and an SSH key that allows its session endpoint.</p>
          <details class="advanced-connection" bind:open={advancedOpen}>
            <summary>Advanced connection settings</summary>
            <p id="advanced-help" class="field-help">These settings apply to this connection. Blank Port and Identity file use your SSH configuration or agent.</p>
            <label for="port">Port <span>(optional)</span></label><input id="port" aria-describedby="advanced-help" type="number" min="1" max="65535" value={draft.port ?? ""} oninput={(event) => draft.port = event.currentTarget.value === "" ? null : Number(event.currentTarget.value)} placeholder="From SSH config" disabled={!ready || busy} />
            <label for="identity">Identity file <span>(optional)</span></label><input id="identity" aria-describedby="advanced-help" bind:value={draft.identityFile} placeholder="From SSH config or agent" autocomplete="off" disabled={!ready || busy} />
            <label for="ssh-options">SSH options <span>(one per line)</span></label>
            <textarea id="ssh-options" aria-describedby="ssh-options-help" rows="3" value={sshOptionsText} oninput={(event) => editSshOptions(event.currentTarget.value)} placeholder="ServerAliveInterval=20" spellcheck="false" autocomplete="off" disabled={!ready || busy}></textarea>
            <p id="ssh-options-help" class="field-help">Order is preserved; blank lines are ignored. Connection tests and transfers keep batch mode enabled, so keys must already be unlocked and hosts trusted.</p>
            <label for="ssh-binary">SSH executable <span>(optional)</span></label><input id="ssh-binary" aria-describedby="ssh-binary-help" bind:value={draft.transport.sshBinary} placeholder="System ssh" autocomplete="off" disabled={!ready || busy} />
            <p id="ssh-binary-help" class="field-help">An executable path, without command-line arguments.</p>
            <label for="timeout">Timeout (milliseconds)</label><input id="timeout" aria-describedby="timeout-help" inputmode="numeric" bind:value={draft.transport.timeoutMs} disabled={!ready || busy} />
            <p id="timeout-help" class="field-help">Default 7,000 ms (7 seconds).</p>
            {#if draft.endpoint.protocol === "shelf"}
              <label for="shelf-item-limit">Item limit (bytes)</label><input id="shelf-item-limit" aria-describedby="shelf-item-help" type="number" min="1" max="268435456" step="1" required bind:value={draft.endpoint.max_item_bytes} disabled={!ready || busy} />
              <p id="shelf-item-help" class="field-help">Default 64 MiB. The server may negotiate a smaller limit.</p>
              <label for="shelf-decode-limit">Decoded image limit (bytes)</label><input id="shelf-decode-limit" aria-describedby="shelf-decode-help" type="number" min="1" max="268435456" step="1" required bind:value={draft.endpoint.max_decoded_bytes} disabled={!ready || busy} />
              <p id="shelf-decode-help" class="field-help">Default 256 MiB of decoded pixels. Images retain their original dimensions and fidelity.</p>
            {:else}
            <label for="transfer-limit">Transfer limit (bytes)</label><input id="transfer-limit" aria-describedby="transfer-limit-help" inputmode="numeric" bind:value={draft.transport.maxSize} disabled={!ready || busy} />
            <p id="transfer-limit-help" class="field-help">Default 10,485,760 bytes (10 MiB), including metadata. Raising this client limit also needs matching proxy and daemon limits. Images still have a separate 64 MiB decoded-pixel limit.</p>
            <label class="toggle"><input type="checkbox" aria-describedby="framing-help" bind:checked={draft.transport.resyncFrames} disabled={!ready || busy} />Recover from text before protocol frames</label>
            <p id="framing-help" class="field-help">Handles a bounded SSH login banner. Turn off for strict framing.</p>
            <label for="resync-budget">Framing recovery budget (bytes)</label><input id="resync-budget" aria-describedby="resync-budget-help" inputmode="numeric" bind:value={draft.transport.resyncMaxBytes} disabled={!ready || busy || !draft.transport.resyncFrames} />
            <p id="resync-budget-help" class="field-help">Default 8,192 bytes. Retained while strict framing is selected.</p>
            {/if}
            <button type="button" class="text-button" disabled={!ready || busy} onclick={resetTransport}>Restore advanced defaults</button>
          </details>
          <button type="button" class="test-button" disabled={!ready || !configured || dirty || busy} onclick={() => perform("test_connection", "SSH proxy and clipboard daemon responded.")}>Test saved connection</button>
          {#if dirty}<p class="field-help">Apply your changes before testing the connection.</p>{/if}
          {#if view.connectionReport && !dirty}
            <section class="connection-report" aria-label="Saved connection diagnostics"><details open={!view.connectionReport.healthy}>
              <summary>{view.connectionReport.healthy ? "Connection verified" : "Connection needs attention"}</summary>
              <ul>
                {#each view.connectionReport.checks as check (check.stage)}
                  <li class:failed={check.status === "failed"}><strong>{check.label} · {check.status === "passed" ? "Verified" : check.status === "failed" ? "Failed" : "Not verified"}</strong><span>{check.detail}</span></li>
                {/each}
              </ul>
              {#if view.connectionReport.hint}<p>{view.connectionReport.hint}</p>{/if}
            </details></section>
          {/if}
        {:else if section === "shortcuts"}
          <p class="section-intro">Open the panel with a shortcut, the tray menu, or the launcher.</p>
          <label class="toggle"><input type="checkbox" aria-describedby="shortcut-status" bind:checked={draft.shortcutEnabled} disabled={!ready || busy} />Enable Open panel shortcut</label>
          <label for="shortcut">Open panel</label>
          <button id="shortcut" aria-describedby="shortcut-status" type="button" class:recording={recording === "openShortcut"} class="shortcut-recorder" disabled={!ready || !draft.shortcutEnabled || busy} onclick={() => startRecording("openShortcut")}>{recording === "openShortcut" ? "Press a shortcut · Esc to cancel" : displayShortcut(draft.openShortcut, view.platform)}</button>
          <p id="shortcut-status" class="field-help">{recording ? `Use ${view.platform === "macos" ? "Command or Option" : "Ctrl or Alt"} with a key. Recording never transfers content.` : view.shortcutStatus}</p>
          <button type="button" class="text-button" disabled={!ready || busy} onclick={() => { draft.openShortcut = "CommandOrControl+Alt+Shift+S"; inform("Default shortcut restored in your draft."); }}>Restore default shortcut</button>
          <h3>Share and copy shortcuts <span class="background-cue">Background only</span></h3>
          <p id="direct-help" class="field-help">Use the panel buttons while this window has focus. Recording never transfers content. Choose feedback in Behavior.</p>
          <label class="toggle"><input type="checkbox" aria-describedby="direct-help" bind:checked={draft.directShortcutsEnabled} disabled={!ready || busy || !view.shortcutAvailable} />Enable direct shortcuts</label>
          {#each [["directShareShortcut", "Share current clipboard"], ["directCopyShortcut", "Copy shared clipboard"]] as [field, label]}
            <label for={field}>{label}</label>
            <button id={field} aria-describedby="direct-help direct-transfer-help" type="button" class:recording={recording === field} class="shortcut-recorder" disabled={!ready || !draft.directShortcutsEnabled || busy || !view.shortcutAvailable} onclick={() => startRecording(field as ShortcutField)}>{recording === field ? "Press a shortcut · Esc to cancel" : draft[field as ShortcutField] ? displayShortcut(draft[field as ShortcutField]!, view.platform) : "Record shortcut"}</button>
            {#if draft[field as ShortcutField]}<button type="button" class="text-button" disabled={!ready || busy} aria-label={`Clear ${label} shortcut`} onclick={() => { draft[field as "directShareShortcut" | "directCopyShortcut"] = null; stopRecording(); }}>Clear shortcut</button>{/if}
          {/each}
          <p id="direct-transfer-help" class="field-help">Share sends what's already on this computer's clipboard; it doesn't copy selected text. Copy receives the latest shared item; paste it normally.</p>
        {:else if section === "updates"}
          <section class="updates-panel" aria-label="Software updates">
          <div class="update-versions"><span>Installed: {view.updateStatus.currentVersion === "0.0.0" ? "Development prototype" : view.updateStatus.currentVersion}</span>{#if view.updateStatus.offeredVersion}<strong>Available: {view.updateStatus.offeredVersion}</strong>{/if}</div>
          <p class="field-help update-detail" role={view.updateStatus.phase === "failed" ? "alert" : "status"}>{view.updateStatus.detail}</p>
          <div class="update-actions">
          {#if view.updateStatus.offeredVersion && view.updateStatus.phase !== "ready"}
            <button type="button" class="primary" disabled={!ready || busy || view.updateStatus.busy} onclick={() => updateAction("download_update", view.updateStatus.offeredVersion!)}>{view.updateStatus.phase === "downloading" ? "Downloading…" : view.updateStatus.phase === "verifying" ? "Verifying…" : "Download update"}</button>
          {/if}
          {#if view.updateStatus.phase === "ready" && view.updateStatus.offeredVersion}
            {#if confirmingInstall === view.updateStatus.offeredVersion}
              <div id="install-confirmation" data-confirmation class="feedback" role="group" aria-label="Install update confirmation">
                <p>Install version {confirmingInstall} and restart SSH Clipboard? Your settings are kept.</p>
                <button type="button" disabled={busy || dirty || !!view.pendingShare || view.updateStatus.busy} onclick={() => updateAction("install_update", confirmingInstall!)}>Install and restart</button>
                <button type="button" disabled={busy} onclick={() => installConfirmation(false)}>Cancel</button>
              </div>
            {:else}
              <button id="install-update" type="button" class="primary" disabled={busy || dirty || !!view.pendingShare || view.updateStatus.busy} onclick={() => installConfirmation(true)}>Install update…</button>
            {/if}
          {/if}
          <button type="button" aria-label="Check for updates" disabled={!ready || busy || view.updateStatus.busy || !view.updateStatus.available} onclick={() => updateAction("check_updates")}>{view.updateStatus.busy && view.updateStatus.phase === "checking" ? "Checking…" : "Check for updates"}</button>
          </div>
          {#if view.updateStatus.phase === "ready" && view.updateStatus.offeredVersion}
            {#if dirty}<p class="field-help">Apply or discard settings changes before installing.</p>{/if}
            {#if view.pendingShare}<p class="field-help">Resolve or discard the uncertain share before installing.</p>{/if}
          {/if}
          <div class="update-operation">
            {@render operationProgress()}
            {#if view.updateStatus.phase === "downloading"}<p class="field-help">{formatSize(view.updateStatus.downloadedBytes)}{view.updateStatus.totalBytes !== null ? ` of ${formatSize(view.updateStatus.totalBytes)}` : " downloaded"}</p>{/if}
          </div>
          {#if updateNotes}
            <section class="update-summary" aria-label="What's new"><h3>What's new</h3><ul>{#each updateNotes as note}<li>{note}</li>{/each}</ul></section>
          {:else if view.updateStatus.notes}
            <details class="update-notes"><summary>What's new</summary><pre class="release-notes">{view.updateStatus.notes}</pre></details>
          {/if}
          </section>
        {:else}
          <p class="section-intro">Preferences for this computer, across all connections.</p>
          <section class="preference-group" aria-label="Startup">
          <h3>Startup</h3>
          <label class="toggle"><input type="checkbox" aria-describedby={startupHelp ? "startup-help" : undefined} checked={startupDraft ?? (view.startupStatus.enabled === true)} disabled={!ready || busy || !view.settingsWritable || !view.startupStatus.available || view.startupStatus.enabled === null} onchange={(event) => startupDraft = event.currentTarget.checked === view.startupStatus.enabled ? null : event.currentTarget.checked} />Start at login</label>
          {#if startupHelp}<p id="startup-help" class="field-help">{startupHelp}</p>{/if}
          </section>
          <section class="preference-group" aria-label="Clipboard panel">
          <h3>Clipboard panel</h3>
          <label class="toggle"><input type="checkbox" aria-describedby="preview-help" bind:checked={draft.showPreview} disabled={!ready || busy} />Show content previews</label>
          <p id="preview-help" class="field-help">Turn off to hide text and images when sharing your screen.</p>
          <label class="toggle"><input type="checkbox" aria-describedby="dismiss-help" bind:checked={draft.closeAfterCopy} disabled={!ready || busy} />Hide panel after copying</label>
          <p id="dismiss-help" class="field-help">The app keeps running so the copied content stays available.</p>
          </section>
          <section class="preference-group" aria-label="Shortcut feedback">
          <h3>Shortcut feedback</h3>
          <label for="notifications">Feedback after shortcut actions</label>
          <select id="notifications" aria-describedby="feedback-help" bind:value={draft.notifications} disabled={!ready || busy}>
            <option value="panel">Open panel with result</option>
            <option value="all">Notify for success and errors</option>
            <option value="errors">Notify for errors only</option>
            <option value="off">Keep quiet</option>
          </select>
          <p id="feedback-help" class="field-help">The latest result stays in the panel.</p>
          {#if notificationMode}
            <p class="field-help">Notifications never show clipboard content.</p>
            <details class="notification-help" open={!view.notificationStatus.available}><summary>Notification help</summary><p class="field-help">{view.notificationStatus.detail}</p></details>
            <button type="button" aria-describedby="test-notification-help" disabled={!ready || busy || !view.notificationStatus.available} onclick={() => perform("test_notification", "Test notification submitted to the system.")}>Test notification</button>
            <p id="test-notification-help" class="field-help">Sends a test now, without applying your draft. System settings may hide it.</p>
          {/if}
          </section>
          {#if view.trayStatus !== "Available"}<div class="capability"><strong>Tray menu</strong><span>{view.trayStatus}</span></div>{/if}
        {/if}
      </form>
      {@render feedback()}
      </div>
    {/if}
  </main>

  {#if page === "settings"}
    <section class="settings-savebar" aria-label="Settings changes">
      <p role="status">{!ready ? "Loading settings…" : staleDraft ? "Saved settings changed" : dirty ? "Unsaved changes" : view.settingsSaved ? "All changes saved" : "Apply to save"}</p>
      {#if staleDraft}<p class="save-help">Discard your draft to load the saved settings.</p>
      {:else if dirty}<p class="save-help">Changes take effect after Apply.</p>{/if}
      <div class="settings-actions">
        <button type="submit" form="settings-form" class="primary" disabled={applyDisabled}>Apply settings</button>
        <button type="button" disabled={!dirty || busy} onclick={discardChanges}>Discard changes</button>
      </div>
      {@render routineFeedback()}
    </section>
  {/if}

  <footer><span>{view.activeShortcut ? displayShortcut(view.activeShortcut, view.platform) : "Open from the launcher or tray"}</span><span>{view.updateStatus.available ? view.updateStatus.currentVersion : "Prototype"}</span><button class="quit-button" disabled={!ready || busy} onclick={() => command("quit_app").catch((cause) => inform(String(cause), true))}>Quit</button></footer>
</div>
