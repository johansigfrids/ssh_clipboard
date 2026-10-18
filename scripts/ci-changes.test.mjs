import test from 'node:test';
import assert from 'node:assert/strict';
import { selectChanges } from './ci-changes.mjs';

test('documentation skips compilation, but executable and unfamiliar inputs fail closed', () => {
  assert.deepEqual(selectChanges(['docs/testing.md', 'README.md']), { core: false, desktop: false, packages: false });
  for (const file of ['docs/update-signing-public-key.pub', 'Cargo.lock', '.github/workflows/ci.yml', '.cargo/config.toml', 'scripts/new-tool.py']) {
    assert.deepEqual(selectChanges([file]), { core: true, desktop: true, packages: true }, file);
  }
});

test('core changes reach desktop consumers while UI changes retain standalone checks without repackaging', () => {
  assert.deepEqual(selectChanges(['src/shelf.rs']), { core: true, desktop: true, packages: false });
  assert.deepEqual(selectChanges(['desktop/src/style.css']), { core: false, desktop: true, packages: false });
  for (const file of ['desktop/src-tauri/build.rs', 'desktop/src-tauri/src/panel.rs', 'desktop/package-lock.json', 'desktop/release-notes/0.4.2.md']) {
    assert.deepEqual(selectChanges([file]), { core: false, desktop: true, packages: true }, file);
  }
  // Both names of renames/deletions are supplied by git diff --no-renames.
  assert.equal(selectChanges(['desktop/src/App.svelte', 'docs/moved.md']).desktop, true);
});
