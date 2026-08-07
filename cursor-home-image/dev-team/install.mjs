#!/usr/bin/env node
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

function parseArgs(argv) {
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

function readJson(filePath) {
  return JSON.parse(readFileSync(filePath, 'utf-8'));
}

function writeJson(filePath, value) {
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`, 'utf-8');
}

/** Expand path tokens; normalize backslashes to `/` (for non-JSON text callers). */
export function expandHomePathTokens(text, absoluteRoot) {
  if (text == null) throw new Error('text is required');
  if (typeof text !== 'string') throw new Error('text must be a string');
  if (absoluteRoot == null || absoluteRoot === '') {
    throw new Error('absoluteRoot is required');
  }
  if (typeof absoluteRoot !== 'string') throw new Error('absoluteRoot must be a string');
  const normalized = absoluteRoot.replace(/\\/g, '/');
  return text
    .replaceAll('__DEV_TEAM_ROOT__', normalized)
    .replaceAll('__DEV_TEAM_RUNTIME_ROOT__', normalized)
    .replace(/\\/g, '/');
}

function expandPathTokens(text, absoluteRoot) {
  const normalized = absoluteRoot.replace(/\\/g, '/');
  return text
    .replaceAll('__DEV_TEAM_ROOT__', normalized)
    .replaceAll('__DEV_TEAM_RUNTIME_ROOT__', normalized);
}

function writeExpanded(srcFile, destFile, absoluteRoot) {
  mkdirSync(path.dirname(destFile), { recursive: true });
  const base = path.basename(srcFile);
  if (base === 'openspec-bundled.js' || base.endsWith('.map')) {
    writeFileSync(destFile, readFileSync(srcFile));
    return;
  }
  const raw = readFileSync(srcFile, 'utf-8');
  const expanded = expandPathTokens(raw, absoluteRoot);
  const isJson = base.endsWith('.json');
  writeFileSync(destFile, isJson ? expanded : expanded.replace(/\\/g, '/'), 'utf-8');
}

function listFilesRecursive(dir, base = dir) {
  const out = [];
  if (!existsSync(dir)) return out;
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) out.push(...listFilesRecursive(full, base));
    else if (entry.isFile()) out.push(path.relative(base, full).replace(/\\/g, '/'));
  }
  return out;
}

function requireNamePrefix(namePrefix) {
  if (typeof namePrefix !== 'string' || namePrefix === '') {
    throw new Error('manifest.namePrefix is required');
  }
  return namePrefix;
}

/** Paths that are merged or installer metadata — not blind-copied into the install root. */
function syncSkipSet(manifest) {
  return new Set([
    manifest.hooksFile ?? 'hooks.json',
    manifest.mcpFragment ?? 'mcp.dev-team.json',
    'install.mjs',
    'manifest.json',
  ]);
}

/**
 * Sync one manifest.managedPaths entry from the image into the install root.
 * Directories are replaced wholesale; files are written with path-token expansion.
 */
function syncManagedPath(rel, absoluteRoot) {
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

function isManagedHookEntry(entry, namePrefix) {
  return Boolean(
    entry &&
    typeof entry === 'object' &&
    typeof entry.command === 'string' &&
    entry.command.includes(namePrefix),
  );
}

/**
 * Upsert/remove managed hook entries (command contains namePrefix); keep user entries.
 */
export function mergeManagedHooks(existing, incoming, namePrefix) {
  requireNamePrefix(namePrefix);
  const result =
    existing && typeof existing === 'object'
      ? structuredClone(existing)
      : { version: 1, hooks: {} };
  if (!result.hooks || typeof result.hooks !== 'object') result.hooks = {};
  const imageHooks = incoming && typeof incoming === 'object' ? incoming : { hooks: {} };
  for (const [eventName, imageEntries] of Object.entries(imageHooks.hooks ?? {})) {
    if (!Array.isArray(imageEntries)) continue;
    const current = Array.isArray(result.hooks[eventName]) ? result.hooks[eventName] : [];
    const kept = current.filter((entry) => !isManagedHookEntry(entry, namePrefix));
    const next = imageEntries.map((entry) => structuredClone(entry));
    result.hooks[eventName] = [...kept, ...next];
  }
  // Drop managed entries for events absent from incoming (image deleted the hook).
  for (const eventName of Object.keys(result.hooks)) {
    if (Object.prototype.hasOwnProperty.call(imageHooks.hooks ?? {}, eventName)) continue;
    if (!Array.isArray(result.hooks[eventName])) continue;
    result.hooks[eventName] = result.hooks[eventName].filter(
      (entry) => !isManagedHookEntry(entry, namePrefix),
    );
  }
  return result;
}

/**
 * Upsert/remove mcpServers keys starting with namePrefix; keep other servers.
 */
export function mergeManagedMcp(existing, fragment, namePrefix) {
  requireNamePrefix(namePrefix);
  const result =
    existing && typeof existing === 'object' ? structuredClone(existing) : { mcpServers: {} };
  if (!result.mcpServers || typeof result.mcpServers !== 'object') result.mcpServers = {};
  for (const key of Object.keys(result.mcpServers)) {
    if (key.startsWith(namePrefix)) delete result.mcpServers[key];
  }
  const imageMcp = fragment && typeof fragment === 'object' ? fragment : {};
  for (const [key, server] of Object.entries(imageMcp.mcpServers ?? {})) {
    if (!key.startsWith(namePrefix)) continue;
    result.mcpServers[key] = structuredClone(server);
  }
  return result;
}

function mergeHooks(existing, imageHooks, absoluteRoot, namePrefix) {
  const expanded = structuredClone(imageHooks);
  for (const entries of Object.values(expanded.hooks ?? {})) {
    if (!Array.isArray(entries)) continue;
    for (const entry of entries) {
      if (typeof entry.command === 'string') {
        entry.command = expandPathTokens(entry.command, absoluteRoot);
      }
    }
  }
  return mergeManagedHooks(existing, expanded, namePrefix);
}

function mergeMcp(existing, imageMcp, absoluteRoot, namePrefix) {
  const expanded = structuredClone(imageMcp);
  for (const server of Object.values(expanded.mcpServers ?? {})) {
    if (server && Array.isArray(server.args)) {
      server.args = server.args.map((arg) =>
        typeof arg === 'string' ? expandPathTokens(arg, absoluteRoot) : arg,
      );
    }
  }
  return mergeManagedMcp(existing, expanded, namePrefix);
}

function main() {
  const { root } = parseArgs(process.argv.slice(2));
  const absoluteRoot = path.resolve(root);
  const manifest = readJson(path.join(IMAGE_ROOT, 'manifest.json'));
  const namePrefix = requireNamePrefix(manifest.namePrefix);
  if (!Array.isArray(manifest.managedPaths)) {
    throw new Error('manifest.managedPaths must be an array');
  }
  const skip = syncSkipSet(manifest);
  const managedPaths = [];

  mkdirSync(absoluteRoot, { recursive: true });
  for (const rel of manifest.managedPaths) {
    if (typeof rel !== 'string' || rel === '' || skip.has(rel)) continue;
    syncManagedPath(rel, absoluteRoot);
    managedPaths.push(rel.replace(/\\/g, '/'));
  }

  const hooksFile = manifest.hooksFile ?? 'hooks.json';
  const hooksPath = path.join(absoluteRoot, 'hooks.json');
  const existingHooks = existsSync(hooksPath) ? readJson(hooksPath) : null;
  writeJson(
    hooksPath,
    mergeHooks(existingHooks, readJson(path.join(IMAGE_ROOT, hooksFile)), absoluteRoot, namePrefix),
  );

  const mcpPath = path.join(absoluteRoot, 'mcp.json');
  const mcpFragment = manifest.mcpFragment ?? 'mcp.dev-team.json';
  const existingMcp = existsSync(mcpPath) ? readJson(mcpPath) : null;
  writeJson(
    mcpPath,
    mergeMcp(existingMcp, readJson(path.join(IMAGE_ROOT, mcpFragment)), absoluteRoot, namePrefix),
  );

  writeJson(path.join(absoluteRoot, STATE_FILE), {
    version: manifest.version,
    root: absoluteRoot,
    installedAt: new Date().toISOString(),
    managedPaths: [...new Set(managedPaths)].sort(),
  });

  process.stdout.write(
    `dev-team home image v${manifest.version} installed to ${absoluteRoot}\n` +
      'Please run "Developer: Reload Window" in Cursor to pick up skills/agents/hooks/mcp.\n',
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
