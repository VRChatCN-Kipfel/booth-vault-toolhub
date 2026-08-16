import { cpSync, existsSync, mkdirSync, readdirSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const root = resolve(scriptDir, "..", "..");
const names = ["booth.exe", "booth-mcp.exe", "booth-shell.exe"];

const candidates = [];
const collectRelease = (dir) => {
  const release = join(dir, "release");
  if (existsSync(release)) candidates.push(release);
};
collectRelease(join(root, "target"));
if (existsSync(join(root, "target"))) {
  for (const entry of readdirSync(join(root, "target"), { withFileTypes: true })) {
    if (entry.isDirectory()) collectRelease(join(root, "target", entry.name));
  }
}

const picked = new Map();
for (const dir of candidates) {
  for (const name of names) {
    const path = join(dir, name);
    if (!existsSync(path)) continue;
    const mtime = statSync(path).mtimeMs;
    if (!picked.has(name) || mtime > picked.get(name)[1]) {
      picked.set(name, [path, mtime]);
    }
  }
}

const missing = names.filter((name) => !picked.has(name));
if (missing.length > 0) {
  console.error(`[stage-cli] 缺少 CLI 二进制：${missing.join(", ")}`);
  console.error("[stage-cli] 请先执行 cargo build --release --workspace");
  process.exit(1);
}

const dest = resolve(scriptDir, "..", "src-tauri", "resources");
mkdirSync(dest, { recursive: true });
for (const [name, [src]] of picked) {
  cpSync(src, join(dest, name));
  console.log(`[stage-cli] ${name} <- ${src}`);
}
console.log("[stage-cli] 完成");
