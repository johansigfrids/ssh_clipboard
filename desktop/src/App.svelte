<script lang="ts">
  import { onMount } from "svelte";
  import { cloneSettings, cloneTransport, command, defaultTransport, initialView, subscribe, type OperationView, type PanelView, type Settings } from "./api";
  import { displayShortcut, shortcutFromEvent } from "./shortcut.js";

  let view = $state<PanelView>(initialView);
  let draft = $state<Settings>(cloneSettings(initialView.settings));
  let sshOptionsText = $state("");
  let page = $state("panel");
  let section = $state("connection");
  let localBusy = $state(false);
  let operation = $state<OperationView>({ generation: "0", active: null });
  const busy = $derived(localBusy || operation.active !== null);
  let message = $state("");
  let error = $state(false);
  type ShortcutField = "openShortcut" | "directShareShortcut" | "directCopyShortcut";
  let recording = $state<ShortcutField | null>(null);
  let recordingRequested = $state(false);
  let recordingEpoch = 0;
  let recordingSync = Promise.resolve();
  let removingProfile = $state(false);
  let confirmingHistory = $state<{ kind: "delete" | "clear"; epoch: string; itemId: string | null } | null>(null);
  let ready = $state(false);
  let content = $state<HTMLElement>();
  let draftRevision = $state<string | null>(null);
  let reloadPending = false;
  let reloading = false;
  let disposed = false;
  let operationEpoch = 0;

  const configured = $derived(view.settings.target.trim().length > 0);
  const dirty = $derived(JSON.stringify(draft) !== JSON.stringify(view.settings));
  const staleDraft = $derived(dirty && draftRevision !== view.settingsRevision);
  const selectedHistory = $derived(view.history?.items.find((item) => item.id === view.history?.selectedId));

  function requestHistoryRemoval(kind: "delete" | "clear") {
    if (!view.history?.epoch) return;
    confirmingHistory = { kind, epoch: view.history.epoch, itemId: view.history.selectedId };
  }

  function confirmHistoryRemoval() {
    const confirmation = confirmingHistory;
    if (!confirmation) return;
    void perform(confirmation.kind === "clear" ? "clear_history" : "delete_item", confirmation.kind === "clear" ? "History cleared." : "Item deleted.", { epoch: confirmation.epoch, ...(confirmation.kind === "delete" ? { itemId: confirmation.itemId } : {}) });
  }

  function inform(text: string, failed = false) { message = text; error = failed; }

  function acceptOperation(next: OperationView) {
    if (disposed || !next || !/^(0|[1-9][0-9]*)$/.test(next.generation)) return;
    if (BigInt(next.generation) < BigInt(operation.generation)) return;
    operation = next;
    if (!next.active && reloadPending && !localBusy) void reloadSavedSettings();
  }

  async function cancelOperation() {
    const active = operation.active;
    if (!active?.cancellable) return;
    try { acceptOperation(await command<OperationView>("cancel_operation", { id: active.id })); }
    catch (cause) { inform(String(cause), true); }
  }

  function discardDraft() { draft = cloneSettings(view.settings); sshOptionsText = draft.transport.sshOptions.join("\n"); draftRevision = view.settingsRevision; stopRecording(); removingProfile = false; }

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
      const retainDraft = dirty && JSON.stringify(draft) !== JSON.stringify(next.settings);
      if (JSON.stringify(next.operationNotice) !== JSON.stringify(view.operationNotice)) inform("");
      view = next;
      removingProfile = false;
      confirmingHistory = null;
      if (retainDraft && draftRevision !== next.settingsRevision) inform("Saved settings changed. Discard your draft to load them before editing again.", true);
      else if (!retainDraft) discardDraft();
    } catch (cause) { if (!disposed) inform(String(cause), true); }
    finally {
      reloading = false;
      if (reloadPending && !busy && !disposed) void reloadSavedSettings();
    }
  }

  async function perform(name: string, success: string, args?: Record<string, unknown>) {
    if (busy || !ready) return;
    const hadDraft = dirty;
    localBusy = true;
    operationEpoch++;
    inform("");
    try {
      const guardedArgs = ["test_connection", "refresh_item", "share_clipboard", "copy_item", "remove_profile", "select_item", "delete_item", "clear_history", "reconnect_history", "recover_share", "discard_share"].includes(name) ? { ...args, expectedRevision: view.settingsRevision } : args;
      confirmingHistory = null;
      view = await command<PanelView>(name, guardedArgs);
      if (name === "apply_settings" || name === "remove_profile") discardDraft();
      inform(name === "recover_share" && view.operationNotice ? view.operationNotice.message : name === "refresh_item" && !view.item ? "Checked the server. No shared item is available." : success);
      if (name === "select_item" && content) content.scrollTop = 0;
    } catch (cause) {
      inform(cause instanceof Error ? cause.message : String(cause), true);
      // Reflect connection/registration state even after a failed operation.
      try { view = await command<PanelView>("get_panel"); if (!hadDraft) discardDraft(); } catch { /* Keep the last view. */ }
    } finally { localBusy = false; if (reloadPending) void reloadSavedSettings(); }
  }

  function navigate(next: string) {
    if (busy) return;
    stopRecording();
    removingProfile = false;
    confirmingHistory = null;
    page = next;
    if (content) content.scrollTop = 0;
  }

  function selectSection(next: string) {
    section = next;
    stopRecording();
    removingProfile = false;
    if (content) content.scrollTop = 0;
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
      if (page === "settings") navigate("panel");
      else void command("hide_panel").catch((cause) => inform(String(cause), true));
    }
  }

  function recordShortcut(binding: string) {
    if (!recording || busy || !ready) return;
    draft[recording] = binding;
    stopRecording();
  }

  function formatSize(bytes: number) { return bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} KiB`; }
  function formatTime(milliseconds: number | string) {
    const date = new Date(Number(milliseconds));
    return Number.isNaN(date.getTime()) ? "Unknown time" : date.toLocaleString();
  }

  onMount(() => {
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
        }, acceptOperation);
        if (disposed) unsubscribe(); else stop = unsubscribe;
      } catch { inform("Could not listen for tray or settings changes. Reopen through the launcher.", true); }
      if (disposed) return;
      try {
        acceptOperation(await command<OperationView>("get_operation"));
        const next = await command<PanelView>("get_panel");
        if (disposed) return;
        view = next; discardDraft(); ready = true;
        if (reloadPending) void reloadSavedSettings();
      } catch (cause) { inform(cause instanceof Error ? cause.message : String(cause), true); }
    })();
    return () => { disposed = true; stopRecording(); stop(); };
  });
</script>

<svelte:window onkeydown={handleKey} onblur={stopRecording} />

{#snippet feedback()}
  {#if message}<div class:error class="feedback" role={error ? "alert" : "status"}>{message}</div>
  {:else if view.operationNotice}<div class:error={view.operationNotice.failed} class="feedback" role={view.operationNotice.failed ? "alert" : "status"}>{view.operationNotice.message}</div>{/if}
{/snippet}

<div class="app-shell">
  <header>
    <div class="brand-mark" aria-hidden="true">↔</div>
    <div class="brand"><h1>SSH Clipboard</h1><span>{configured ? view.settings.target : "Connect your machines"}</span></div>
    {#if page === "panel"}
      <button class="icon-button" aria-label="Open settings" onclick={() => navigate("settings")} disabled={!ready || busy}>⚙</button>
    {:else}
      <button class="text-button" onclick={() => navigate("panel")} disabled={!ready || busy}>Done</button>
    {/if}
  </header>

  <main bind:this={content}>
    {#if operation.active}
      <div class="feedback operation-progress" role="status">
        <span>{operation.active.label}… {operation.active.cancelling ? "Stopping…" : !operation.active.cancellable ? "Finishing…" : ""}</span>
        <button type="button" disabled={!operation.active.cancellable} onclick={cancelOperation}>Cancel operation</button>
      </div>
    {/if}
    {#if view.settingsNotice}<div class="feedback error" role="alert">{view.settingsNotice}</div>{/if}
    {#if page === "panel"}
      <div class="status-line"><span class:connected={view.connected} class="status-dot"></span>{busy ? "Working…" : view.connected ? "Server reachable" : configured ? "Not connected" : "Setup needed"}<span class="mode">{view.settings.profileName}</span></div>
      {#if view.historyStatus}
        <div class="history-connection" aria-label="History connection">
          <p>{view.historyStatus.detail}{view.historyStatus.retryInMs !== null ? ` Retrying in ${Math.ceil(view.historyStatus.retryInMs / 1000)} seconds.` : ""}</p>
          {#if view.historyStatus.phase === "needs_action" || view.historyStatus.phase === "reconnecting"}<button type="button" class="text-button" disabled={!ready || busy} onclick={() => perform("reconnect_history", "Reconnecting history…")}>Reconnect history</button>{/if}
        </div>
      {/if}
      {#if !configured}
        <section class="empty-state">
          <div class="empty-icon" aria-hidden="true">↔</div>
          <h2>Your clipboard, across machines</h2>
          <p>Connect to your SSH clipboard server. Then share from one computer and copy on another.</p>
          <button class="primary" onclick={() => navigate("settings")}>Set up connection</button>
        </section>
      {:else if view.item}
        <section class="item-card" aria-label="Shared item">
          <div class="item-top"><span class="kind">{view.item.kind === "image" ? "Image" : "Text"}</span><span>{formatSize(view.item.encodedBytes)}</span></div>
          <h2>{view.item.label}</h2>
          {#if view.item.imageUrl}<img class="preview-image" src={view.item.imageUrl} alt="Shared clipboard preview" />
          {:else if view.item.text !== null}<pre>{view.item.text || "(Empty text)"}</pre>
          {:else}<p class="muted">{view.settings.showPreview ? "Refresh or select this item to load its preview." : "Content preview is hidden."}</p>{/if}
          <div class="item-meta">{formatTime(view.item.createdAt)}<span>{view.history ? selectedHistory?.source ?? "Unknown source device" : "Source device unavailable on this server"}</span></div>
        </section>
      {:else if selectedHistory}
        <section class="item-card" aria-label="Selected history item"><h2>{selectedHistory.label}</h2><p class="muted">Select this item in history to load its preview, or copy it to this computer.</p></section>
      {:else if view.clipboardChecked}
        <section class="empty-state compact"><h2>Server clipboard is empty</h2><p>Copy some text or an image on this computer, then choose Share current clipboard.</p></section>
      {:else}
        <section class="empty-state compact"><h2>No shared item loaded</h2><p>Refresh to see the server's clipboard, or share what you copied on this computer.</p></section>
      {/if}

      <div class="actions">
        <button class="primary" disabled={!ready || (!view.item && !selectedHistory) || busy} onclick={() => perform("copy_item", "Copied to this computer. You can paste it now.")}>Copy to this computer</button>
        <button disabled={!ready || !configured || busy || view.pendingShare !== null} onclick={() => perform("share_clipboard", "Shared your clipboard with the server.")}>Share current clipboard</button>
        <button class="text-button" disabled={!ready || !configured || busy} onclick={() => perform("refresh_item", view.history ? "History refreshed." : "Shared clipboard refreshed.")}>{view.history ? "Refresh history" : "Refresh shared clipboard"}</button>
      </div>
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
      {@render feedback()}
      {#if configured && view.history}
        <section class="history" aria-label="Temporary clipboard history">
          <div class="history-heading"><h2>Temporary history <span>{view.history.items.length}</span></h2><button type="button" class="text-button" disabled={!ready || busy || !view.history.items.length} onclick={() => requestHistoryRemoval("clear")}>Clear history…</button></div>
          {#if view.history.items.length}
            <ul class="history-list">
              {#each view.history.items as item (item.id)}
                <li><button type="button" class:selected={item.id === view.history.selectedId} aria-pressed={item.id === view.history.selectedId} disabled={!ready || busy} onclick={() => perform("select_item", "Item selected. Your clipboard is unchanged.", { itemId: item.id, epoch: view.history?.epoch })}>
                  <span class="history-label">{item.label}</span><span class="history-detail">{item.source ?? "Unknown source"} · {formatSize(item.encodedBytes)}</span><span class="history-detail">{formatTime(item.receivedAt)} · Expires {formatTime(item.expiresAt)}</span>
                </button></li>
              {/each}
            </ul>
            <button class="text-button" disabled={!ready || busy || !selectedHistory} onclick={() => requestHistoryRemoval("delete")}>Delete selected item…</button>
          {:else}<p class="field-help">Shared items expire automatically. History is also cleared when the server restarts.</p>{/if}
          {#if confirmingHistory}
            <div class="remove-confirmation" role="group" aria-label="History removal confirmation">
              <p>{confirmingHistory.kind === "clear" ? "Clear all items from this server's temporary history?" : "Delete the selected item from the server?"} Copies already saved on other computers remain available.</p>
              <div class="profile-actions"><button type="button" disabled={busy} onclick={confirmHistoryRemoval}>{confirmingHistory.kind === "clear" ? "Clear history" : "Delete item"}</button><button type="button" disabled={busy} onclick={() => confirmingHistory = null}>Cancel</button></div>
            </div>
          {/if}
        </section>
      {:else if configured}<p class="field-help">This connection uses the legacy server's single shared item.</p>{/if}
      <p class="quiet-note">{view.history ? "History updates automatically. Sharing and copying happen only when you ask." : "Sharing and copying happen only when you ask."}</p>
    {:else}
      <div class="settings-title"><h2>Settings</h2><span>{view.settingsSaved ? "Saved on this computer" : "Apply to save"}</span></div>
      {#if view.migrationSummary}<p class="field-help">{view.migrationSummary}</p>{/if}
      <nav aria-label="Settings sections">
        {#each [["connection", "Connection"], ["shortcuts", "Shortcuts"], ["behavior", "Behavior"]] as [id, label]}
          <button class:active={section === id} aria-current={section === id ? "page" : undefined} onclick={() => selectSection(id)}>{label}</button>
        {/each}
      </nav>
      <form onsubmit={(event) => { event.preventDefault(); void perform("apply_settings", "Settings applied.", { settings: cloneSettings(draft), expectedRevision: draftRevision }); }}>
        {#if section === "connection"}
          <label for="profile">Connection</label>
          <select id="profile" value={draft.profileId ?? ""} onchange={(event) => chooseProfile(event.currentTarget.value)} disabled={!ready || !view.settingsWritable || dirty || busy}>
            {#each view.profiles as profile (profile.id)}<option value={profile.id}>{profile.name} — {profile.target || "Setup needed"}</option>{/each}
            {#if draft.profileId === null}<option value="">New connection</option>{/if}
            {#if draft.profileId !== null && !view.profiles.some((profile) => profile.id === draft.profileId)}<option value={draft.profileId}>{draft.profileName} (removed)</option>{/if}
          </select>
          <p class="field-help">Choose a connection, then Apply to use it. Apply or discard your draft before choosing another.</p>
          <div class="profile-actions">
            <button type="button" disabled={!ready || !view.settingsWritable || !view.settingsSaved || dirty || busy} onclick={addProfile}>Add connection</button>
            <button type="button" disabled={!ready || !view.settingsWritable || !view.settingsSaved || view.profiles.length < 2 || dirty || busy} onclick={() => removingProfile = !removingProfile}>Remove…</button>
          </div>
          {#if removingProfile}
            <div class="remove-confirmation" role="group" aria-label="Remove connection confirmation">
              <p>Remove “{view.settings.profileName}” from this computer? Another saved connection will become active. The server and its clipboard are kept.</p>
              <div class="profile-actions"><button type="button" disabled={!view.settingsWritable || dirty || busy} onclick={() => perform("remove_profile", "Connection removed. Another saved connection is now active.", { profileId: view.settings.profileId })}>Remove connection</button><button type="button" disabled={busy} onclick={() => removingProfile = false}>Cancel</button></div>
            </div>
          {/if}
          <label for="profile-name">Connection name</label><input id="profile-name" bind:value={draft.profileName} autocomplete="off" disabled={!ready || busy} />
          <label for="target">SSH alias or user@host</label><input id="target" bind:value={draft.target} placeholder="clipboard-hub" autocomplete="off" disabled={!ready || busy} />
          <p class="field-help">Uses your existing SSH configuration and unlocked key. The server must already be installed.</p>
          <label for="server-features">Server features</label>
          <select id="server-features" value={draft.endpoint.protocol} disabled={!ready || busy} onchange={(event) => draft.endpoint = event.currentTarget.value === "shelf" ? { protocol: "shelf", max_item_bytes: 67108864, max_decoded_bytes: 268435456 } : { protocol: "legacy" }}>
            <option value="legacy">Single shared item (legacy server)</option><option value="shelf">Temporary history (new server)</option>
          </select>
          <p class="field-help">Choose the features installed on this server. Temporary history requires the new server and an SSH key that allows its session endpoint.</p>
          <label for="port">Port <span>(optional)</span></label><input id="port" type="number" min="1" max="65535" value={draft.port ?? ""} oninput={(event) => draft.port = event.currentTarget.value === "" ? null : Number(event.currentTarget.value)} placeholder="From SSH config" disabled={!ready || busy} />
          <label for="identity">Identity file <span>(optional)</span></label><input id="identity" bind:value={draft.identityFile} placeholder="From SSH config or agent" autocomplete="off" disabled={!ready || busy} />
          <details class="advanced-connection">
            <summary>Advanced connection settings</summary>
            <p class="field-help">These settings apply to this connection. SSH aliases already use your OpenSSH configuration.</p>
            <label for="ssh-options">SSH options <span>(one per line)</span></label>
            <textarea id="ssh-options" rows="3" value={sshOptionsText} oninput={(event) => editSshOptions(event.currentTarget.value)} placeholder="ServerAliveInterval=20" spellcheck="false" autocomplete="off" disabled={!ready || busy}></textarea>
            <p class="field-help">Order is preserved; blank lines are ignored. Connection tests and transfers keep batch mode enabled, so keys must already be unlocked and hosts trusted.</p>
            <label for="ssh-binary">SSH executable <span>(optional)</span></label><input id="ssh-binary" bind:value={draft.transport.sshBinary} placeholder="System ssh" autocomplete="off" disabled={!ready || busy} />
            <p class="field-help">An executable path, without command-line arguments.</p>
            <label for="timeout">Timeout (milliseconds)</label><input id="timeout" inputmode="numeric" bind:value={draft.transport.timeoutMs} disabled={!ready || busy} />
            <p class="field-help">Default 7,000 ms (7 seconds).</p>
            {#if draft.endpoint.protocol === "shelf"}
              <label for="shelf-item-limit">Item limit (bytes)</label><input id="shelf-item-limit" type="number" min="1" max="268435456" step="1" required bind:value={draft.endpoint.max_item_bytes} disabled={!ready || busy} />
              <p class="field-help">Default 64 MiB. The server may negotiate a smaller limit.</p>
              <label for="shelf-decode-limit">Decoded image limit (bytes)</label><input id="shelf-decode-limit" type="number" min="1" max="268435456" step="1" required bind:value={draft.endpoint.max_decoded_bytes} disabled={!ready || busy} />
              <p class="field-help">Default 256 MiB of decoded pixels. Images retain their original dimensions and fidelity.</p>
            {:else}
            <label for="transfer-limit">Transfer limit (bytes)</label><input id="transfer-limit" inputmode="numeric" bind:value={draft.transport.maxSize} disabled={!ready || busy} />
            <p class="field-help">Default 10,485,760 bytes (10 MiB), including metadata. Raising this client limit also needs matching proxy and daemon limits. Images still have a separate 64 MiB decoded-pixel limit.</p>
            <label class="toggle"><input type="checkbox" bind:checked={draft.transport.resyncFrames} disabled={!ready || busy} />Recover from text before protocol frames</label>
            <p class="field-help">Handles a bounded SSH login banner. Turn off for strict framing.</p>
            <label for="resync-budget">Framing recovery budget (bytes)</label><input id="resync-budget" inputmode="numeric" bind:value={draft.transport.resyncMaxBytes} disabled={!ready || busy || !draft.transport.resyncFrames} />
            <p class="field-help">Default 8,192 bytes. Retained while strict framing is selected.</p>
            {/if}
            <button type="button" class="text-button" disabled={!ready || busy} onclick={resetTransport}>Restore advanced defaults</button>
          </details>
          <button type="button" class="test-button" disabled={!ready || !configured || dirty || busy} onclick={() => perform("test_connection", "SSH proxy and clipboard daemon responded.")}>Test saved connection</button>
          {#if dirty}<p class="field-help">Apply your changes before testing the connection.</p>{/if}
          {#if view.connectionReport && !dirty}
            <section class="connection-report" aria-label="Saved connection diagnostics">
              <h3>{view.connectionReport.healthy ? "Connection verified" : "Connection needs attention"}</h3>
              <ul>
                {#each view.connectionReport.checks as check (check.stage)}
                  <li class:failed={check.status === "failed"}><strong>{check.label} · {check.status === "passed" ? "Verified" : check.status === "failed" ? "Failed" : "Not verified"}</strong><span>{check.detail}</span></li>
                {/each}
              </ul>
              {#if view.connectionReport.hint}<p>{view.connectionReport.hint}</p>{/if}
            </section>
          {/if}
        {:else if section === "shortcuts"}
          <p class="section-intro">One shortcut opens the clipboard panel. You can also use the tray menu or launch the app.</p>
          <label class="toggle"><input type="checkbox" bind:checked={draft.shortcutEnabled} disabled={!ready || busy} />Enable Open panel shortcut</label>
          <label for="shortcut">Open panel</label>
          <button id="shortcut" type="button" class:recording={recording === "openShortcut"} class="shortcut-recorder" disabled={!ready || !draft.shortcutEnabled || busy} onclick={() => startRecording("openShortcut")}>{recording === "openShortcut" ? "Press a shortcut · Esc to cancel" : displayShortcut(draft.openShortcut)}</button>
          <p class="field-help">{recording ? "Use Ctrl/Cmd or Alt with a key." : view.shortcutStatus}</p>
          <button type="button" class="text-button" disabled={!ready || busy} onclick={() => draft.openShortcut = "CommandOrControl+Shift+V"}>Restore default shortcut</button>
          <h3>Optional direct shortcuts</h3>
          <p class="field-help">Share or copy immediately while the app is in the background. While this window has focus, use the panel buttons. Choose how results appear in Behavior settings.</p>
          <label class="toggle"><input type="checkbox" bind:checked={draft.directShortcutsEnabled} disabled={!ready || busy || !view.shortcutAvailable} />Enable direct shortcuts</label>
          {#each [["directShareShortcut", "Share current clipboard"], ["directCopyShortcut", "Copy shared clipboard"]] as [field, label]}
            <label for={field}>{label}</label>
            <button id={field} type="button" class:recording={recording === field} class="shortcut-recorder" disabled={!ready || !draft.directShortcutsEnabled || busy || !view.shortcutAvailable} onclick={() => startRecording(field as ShortcutField)}>{recording === field ? "Press a shortcut · Esc to cancel" : draft[field as ShortcutField] ? displayShortcut(draft[field as ShortcutField]!) : "Record shortcut"}</button>
            {#if draft[field as ShortcutField]}<button type="button" class="text-button" disabled={!ready || busy} aria-label={`Clear ${label} shortcut`} onclick={() => { draft[field as "directShareShortcut" | "directCopyShortcut"] = null; stopRecording(); }}>Clear shortcut</button>{/if}
          {/each}
          <p class="field-help">To share: copy text or an image normally first, then press your Share shortcut. It sends what's already on this computer's clipboard; it doesn't copy selected text.</p>
          <p class="field-help">To receive: press your Copy shortcut to put the latest shared item on this computer's clipboard, then paste normally. It doesn't paste into the active app. Recording a shortcut never transfers content.</p>
        {:else}
          <label class="toggle"><input type="checkbox" bind:checked={draft.showPreview} disabled={!ready || busy} />Show content previews</label>
          <p class="field-help">Hide text and image previews when sharing your screen.</p>
          <label class="toggle"><input type="checkbox" bind:checked={draft.closeAfterCopy} disabled={!ready || busy} />Hide panel after copying</label>
          <p class="field-help">The app keeps running so the copied content stays available.</p>
          <label for="notifications">Direct action feedback</label>
          <select id="notifications" bind:value={draft.notifications} disabled={!ready || busy}>
            <option value="panel">Open panel with result</option>
            <option value="all">Notify for success and errors</option>
            <option value="errors">Notify for errors only</option>
            <option value="off">Keep quiet</option>
          </select>
          <p class="field-help">Notifications never include clipboard content, previews, connection details, or raw errors. The latest result is kept in the panel.</p>
          <p class="field-help">{view.notificationStatus.detail}</p>
          <button type="button" disabled={!ready || busy || !view.notificationStatus.available} onclick={() => perform("test_notification", "Test notification submitted to the system.")}>Test notification</button>
          <div class="capability"><strong>Tray menu</strong><span>{view.trayStatus}</span></div>
          <p class="field-help">Start at login and updates will be added after the desktop foundation is verified.</p>
        {/if}
        {#if staleDraft}<p class="field-help">Saved settings changed. Discard your draft to load them.</p>{/if}
        <div class="settings-actions"><button type="submit" class="primary" disabled={!ready || !view.settingsWritable || !dirty || staleDraft || busy || recordingRequested}>Apply settings</button><button type="button" disabled={!dirty || busy} onclick={() => { discardDraft(); inform("Loaded current saved settings."); }}>Discard changes</button></div>
      </form>
      {@render feedback()}
    {/if}
  </main>

  <footer><span>{view.activeShortcut ? displayShortcut(view.activeShortcut) : "Open from the launcher or tray"}</span><span>Prototype</span><button class="quit-button" disabled={!ready || busy} onclick={() => command("quit_app").catch((cause) => inform(String(cause), true))}>Quit</button></footer>
</div>
