// Unknown inputs deliberately select every check. Paths come from Git, never shell text.
import { execFileSync } from 'node:child_process';
import { appendFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

export function selectChanges(paths) {
  const selected = { core: false, desktop: false, packages: false };
  for (const file of paths) {
    if (/^(docs\/.*\.md|README\.md|ARCHITECTURE\.md|CHANGELOG\.md|AGENTS\.md|LICENSE)$/.test(file)) continue;
    if (/^(src\/|tests\/)/.test(file)) {
      selected.core = selected.desktop = true; // Desktop links the shared core.
    } else if (/^desktop\/(src\/|tests\/)/.test(file)) {
      selected.desktop = true;
    } else if (/^desktop\//.test(file)) {
      selected.desktop = selected.packages = true;
    } else {
      return { core: true, desktop: true, packages: true };
    }
  }
  return selected;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [base, head = 'HEAD'] = process.argv.slice(2);
  const args = !base || /^0+$/.test(base)
    ? ['ls-files', '-z']
    : ['diff', '--name-only', '--no-renames', '-z', base, head];
  const paths = execFileSync('git', args, { encoding: 'utf8' }).split('\0').filter(Boolean);
  const selected = selectChanges(paths);
  for (const [key, value] of Object.entries(selected)) {
    console.log(`${key}=${value}`);
    if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, `${key}=${value}\n`);
  }
}
