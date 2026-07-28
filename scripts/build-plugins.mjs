import { spawn } from 'node:child_process';
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const PLUGIN_NAME = 'dev-team';
const DEFAULT_DESCRIPTION =
  'A custom Claude Code plugin for enhancing development workflow with OpenSpec integration';
const DEFAULT_META = {
  description: DEFAULT_DESCRIPTION,
  bin: './bin',
  openspecVersion: '1.2.0',
  author: { name: 'zhangbohan' },
};

const ALLOWED_TOP_DIRS = new Set(['agents', 'skills', 'hooks', 'utils', 'templates']);
const BIN_RUNTIME_FILES = new Set([
  'bin/dev-team-mcp.cjs',
  'bin/dev-team-cli.cjs',
  'bin/dev-team-hooks.cjs',
  'bin/dev-team-config.schema.json',
  'bin/openspec',
  'bin/openspec.cmd',
  'bin/openspec-bundled.js',
]);

function requireNonEmptyString(value, name) {
  if (value == null || typeof value !== 'string' || value === '') {
    throw new Error(`${name} must be a non-empty string`);
  }
  return value;
}

function normalizeRelativePath(relativePath) {
  return relativePath.replaceAll('\\', '/').replace(/^\.\//, '');
}

/**
 * Read the authoritative plugin version from package.json.
 * @param {string} packageJsonPath
 * @returns {string}
 */
export function readPackageVersion(packageJsonPath) {
  requireNonEmptyString(packageJsonPath, 'packageJsonPath');
  const raw = readFileSync(packageJsonPath, 'utf8');
  const parsed = JSON.parse(raw);
  if (parsed == null || typeof parsed !== 'object' || Array.isArray(parsed)) {
    throw new Error(`Invalid package.json at ${packageJsonPath}: expected an object`);
  }
  if (typeof parsed.version !== 'string' || parsed.version === '') {
    throw new Error(`Missing or empty version in ${packageJsonPath}`);
  }
  return parsed.version;
}

/**
 * Run vite-plus `vp pack` with cwd at the plugin source root.
 * @param {string} pluginRoot
 * @returns {Promise<void>}
 */
export function runVpPack(pluginRoot) {
  requireNonEmptyString(pluginRoot, 'pluginRoot');
  return new Promise((resolve, reject) => {
    // On Windows, vite-plus exposes `vp` via a .cmd shim; shell is required for PATH lookup.
    const child = spawn('vp pack', {
      cwd: pluginRoot,
      stdio: 'inherit',
      shell: true,
    });
    child.on('error', reject);
    child.on('close', (code) => {
      if (code === 0) {
        resolve();
        return;
      }
      reject(new Error(`vp pack failed with exit code ${code}`));
    });
  });
}

/**
 * Whether a source-relative path belongs in an installable product tree.
 * @param {string} relativePath
 * @returns {boolean}
 */
export function shouldCopyPath(relativePath) {
  if (relativePath == null || typeof relativePath !== 'string' || relativePath === '') {
    return false;
  }
  const norm = normalizeRelativePath(relativePath);
  if (norm.includes('\0')) {
    return false;
  }
  if (norm === '.claude-plugin' || norm.startsWith('.claude-plugin/')) {
    return false;
  }
  if (norm === '.cursor-plugin' || norm.startsWith('.cursor-plugin/')) {
    return false;
  }
  if (norm === '.mcp.json') {
    return true;
  }
  if (norm.split('/').includes('node_modules')) {
    return false;
  }
  if (norm.endsWith('.map')) {
    return false;
  }

  const [top] = norm.split('/');
  if (ALLOWED_TOP_DIRS.has(top)) {
    return true;
  }

  return BIN_RUNTIME_FILES.has(norm);
}

function walkFiles(dir, baseDir, out) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const abs = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      walkFiles(abs, baseDir, out);
      continue;
    }
    if (entry.isFile()) {
      out.push(path.relative(baseDir, abs));
    }
  }
}

/**
 * Clear dest and sync installable plugin content from src.
 * @param {string} srcRoot
 * @param {string} destRoot
 * @returns {Promise<void>}
 */
export async function assembleProductTree(srcRoot, destRoot) {
  requireNonEmptyString(srcRoot, 'srcRoot');
  requireNonEmptyString(destRoot, 'destRoot');
  if (!existsSync(srcRoot) || !statSync(srcRoot).isDirectory()) {
    throw new Error(`Source root does not exist or is not a directory: ${srcRoot}`);
  }

  rmSync(destRoot, { recursive: true, force: true });
  mkdirSync(destRoot, { recursive: true });

  const relatives = [];
  walkFiles(srcRoot, srcRoot, relatives);
  for (const rel of relatives) {
    if (!shouldCopyPath(rel)) {
      continue;
    }
    const from = path.join(srcRoot, rel);
    const to = path.join(destRoot, rel);
    mkdirSync(path.dirname(to), { recursive: true });
    copyFileSync(from, to);
  }
}

/**
 * Reserved hook for Cursor-only extras (e.g. dedicated mcp.json). No-op this iteration.
 * @param {string} _productRoot
 * @returns {Promise<void>}
 */
export async function assembleCursorExtras(_productRoot) {
  // Intentionally empty — Cursor may later receive platform-specific extras.
}

/**
 * Write Claude or Cursor platform plugin.json into a product tree.
 * @param {string} productRoot
 * @param {'claude'|'cursor'} platform
 * @param {string} version
 * @param {object} [meta]
 * @returns {Promise<void>}
 */
export async function writePlatformManifest(productRoot, platform, version, meta) {
  requireNonEmptyString(productRoot, 'productRoot');
  requireNonEmptyString(platform, 'platform');
  requireNonEmptyString(version, 'version');
  if (platform !== 'claude' && platform !== 'cursor') {
    throw new Error(`Unsupported platform: ${platform}`);
  }

  const dirName = platform === 'claude' ? '.claude-plugin' : '.cursor-plugin';
  const manifestDir = path.join(productRoot, dirName);
  mkdirSync(manifestDir, { recursive: true });

  const base = {
    name: PLUGIN_NAME,
    description: DEFAULT_META.description,
    version,
    ...(platform === 'claude'
      ? {
          bin: DEFAULT_META.bin,
          openspecVersion: DEFAULT_META.openspecVersion,
          author: DEFAULT_META.author,
        }
      : {
          // This iteration allows Cursor metadata to mirror Claude's installable fields.
          bin: DEFAULT_META.bin,
          openspecVersion: DEFAULT_META.openspecVersion,
          author: DEFAULT_META.author,
        }),
  };

  const merged = {
    ...base,
    ...(meta && typeof meta === 'object' ? meta : {}),
    name: PLUGIN_NAME,
    version,
  };

  writeFileSync(path.join(manifestDir, 'plugin.json'), `${JSON.stringify(merged, null, 2)}\n`);
}

/**
 * Fail if the source tree still contains platform manifest directories.
 * @param {string} pluginRoot
 */
export function ensureSourceCleanOfPlatformManifests(pluginRoot) {
  requireNonEmptyString(pluginRoot, 'pluginRoot');
  if (!existsSync(pluginRoot)) {
    throw new Error(`Plugin root does not exist: ${pluginRoot}`);
  }
  for (const dir of ['.claude-plugin', '.cursor-plugin']) {
    const target = path.join(pluginRoot, dir);
    if (existsSync(target)) {
      throw new Error(`Source tree must not contain ${dir}: ${target}`);
    }
  }
}

/**
 * Orchestrate pack + dual product assembly for the repository.
 * @param {{ repoRoot?: string }} [options]
 * @returns {Promise<void>}
 */
export async function buildPlugins(options = {}) {
  if (options != null && Object.hasOwn(options, 'repoRoot') && options.repoRoot == null) {
    throw new Error('options.repoRoot must not be null or undefined when provided');
  }
  const repoRoot =
    options.repoRoot == null
      ? path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
      : requireNonEmptyString(options.repoRoot, 'options.repoRoot');

  const pluginRoot = path.join(repoRoot, 'plugins', PLUGIN_NAME);
  const packageJsonPath = path.join(pluginRoot, 'package.json');
  const version = readPackageVersion(packageJsonPath);

  if (version === '1.0.0') {
    throw new Error(
      'Refusing to build with version 1.0.0; set plugins/dev-team/package.json version to the released semver (e.g. 2.10.3)',
    );
  }

  await runVpPack(pluginRoot);

  const claudeRoot = path.join(repoRoot, 'claude-plugins', PLUGIN_NAME);
  const cursorRoot = path.join(repoRoot, 'cursor-plugins', PLUGIN_NAME);

  await assembleProductTree(pluginRoot, claudeRoot);
  await assembleProductTree(pluginRoot, cursorRoot);
  await assembleCursorExtras(cursorRoot);

  await writePlatformManifest(claudeRoot, 'claude', version);
  await writePlatformManifest(cursorRoot, 'cursor', version);

  ensureSourceCleanOfPlatformManifests(pluginRoot);
}

const isMain =
  process.argv[1] &&
  path.resolve(fileURLToPath(import.meta.url)) === path.resolve(process.argv[1]);

if (isMain) {
  buildPlugins()
    .then(() => {
      console.log('Built claude-plugins/dev-team and cursor-plugins/dev-team');
    })
    .catch((err) => {
      console.error(err instanceof Error ? err.message : err);
      process.exitCode = 1;
    });
}
