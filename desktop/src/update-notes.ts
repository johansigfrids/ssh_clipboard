// Display only the small plain-bullet format we recognize. Other feeds retain
// their complete inert text in a disclosure; never discard or execute markup.
export function conciseUpdateNotes(notes: string, version: string | null): string[] | null {
  if (!version || notes.length > 1200) return null;
  const [heading, ...body] = notes.replace(/\r\n/g, "\n").trim().split("\n");
  const bullets = body.filter((line) => line.trim() !== "");
  if (heading !== `# ${version}` || !bullets.length || bullets.length > 6 || bullets.some((line) => !/^- \S/.test(line))) return null;
  return bullets.map((line) => line.slice(2));
}
