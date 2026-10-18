import test from 'node:test';
import assert from 'node:assert/strict';
import { selectChanges } from './ci-changes.mjs';

test('documentation skips compilation, but executable and unfamiliar inputs fail closed', () => {
  assert.deepEqual(selectChanges(['docs/testing.md', 'README.md']), { core: false, desktop: false, packages: false, maintenance: false });
  for (const file of ['docs/update-signing-public-key.pub', 'docs/images/key.pub', 'docs/images/run.mjs', 'docs/unknown.png', 'Cargo.lock', '.github/workflows/ci.yml', '.cargo/config.toml', 'scripts/new-tool.py']) {
    assert.deepEqual(selectChanges([file]), { core: true, desktop: true, packages: true, maintenance: true }, file);
  }
});

test('documentation images do not widen UI changes or renamed/deleted screenshot checks', () => {
  const images = ['docs/images/old.png', 'docs/images/new.png', 'docs/images/nested/panel.JPG', 'docs/images/panel.jpeg', 'docs/images/panel.gif', 'docs/images/panel.webp', 'docs/images/panel.svg'];
  assert.deepEqual(selectChanges(images), { core: false, desktop: false, packages: false, maintenance: false });
  assert.deepEqual(selectChanges(['desktop/src/style.css', 'docs/history/desktop-ux-plan.md', ...images]), { core: false, desktop: true, packages: false, maintenance: false });
  assert.deepEqual(selectChanges(['docs/images/panel.png', 'docs/update-signing-public-key.pub']), { core: true, desktop: true, packages: true, maintenance: true });
});

test('core changes reach desktop consumers while UI changes retain standalone checks without repackaging', () => {
  assert.deepEqual(selectChanges(['src/shelf.rs']), { core: true, desktop: true, packages: false, maintenance: true });
  assert.deepEqual(selectChanges(['desktop/src/style.css']), { core: false, desktop: true, packages: false, maintenance: false });
  for (const file of ['desktop/src-tauri/build.rs', 'desktop/src-tauri/src/panel.rs', 'desktop/package-lock.json', 'desktop/release-notes/0.4.2.md']) {
    assert.deepEqual(selectChanges([file]), { core: false, desktop: true, packages: true, maintenance: file === 'desktop/package-lock.json' }, file);
  }
  // Both names of renames/deletions are supplied by git diff --no-renames.
  assert.equal(selectChanges(['desktop/src/App.svelte', 'docs/moved.md']).desktop, true);
});

test('server smoke tooling selects the isolated maintenance lane without native desktop builds', () => {
  for (const file of ['scripts/test-managed-server.py', 'desktop/scripts/smoke-server-maintenance.mjs', 'desktop/scripts/smoke-server-package.mjs', 'desktop/scripts/build-server.mjs']) {
    assert.deepEqual(selectChanges([file]), { core: false, desktop: false, packages: false, maintenance: true }, file);
  }
});
