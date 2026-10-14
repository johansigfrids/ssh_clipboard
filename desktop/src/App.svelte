<script lang="ts">
  import { onMount } from "svelte";
  import { command, initialView, subscribe, type PanelView, type Settings } from "./api";
  import { displayShortcut, shortcutFromEvent } from "./shortcut.js";

  let view = $state<PanelView>(initialView);
  let draft = $state<Settings>({ ...initialView.settings });
  let page = $state("panel");
  let section = $state("connection");
  let busy = $state(false);
  let message = $state("");
  let error = $state(false);
  let recording = $state(false);
  let removingProfile = $state(false);
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

  function inform(text: string, failed = false) { message = text; error = failed; }

  function discardDraft() { draft = { ...view.settings }; draftRevision = view.settingsRevision; recording = false; removingProfile = false; }

  function chooseProfile(id: string) {
    if (dirty || busy || !ready) return;
    const profile = view.profiles.find((profile) => profile.id === id);
    if (!profile) return;
    draft = { ...draft, profileId: profile.id, profileName: profile.name, target: profile.target, port: profile.port, identityFile: profile.identityFile };
    removingProfile = false;
    inform("");
  }

  function addProfile() {
    if (dirty || busy || !ready) return;
    draft = { ...draft, profileId: null, profileName: "New connection", target: "", port: null, identityFile: "" };
    removingProfile = false;
    inform("");
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
      view = next;
      removingProfile = false;
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
    busy = true;
    operationEpoch++;
    inform("");
    try {
      const guardedArgs = ["test_connection", "refresh_item", "share_clipboard", "copy_item", "remove_profile"].includes(name) ? { ...args, expectedRevision: view.settingsRevision } : args;
      view = await command<PanelView>(name, guardedArgs);
      if (name === "apply_settings" || name === "remove_profile") discardDraft();
      inform(name === "refresh_item" && !view.item ? "Checked the server. No shared item is available." : success);
    } catch (cause) {
      inform(cause instanceof Error ? cause.message : String(cause), true);
      // Reflect connection/registration state even after a failed operation.
      try { view = await command<PanelView>("get_panel"); if (!hadDraft) discardDraft(); } catch { /* Keep the last view. */ }
    } finally { busy = false; if (reloadPending) void reloadSavedSettings(); }
  }

  function navigate(next: string) {
    if (busy) return;
    recording = false;
    removingProfile = false;
    page = next;
    if (content) content.scrollTop = 0;
  }

  function selectSection(next: string) {
    section = next;
    recording = false;
    removingProfile = false;
    if (content) content.scrollTop = 0;
  }

  function handleKey(event: KeyboardEvent) {
    if (recording) {
      event.preventDefault();
      if (event.key === "Escape") { recording = false; return; }
      const shortcut = shortcutFromEvent(event);
      if (shortcut) { draft.openShortcut = shortcut; recording = false; }
      return;
    }
    if (event.key === "Escape" && !busy) {
      if (page === "settings") navigate("panel");
      else void command("hide_panel").catch((cause) => inform(String(cause), true));
    }
  }

  function formatSize(bytes: number) { return bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} KiB`; }
  function formatTime(milliseconds: number) {
    const date = new Date(milliseconds);
    return Number.isNaN(date.getTime()) ? "Unknown time" : date.toLocaleString();
  }

  onMount(() => {
    let stop = () => {};
    void (async () => {
      try {
        // Subscribe before taking the initial snapshot so startup changes cannot be missed.
        const unsubscribe = await subscribe(navigate, (text) => inform(text, true), () => {
          if (!ready) reloadPending = true; else void reloadSavedSettings();
        });
        if (disposed) unsubscribe(); else stop = unsubscribe;
      } catch { inform("Could not listen for tray or settings changes. Reopen through the launcher.", true); }
      if (disposed) return;
      try {
        const next = await command<PanelView>("get_panel");
        if (disposed) return;
        view = next; discardDraft(); ready = true;
        if (reloadPending) void reloadSavedSettings();
      } catch (cause) { inform(cause instanceof Error ? cause.message : String(cause), true); }
    })();
    return () => { disposed = true; stop(); };
  });
</script>

<svelte:window onkeydown={handleKey} />

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
    {#if view.settingsNotice}<div class="feedback error" role="alert">{view.settingsNotice}</div>{/if}
    {#if page === "panel"}
      <div class="status-line"><span class:connected={view.connected} class="status-dot"></span>{busy ? "Working…" : view.connected ? "Server reachable" : configured ? "Not connected" : "Setup needed"}<span class="mode">{view.settings.profileName}</span></div>
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
          {:else}<p class="muted">Content preview is hidden.</p>{/if}
          <div class="item-meta">{formatTime(view.item.createdAt)}<span>Source device unavailable on this server</span></div>
        </section>
      {:else if view.clipboardChecked}
        <section class="empty-state compact"><h2>Server clipboard is empty</h2><p>Copy some text or an image on this computer, then choose Share current clipboard.</p></section>
      {:else}
        <section class="empty-state compact"><h2>No shared item loaded</h2><p>Refresh to see the server's clipboard, or share what you copied on this computer.</p></section>
      {/if}

      <div class="actions">
        <button class="primary" disabled={!ready || !view.item || busy} onclick={() => perform("copy_item", "Copied to this computer. You can paste it now.")}>Copy to this computer</button>
        <button disabled={!ready || !configured || busy} onclick={() => perform("share_clipboard", "Shared your clipboard with the server.")}>Share current clipboard</button>
        <button class="text-button" disabled={!ready || !configured || busy} onclick={() => perform("refresh_item", "Shared clipboard refreshed.")}>Refresh shared clipboard</button>
      </div>
      <p class="quiet-note">Sharing and copying happen only when you ask.</p>
    {:else}
      <div class="settings-title"><h2>Settings</h2><span>{view.settingsSaved ? "Saved on this computer" : "Apply to save"}</span></div>
      {#if view.migrationSummary}<p class="field-help">{view.migrationSummary}</p>{/if}
      <nav aria-label="Settings sections">
        {#each [["connection", "Connection"], ["shortcuts", "Shortcuts"], ["behavior", "Behavior"]] as [id, label]}
          <button class:active={section === id} aria-current={section === id ? "page" : undefined} onclick={() => selectSection(id)}>{label}</button>
        {/each}
      </nav>
      <form onsubmit={(event) => { event.preventDefault(); void perform("apply_settings", "Settings applied.", { settings: { ...draft }, expectedRevision: draftRevision }); }}>
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
          <label for="port">Port <span>(optional)</span></label><input id="port" type="number" min="1" max="65535" value={draft.port ?? ""} oninput={(event) => draft.port = event.currentTarget.value === "" ? null : Number(event.currentTarget.value)} placeholder="From SSH config" disabled={!ready || busy} />
          <label for="identity">Identity file <span>(optional)</span></label><input id="identity" bind:value={draft.identityFile} placeholder="From SSH config or agent" autocomplete="off" disabled={!ready || busy} />
          <button type="button" class="test-button" disabled={!ready || !configured || dirty || busy} onclick={() => perform("test_connection", "SSH proxy and clipboard daemon responded.")}>Test saved connection</button>
          {#if dirty}<p class="field-help">Apply your changes before testing the connection.</p>{/if}
        {:else if section === "shortcuts"}
          <p class="section-intro">One shortcut opens the clipboard panel. You can also use the tray menu or launch the app.</p>
          <label class="toggle"><input type="checkbox" bind:checked={draft.shortcutEnabled} disabled={!ready || busy} />Enable Open panel shortcut</label>
          <label for="shortcut">Open panel</label>
          <button id="shortcut" type="button" class:recording class="shortcut-recorder" disabled={!ready || !draft.shortcutEnabled || busy} onclick={() => recording = !recording}>{recording ? "Press a shortcut · Esc to cancel" : displayShortcut(draft.openShortcut)}</button>
          <p class="field-help">{recording ? "Use Ctrl/Cmd or Alt with a key." : view.shortcutStatus}</p>
          <button type="button" class="text-button" disabled={!ready || busy} onclick={() => draft.openShortcut = "CommandOrControl+Shift+V"}>Restore default shortcut</button>
        {:else}
          <label class="toggle"><input type="checkbox" bind:checked={draft.showPreview} disabled={!ready || busy} />Show content previews</label>
          <p class="field-help">Hide text and image previews when sharing your screen.</p>
          <label class="toggle"><input type="checkbox" bind:checked={draft.closeAfterCopy} disabled={!ready || busy} />Hide panel after copying</label>
          <p class="field-help">The app keeps running so the copied content stays available.</p>
          <div class="capability"><strong>Tray menu</strong><span>{view.trayStatus}</span></div>
          <p class="field-help">Start at login and updates will be added after the desktop foundation is verified.</p>
        {/if}
        {#if staleDraft}<p class="field-help">Saved settings changed. Discard your draft to load them.</p>{/if}
        <div class="settings-actions"><button type="submit" class="primary" disabled={!ready || !view.settingsWritable || !dirty || staleDraft || busy || recording}>Apply settings</button><button type="button" disabled={!dirty || busy} onclick={() => { discardDraft(); inform("Loaded current saved settings."); }}>Discard changes</button></div>
      </form>
    {/if}

    {#if message}<div class:error class="feedback" role={error ? "alert" : "status"}>{message}</div>{/if}
  </main>

  <footer><span>{view.activeShortcut ? displayShortcut(view.activeShortcut) : "Open from the launcher or tray"}</span><span>Prototype</span><button class="quit-button" disabled={!ready || busy} onclick={() => command("quit_app").catch((cause) => inform(String(cause), true))}>Quit</button></footer>
</div>
