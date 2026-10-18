import { expect, test, type Page } from "@playwright/test";
import { ipc } from "../fixtures/ipc";
import type { PanelView } from "../../src/api";
import type {} from "./tauri-core";

async function openPanel(page: Page, state: PanelView = structuredClone(ipc.legacy)) {
  // Block accidental external navigation/fetches. This suite has no native or SSH adapters.
  await page.route("**/*", route => new URL(route.request().url()).origin === "http://127.0.0.1:1421" ? route.continue() : route.abort());
  await page.addInitScript(state => { window.__panelFixture = { state, calls: [] }; }, state);
  await page.goto("/");
  await expect(page.getByRole("button", { name: "Open settings" })).toBeEnabled();
}

test("real Tab navigation, settings arrows and confirmation cancellation restore focus without commands", async ({ page }) => {
  const state: PanelView = structuredClone(ipc.legacy);
  state.profiles.push({ ...structuredClone(state.profiles[0]), id: "other", name: "Other connection" });
  await openPanel(page, state);
  const open = page.getByRole("button", { name: "Open settings" });
  await open.focus();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("tab", { name: "Connection", exact: true })).toBeFocused();
  await page.keyboard.press("ArrowRight");
  await expect(page.getByRole("tab", { name: "Shortcuts", exact: true })).toBeFocused();
  await page.keyboard.press("End");
  await expect(page.getByRole("tab", { name: "Updates", exact: true })).toBeFocused();
  await page.keyboard.press("Home");
  await page.keyboard.press("Tab");
  await expect(page.getByRole("combobox", { name: "Connection", exact: true })).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(page.getByRole("button", { name: "Add connection" })).toBeFocused();
  await page.keyboard.press("Tab");
  const remove = page.getByRole("button", { name: "Remove…", exact: true });
  await expect(remove).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("button", { name: "Remove connection", exact: true })).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(page.getByRole("button", { name: "Cancel", exact: true })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(remove).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(open).toBeFocused();
  expect(await page.evaluate(() => window.__panelFixture.calls.map(call => call.name))).toEqual(["get_operation", "get_panel"]);
});

test("minimum panel size keeps Apply and Discard reachable while long update content scrolls", async ({ page }) => {
  await page.setViewportSize({ width: 340, height: 440 });
  const state: PanelView = structuredClone(ipc.legacy);
  state.settings.profileName = "A long connection name that should wrap within the supported minimum panel width";
  state.profiles[0].name = state.settings.profileName;
  state.updateStatus = { ...ipc.updates, notes: Array.from({ length: 40 }, (_, i) => `- Change ${i}: long synthetic release notes that require scrolling in a small panel.`).join("\n") };
  await openPanel(page, state);
  await page.getByRole("button", { name: "Open settings" }).click();
  await page.getByRole("textbox", { name: "Connection name", exact: true }).fill("Changed connection name");
  await page.getByRole("tab", { name: "Updates", exact: true }).click();
  const main = page.getByRole("main");
  await main.hover();
  await page.mouse.wheel(0, 10000);
  await expect.poll(() => main.evaluate(element => element.scrollTop)).toBeGreaterThan(0);
  for (const name of ["Apply settings", "Discard changes"]) {
    const button = page.getByRole("button", { name, exact: true });
    await expect(button).toBeInViewport({ ratio: 1 });
    await button.click({ trial: true }); // Also verifies it is not clipped or covered.
  }
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.getByRole("button", { name: "Apply settings", exact: true }).click();
  await expect(page.getByText("All changes saved", { exact: true })).toBeVisible();
  const applied = await page.evaluate(() => window.__panelFixture.calls.filter(call => call.name === "apply_settings"));
  expect(applied).toHaveLength(1);
  expect(applied[0].args?.expectedRevision).toBe("1");
  expect((applied[0].args?.settings as PanelView["settings"]).profileName).toBe("Changed connection name");
  await page.getByRole("tab", { name: "Connection", exact: true }).click();
  await page.getByRole("textbox", { name: "Connection name", exact: true }).fill("Discard this draft");
  await page.getByRole("tab", { name: "Updates", exact: true }).click();
  await page.getByRole("button", { name: "Discard changes", exact: true }).click();
  await expect(page.getByText("All changes saved", { exact: true })).toBeVisible();
  expect(await page.evaluate(() => window.__panelFixture.calls.filter(call => call.name === "apply_settings").length)).toBe(1);
});

test("native form validation reveals a collapsed invalid field and blocks IPC until corrected", async ({ page }) => {
  await openPanel(page);
  await page.getByRole("button", { name: "Open settings" }).click();
  const summary = page.getByText("Advanced connection settings", { exact: true });
  await summary.click();
  const port = page.getByRole("spinbutton", { name: "Port (optional)", exact: true });
  await port.fill("70000");
  await summary.click();
  await expect(port).toBeHidden();
  await page.getByRole("button", { name: "Apply settings", exact: true }).click();
  await expect(port).toBeVisible();
  await expect(port).toBeFocused();
  expect(await port.evaluate(element => (element as HTMLInputElement).validity.rangeOverflow)).toBe(true);
  expect(await page.evaluate(() => window.__panelFixture.calls.map(call => call.name))).toEqual(["get_operation", "get_panel"]);
  await port.fill("22");
  await page.keyboard.press("Enter");
  await expect(page.getByText("All changes saved", { exact: true })).toBeVisible();
  const applied = await page.evaluate(() => window.__panelFixture.calls.filter(call => call.name === "apply_settings"));
  expect(applied).toHaveLength(1);
  expect((applied[0].args?.settings as PanelView["settings"]).port).toBe(22);
});
