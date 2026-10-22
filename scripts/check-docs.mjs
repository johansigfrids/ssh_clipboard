// Check current Markdown links, including new guides, before staging or builds.
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';

const root = process.cwd();
const files = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z', '*.md'], { encoding: 'utf8' }).split('\0').filter(file => file && existsSync(file));
const failures = [];
function headings(text) {
  const seen = new Map();
  return new Set([...text.matchAll(/^#{1,6}\s+(.+?)\s*#*$/gm)].map(([, title]) => {
    const slug = title.toLowerCase().replace(/<[^>]*>/g, '').replace(/[^\p{L}\p{N}_\-\s]/gu, '').replace(/\s/g, '-');
    const count = seen.get(slug) ?? 0;
    seen.set(slug, count + 1);
    return count ? `${slug}-${count}` : slug;
  }));
}
for (const file of files) {
  const text = readFileSync(file, 'utf8').replace(/^```[^\n]*\n[\s\S]*?^```\s*$/gm, '');
  for (const [, raw] of text.matchAll(/!?\[[^\]\n]*\]\(([^\s)]+)(?:\s+"[^"]*")?\)/g)) {
    if (/^[a-z][a-z\d+.-]*:/i.test(raw)) continue;
    const [relative, anchor] = raw.replace(/^<|>$/g, '').split('#');
    const target = path.resolve(path.dirname(file), decodeURIComponent(relative || path.basename(file)));
    if (!existsSync(target)) failures.push(`${file}: missing ${raw}`);
    else if (anchor && target.endsWith('.md') && !headings(readFileSync(target, 'utf8')).has(decodeURIComponent(anchor))) failures.push(`${file}: missing heading ${raw}`);
  }
}
const index = readFileSync('docs/index.md', 'utf8');
for (const file of files.filter(file => /^docs\/[^/]+\.md$/.test(file) && file !== 'docs/index.md')) {
  if (!index.includes(`](${file.slice(5)}`)) failures.push(`docs/index.md: missing guide ${file}`);
}
if (failures.length) {
  console.error(failures.join('\n'));
  process.exitCode = 1;
} else console.log(`Checked links and headings in ${files.length} Markdown files and current guide index coverage.`);
