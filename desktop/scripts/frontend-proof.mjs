import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const desktop = fileURLToPath(new URL("../", import.meta.url));
const proofFile = root => path.join(root, "artifacts", "frontend-proof.json");

function tree(root, relative) {
  const absolute = path.join(root, relative);
  if (!existsSync(absolute)) return [];
  return readdirSync(absolute, { withFileTypes: true }).flatMap(entry => {
    const child = `${relative}/${entry.name}`;
    if (entry.isSymbolicLink()) throw new Error(`Frontend proof refuses symlinks: ${child}`);
    return entry.isDirectory() ? tree(root, child) : [child];
  });
}

function digest(root, files) {
  const hash = createHash("sha256");
  for (const file of files.toSorted()) hash.update(file).update("\0").update(readFileSync(path.join(root, file))).update("\0");
  return hash.digest("hex");
}

function snapshot(root, env) {
  const inputs = ["package.json", "package-lock.json", "tsconfig.json", "vite.config.ts", "index.html",
    ...tree(root, "src"), ...tree(root, "public"), ...tree(root, "tests/fixtures"),
    ...readdirSync(root).filter(file => file === ".env" || file.startsWith(".env."))];
  if (!existsSync(path.join(root, "dist/index.html"))) throw new Error("Build the frontend before recording or reusing it.");
  return {
    version: 1, node: process.versions.node,
    environment: createHash("sha256").update(JSON.stringify(Object.entries(env).filter(([key]) => key === "NODE_ENV" || key.startsWith("VITE_")).toSorted(([a], [b]) => a.localeCompare(b)))).digest("hex"),
    inputs: digest(root, inputs), outputs: digest(root, tree(root, "dist")),
  };
}

export function recordFrontend(root = desktop, env = process.env) {
  const proof = snapshot(root, env);
  mkdirSync(path.dirname(proofFile(root)), { recursive: true });
  writeFileSync(proofFile(root), JSON.stringify(proof));
}

export function prebuiltFrontendConfig(root = desktop, env = process.env) {
  if (env.SSH_CLIPBOARD_PREBUILT_FRONTEND !== "1") return {};
  let proof;
  try { proof = JSON.parse(readFileSync(proofFile(root), "utf8")); }
  catch { throw new Error("Prebuilt frontend requires proof from the successful check/build step."); }
  if (JSON.stringify(proof) !== JSON.stringify(snapshot(root, env))) throw new Error("Prebuilt frontend inputs or output changed; run the frontend check/build again.");
  console.log("Reusing checked frontend: source, toolchain, environment and output hashes match.");
  return { build: { beforeBuildCommand: "" } };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  if (process.argv.length !== 3 || process.argv[2] !== "--record") throw new Error("Use --record after a successful npm run build.");
  recordFrontend();
}
