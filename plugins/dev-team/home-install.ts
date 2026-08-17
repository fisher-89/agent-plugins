/**
 * Install cursor-home-image/dev-team into a Cursor user home root (default ~/.cursor).
 *
 * Usage: node install.mjs [--root <path>]
 */
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

const IMAGE_ROOT = path.dirname(fileURLToPath(import.meta.url));
const STATE_FILE = 'dev-team-install.json';

type JsonRecord = Record<string, unknown>;

function isRecord(value: unknown): value is JsonRecord {
  return value !== null && typeof value === 'object';
}

function cloneRecord(value: unknown): JsonRecord {
  return isRecord(value) ? structuredClone(value) : {};
}

function parseArgs(argv: string[]): { root: string } {
  let root = path.resolve(os.homedir(), '.cursor');
  for (let i = 0; i < argv.length; i += 1) {
    if (argv[i] === '--root') {
      const value = argv[i + 1];
      if (!value) throw new Error('--root requires a path');
      root = path.resolve(value);
      i += 1;
    }
  }
  return { root };
}

function offsetToLineCol(text: string, offset: number): { line: number; column: number } {
  let line = 1;
  let column = 1;
  const end = Math.min(Math.max(offset, 0), text.length);
  for (let i = 0; i < end; i += 1) {
    if (text[i] === '\n') {
      line += 1;
      column = 1;
    } else {
      column += 1;
    }
  }
  return { line, column };
}

function formatJsonParseError(filePath: string, raw: string, error: unknown): Error {
  const reason = error instanceof Error ? error.message : String(error);
  const offsetText = /position\s+(\d+)/i.exec(reason)?.[1];
  if (offsetText === undefined) {
    return new Error(`Invalid JSON in ${filePath}: ${reason}`);
  }
  const { line, column } = offsetToLineCol(raw, Number(offsetText));
  return new Error(`Invalid JSON in ${filePath}:${line}:${column}: ${reason}`);
}

function readJson(filePath: string): unknown {
  let raw: string;
  try {
    raw = readFileSync(filePath, 'utf-8');
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`Failed to read JSON file ${filePath}: ${reason}`);
  }
  // JSON.parse rejects a leading UTF-8 BOM (U+FEFF); editors on Windows often add one.
  if (raw.charCodeAt(0) === 0xfeff) {
    raw = raw.slice(1);
  }
  try {
    return JSON.parse(raw);
  } catch (error) {
    throw formatJsonParseError(filePath, raw, error);
  }
}

function writeJson(filePath: string, value: unknown): void {
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`, 'utf-8');
}

/** Expand path tokens; normalize Windows separators in the install root only. */
export function expandHomePathTokens(text: unknown, absoluteRoot: unknown): string {
  if (text == null) throw new Error('text is required');
  if (typeof text !== 'string') throw new Error('text must be a string');
  if (absoluteRoot == null || absoluteRoot === '') {
    throw new Error('absoluteRoot is required');
  }
  if (typeof absoluteRoot !== 'string') throw new Error('absoluteRoot must be a string');
  return expandPathTokens(text, absoluteRoot);
}

function expandPathTokens(text: string, absoluteRoot: string): string {
  const normalized = absoluteRoot.replace(/\\/g, '/');
  return text.replaceAll('__INSTALL_PLUGIN_ROOT__', normalized);
}

function writeExpanded(srcFile: string, destFile: string, absoluteRoot: string): void {
  mkdirSync(path.dirname(destFile), { recursive: true });
  const base = path.basename(srcFile);
  if (base.endsWith('.map')) {
    writeFileSync(destFile, readFileSync(srcFile));
    return;
  }
  const raw = readFileSync(srcFile, 'utf-8');
  writeFileSync(destFile, expandPathTokens(raw, absoluteRoot), 'utf-8');
}

function listFilesRecursive(dir: string, base = dir): string[] {
  const out: string[] = [];
  if (!existsSync(dir)) return out;
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) out.push(...listFilesRecursive(full, base));
    else if (entry.isFile()) out.push(path.relative(base, full).replace(/\\/g, '/'));
  }
  return out;
}

function requireNamePrefix(namePrefix: unknown): string {
  if (typeof namePrefix !== 'string' || namePrefix === '') {
    throw new Error('manifest.namePrefix is required');
  }
  return namePrefix;
}

function hooksMap(doc: JsonRecord): JsonRecord {
  return isRecord(doc.hooks) ? doc.hooks : {};
}

function mcpServersMap(doc: JsonRecord): JsonRecord {
  return isRecord(doc.mcpServers) ? doc.mcpServers : {};
}

/** Paths that are merged or installer metadata — not blind-copied into the install root. */
function syncSkipSet(manifest: JsonRecord): Set<string> {
  const hooksFile = typeof manifest.hooksFile === 'string' ? manifest.hooksFile : 'hooks.json';
  const mcpFragment = typeof manifest.mcpFragment === 'string' ? manifest.mcpFragment : 'mcp.json';
  return new Set([hooksFile, mcpFragment, 'install.mjs', 'manifest.json']);
}

/**
 * Sync one manifest.managedPaths entry from the image into the install root.
 * Directories are replaced wholesale; files are written with path-token expansion.
 */
function syncManagedPath(rel: string, absoluteRoot: string): void {
  const src = path.join(IMAGE_ROOT, rel);
  const dest = path.join(absoluteRoot, rel);
  if (!existsSync(src)) {
    throw new Error(`managed path missing from image: ${rel}`);
  }
  if (statSync(src).isDirectory()) {
    if (existsSync(dest)) rmSync(dest, { recursive: true, force: true });
    mkdirSync(dest, { recursive: true });
    for (const child of listFilesRecursive(src)) {
      writeExpanded(path.join(src, child), path.join(dest, child), absoluteRoot);
    }
  } else {
    writeExpanded(src, dest, absoluteRoot);
  }
}

function isManagedHookEntry(entry: unknown, namePrefix: string): boolean {
  if (!isRecord(entry)) return false;
  const command = entry.command;
  return typeof command === 'string' && command.includes(namePrefix);
}

/**
 * Upsert/remove managed hook entries (command contains namePrefix); keep user entries.
 */
export function mergeManagedHooks(
  existing: unknown,
  incoming: unknown,
  namePrefix: string,
): JsonRecord {
  requireNamePrefix(namePrefix);
  const emptyHooks: JsonRecord = {};
  const result: JsonRecord = isRecord(existing)
    ? structuredClone(existing)
    : { version: 1, hooks: emptyHooks };
  if (!isRecord(result.hooks)) result.hooks = {};
  const hooks = hooksMap(result);
  const imageHooks = hooksMap(cloneRecord(incoming));
  for (const [eventName, imageEntries] of Object.entries(imageHooks)) {
    if (!Array.isArray(imageEntries)) continue;
    const current = Array.isArray(hooks[eventName]) ? hooks[eventName] : [];
    const kept = current.filter((entry) => !isManagedHookEntry(entry, namePrefix));
    const next = imageEntries.map((entry) => structuredClone(entry));
    hooks[eventName] = [...kept, ...next];
  }
  // Drop managed entries for events absent from incoming (image deleted the hook).
  for (const eventName of Object.keys(hooks)) {
    if (Object.prototype.hasOwnProperty.call(imageHooks, eventName)) continue;
    const current = hooks[eventName];
    if (!Array.isArray(current)) continue;
    hooks[eventName] = current.filter((entry) => !isManagedHookEntry(entry, namePrefix));
  }
  result.hooks = hooks;
  return result;
}

/**
 * Upsert/remove mcpServers keys starting with namePrefix; keep other servers.
 */
export function mergeManagedMcp(
  existing: unknown,
  fragment: unknown,
  namePrefix: string,
): JsonRecord {
  requireNamePrefix(namePrefix);
  const result = isRecord(existing) ? structuredClone(existing) : { mcpServers: {} };
  if (!isRecord(result.mcpServers)) result.mcpServers = {};
  const servers = mcpServersMap(result);
  for (const key of Object.keys(servers)) {
    if (key.startsWith(namePrefix)) delete servers[key];
  }
  const imageServers = mcpServersMap(cloneRecord(fragment));
  for (const [key, server] of Object.entries(imageServers)) {
    if (!key.startsWith(namePrefix)) continue;
    servers[key] = structuredClone(server);
  }
  result.mcpServers = servers;
  return result;
}

function mergeHooks(
  existing: unknown,
  imageHooks: unknown,
  absoluteRoot: string,
  namePrefix: string,
): JsonRecord {
  const expanded = cloneRecord(imageHooks);
  for (const entries of Object.values(hooksMap(expanded))) {
    if (!Array.isArray(entries)) continue;
    for (const entry of entries) {
      if (!isRecord(entry) || typeof entry.command !== 'string') continue;
      entry.command = expandPathTokens(entry.command, absoluteRoot);
    }
  }
  return mergeManagedHooks(existing, expanded, namePrefix);
}

function mergeMcp(
  existing: unknown,
  imageMcp: unknown,
  absoluteRoot: string,
  namePrefix: string,
): JsonRecord {
  const expanded = cloneRecord(imageMcp);
  for (const server of Object.values(mcpServersMap(expanded))) {
    if (!isRecord(server) || !Array.isArray(server.args)) continue;
    server.args = server.args.map((arg) =>
      typeof arg === 'string' ? expandPathTokens(arg, absoluteRoot) : arg,
    );
  }
  return mergeManagedMcp(existing, expanded, namePrefix);
}

function readManifest(): JsonRecord {
  const raw = readJson(path.join(IMAGE_ROOT, 'manifest.json'));
  if (!isRecord(raw)) throw new Error('invalid manifest.json');
  return raw;
}

function main(): void {
  const { root } = parseArgs(process.argv.slice(2));
  const absoluteRoot = path.resolve(root);
  const manifest = readManifest();
  const namePrefix = requireNamePrefix(manifest.namePrefix);
  if (!Array.isArray(manifest.managedPaths)) {
    throw new Error('manifest.managedPaths must be an array');
  }
  const skip = syncSkipSet(manifest);
  const managedPaths: string[] = [];

  mkdirSync(absoluteRoot, { recursive: true });
  for (const rel of manifest.managedPaths) {
    if (typeof rel !== 'string' || rel === '' || skip.has(rel)) continue;
    syncManagedPath(rel, absoluteRoot);
    managedPaths.push(rel.replace(/\\/g, '/'));
  }

  writeMergedConfigs(absoluteRoot, manifest, namePrefix);
  writeJson(path.join(absoluteRoot, STATE_FILE), {
    version: manifest.version,
    root: absoluteRoot,
    installedAt: new Date().toISOString(),
    managedPaths: [...new Set(managedPaths)].sort(),
  });

  process.stdout.write(
    `dev-team home image v${String(manifest.version)} installed to ${absoluteRoot}\n` +
      'Please run "Developer: Reload Window" in Cursor to pick up skills/agents/hooks/mcp.\n',
  );
}

function writeMergedConfigs(absoluteRoot: string, manifest: JsonRecord, namePrefix: string): void {
  const hooksFile = typeof manifest.hooksFile === 'string' ? manifest.hooksFile : 'hooks.json';
  const hooksPath = path.join(absoluteRoot, 'hooks.json');
  const existingHooks = existsSync(hooksPath) ? readJson(hooksPath) : null;
  writeJson(
    hooksPath,
    mergeHooks(existingHooks, readJson(path.join(IMAGE_ROOT, hooksFile)), absoluteRoot, namePrefix),
  );

  const mcpPath = path.join(absoluteRoot, 'mcp.json');
  const mcpFragment = typeof manifest.mcpFragment === 'string' ? manifest.mcpFragment : 'mcp.json';
  const existingMcp = existsSync(mcpPath) ? readJson(mcpPath) : null;
  writeJson(
    mcpPath,
    mergeMcp(existingMcp, readJson(path.join(IMAGE_ROOT, mcpFragment)), absoluteRoot, namePrefix),
  );
}

const isDirectRun =
  process.argv[1] != null && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (isDirectRun) {
  try {
    main();
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exit(1);
  }
}
