import { readdirSync, statSync } from 'node:fs';
import { extname, join, relative } from 'node:path';

const EXCLUDE_BASENAMES = new Set(['openspec-bundled.js']);
const EXCLUDE_EXTENSIONS = new Set(['.map', '.png', '.jpg', '.jpeg', '.gif', '.webp', '.ico']);
const TEXT_EXTENSIONS = new Set([
  '.md',
  '.json',
  '.cjs',
  '.mjs',
  '.js',
  '.ts',
  '.sh',
  '.cmd',
  '.py',
  '.template',
  '.c4',
  '.rs',
  '.txt',
  '.yaml',
  '.yml',
]);

function isTextFile(filePath: string): boolean {
  const base = filePath.replace(/\\/g, '/').split('/').pop() ?? '';
  if (EXCLUDE_BASENAMES.has(base)) return false;
  const ext = extname(base).toLowerCase();
  if (EXCLUDE_EXTENSIONS.has(ext)) return false;
  if (ext === '') {
    // extensionless scripts such as bin/openspec
    return !base.includes('.');
  }
  return TEXT_EXTENSIONS.has(ext);
}

function walkDir(dir: string, rootDir: string, out: string[]): void {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      walkDir(full, rootDir, out);
      continue;
    }
    if (!entry.isFile()) continue;
    if (!isTextFile(full)) continue;
    out.push(relative(rootDir, full).replace(/\\/g, '/'));
  }
}

/** Enumerate text files under rootDir using wide includes / narrow excludes. */
export function scanTextFiles(rootDir: string): string[] {
  const st = statSync(rootDir);
  if (!st.isDirectory()) return [];
  const out: string[] = [];
  walkDir(rootDir, rootDir, out);
  return out;
}
