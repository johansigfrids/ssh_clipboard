import type { PanelView } from "../../src/api";
import { panelResponder } from "../helpers/panel-responder";

export interface BrowserFixture {
  state: PanelView;
  calls: Array<{ name: string; args?: Record<string, unknown> }>;
}
declare global { interface Window { __panelFixture: BrowserFixture } }

export const isTauri = () => true;
export async function invoke<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  window.__panelFixture.calls.push({ name, args });
  return await panelResponder(() => window.__panelFixture.state)(name, args) as T;
}
