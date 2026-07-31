import * as fs from 'fs';
import * as path from 'path';

import { readConfig } from '../lib/config';
import { matchGlob, toForwardSlash } from '../lib/glob';
import { getProjectDir } from '../lib/project-root';
import { isFileExcluded, isExcludedBySuite } from '../lib/test-exclude';
import {
  type FrameworkConfig,
  getFrameworkConfig,
  detectFrameworkVersion,
} from '../lib/test-framework';
import {
  type TestDetectFrameworksResult,
  type TestPlan,
  type OpenSpecConfig,
  type TestFramework,
  type TestSuite,
} from '../schemas';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface DetectedFile {
  file: string;
  framework: TestFramework | 'unknown';
}

export interface TestDetectFrameworksOptions {
  files?: string[];
  projectRoot?: string;
}

interface ResolvedSuite {
  suite: TestSuite;
  absRoot: string;
  absCwd: string;
  absConfig: string | null;
  /** absCwd relative to projectRoot (POSIX) */
  directory: string;
  frameworkConfig: FrameworkConfig;
  /** Include globs as projectRoot-relative patterns */
  includeGlobs: string[];
}

// ---------------------------------------------------------------------------
// Auto-scan: recursively find files matching any configured glob pattern
// ---------------------------------------------------------------------------

/**
 * Recursively collect all file paths under a root directory, excluding
 * node_modules, .git, and other common non-source directories.
 */
function collectFiles(rootDir: string): string[] {
  const results: string[] = [];
  const excludedDirs = new Set([
    'node_modules',
    '.git',
    '.claude',
    'dist',
    'build',
    'target',
    '.vp',
    'coverage',
    '.nyc_output',
  ]);

  function walk(dir: string): void {
    let entries: fs.Dirent[];
    try {
      entries = fs.readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }

    for (const entry of entries) {
      const fullPath = path.join(dir, entry.name);

      if (entry.isDirectory()) {
        if (!excludedDirs.has(entry.name)) {
          walk(fullPath);
        }
      } else if (entry.isFile()) {
        results.push(fullPath);
      }
    }
  }

  walk(rootDir);
  return results;
}

// ---------------------------------------------------------------------------
// Suite path helpers
// ---------------------------------------------------------------------------

function toPosixRelative(from: string, to: string): string {
  const rel = path.relative(from, to);
  const posix = toForwardSlash(rel);
  return posix === '' ? '.' : posix;
}

function joinRootScoped(root: string, pattern: string): string {
  return path.posix.normalize(path.posix.join(toForwardSlash(root), toForwardSlash(pattern)));
}

/**
 * Expand `{config_args}` in a test_execution template.
 * Replaces every occurrence; never appends to the end of the whole string.
 */
function expandConfigArgs(template: string, configArgs: string): string {
  // Collapse leftover whitespace when configArgs is empty
  if (!configArgs) {
    return template.replace(/\s*\{config_args\}/g, '').replace(/\{config_args\}/g, '');
  }
  return template.replace(/\{config_args\}/g, configArgs);
}

function resolveConfigArgs(
  suite: TestSuite,
  frameworkConfig: FrameworkConfig,
  absCwd: string,
  absConfig: string | null,
): string {
  if (!suite.config) {
    return '';
  }
  if (!frameworkConfig.config_flag) {
    throw new Error(
      `Suite root "${suite.root}" declares config "${suite.config}" but framework ` +
        `"${suite.framework}" does not support config injection (config_flag is null)`,
    );
  }
  if (!absConfig) {
    throw new Error(
      `Suite root "${suite.root}" declares config "${suite.config}" but absConfig could not be resolved`,
    );
  }
  const relConfig = toPosixRelative(absCwd, absConfig);
  return `${frameworkConfig.config_flag} ${relConfig}`;
}

function resolveSuite(suite: TestSuite, projectRoot: string): ResolvedSuite {
  const frameworkConfig = getFrameworkConfig(suite.framework);
  const absRoot = path.resolve(projectRoot, suite.root);
  const absCwd = path.resolve(absRoot, suite.cwd ?? '.');
  const absConfig = suite.config ? path.resolve(absRoot, suite.config) : null;
  const directory = toPosixRelative(projectRoot, absCwd);

  const includePatterns = suite.includes?.length ? suite.includes : [frameworkConfig.default_glob];
  const includeGlobs = includePatterns.map((pattern) => joinRootScoped(suite.root, pattern));

  return {
    suite,
    absRoot,
    absCwd,
    absConfig,
    directory,
    frameworkConfig,
    includeGlobs,
  };
}

/**
 * Whether a project-relative file path is in a suite's scope:
 * under(root) ∧ match(includesEffective) ∧ ¬excludes (this suite only).
 */
function isInSuiteScope(relativePath: string, resolved: ResolvedSuite): boolean {
  const posix = toForwardSlash(relativePath);
  const root = path.posix.normalize(toForwardSlash(resolved.suite.root)).replace(/\/$/, '');

  if (posix !== root && !posix.startsWith(root + '/')) {
    return false;
  }

  if (isExcludedBySuite(posix, resolved.suite)) {
    return false;
  }

  return resolved.includeGlobs.some((glob) => matchGlob(posix, glob));
}

// ---------------------------------------------------------------------------
// generateShellScript / generateCmdScript
// ---------------------------------------------------------------------------

/**
 * Generate a POSIX shell (bash) execution script from plan entry fields.
 * Uses Unix shell syntax: `cd`, `rm -rf`, `\n` line separation, `;` chaining
 * and `_X=$?` exit code capture (embedded in the test_execution template).
 */
function generateShellScript(
  directory: string,
  frameworkConfig: FrameworkConfig,
  configArgs: string,
  version: string,
): string {
  if (frameworkConfig === null || frameworkConfig === undefined) {
    throw new TypeError('generateShellScript input must not be null or undefined');
  }

  const test_execution = expandConfigArgs(
    frameworkConfig.shell.test_execution(version),
    configArgs,
  );
  const { coverage_cleanup } = frameworkConfig.shell;

  if (typeof directory !== 'string') {
    throw new TypeError('generateShellScript: directory must be a string');
  }
  if (typeof test_execution !== 'string') {
    throw new TypeError('generateShellScript: test_execution must return a string');
  }
  if (!Array.isArray(coverage_cleanup)) {
    throw new TypeError('generateShellScript: coverage_cleanup must be an array');
  }

  const lines: string[] = [];

  if (directory !== '.') {
    lines.push(`cd ${directory}`);
  }

  for (const item of coverage_cleanup) {
    lines.push(`rm -rf ${item}`);
  }

  lines.push(test_execution);

  return lines.join('\n') + '\n';
}

/**
 * Generate a Windows cmd.exe execution script from plan entry fields.
 *
 * Constraints when run via Node `execSync(..., { shell: cmd.exe })`:
 * 1. Join steps with ` & ` (single line) — multiline `/c` strings only run the first line.
 * 2. Wrap each `if exist ...` in an outer `(...)` — without it, a trailing `&` after a
 *    parenthesized IF with no ELSE is skipped when the condition is false.
 */
function generateCmdScript(
  directory: string,
  frameworkConfig: FrameworkConfig,
  configArgs: string,
  version: string,
): string {
  if (frameworkConfig === null || frameworkConfig === undefined) {
    throw new TypeError('generateCmdScript input must not be null or undefined');
  }

  const test_execution = expandConfigArgs(frameworkConfig.cmd.test_execution(version), configArgs);
  const { coverage_cleanup } = frameworkConfig.cmd;

  if (typeof directory !== 'string') {
    throw new TypeError('generateCmdScript: directory must be a string');
  }
  if (typeof test_execution !== 'string') {
    throw new TypeError('generateCmdScript: test_execution must return a string');
  }
  if (!Array.isArray(coverage_cleanup)) {
    throw new TypeError('generateCmdScript: coverage_cleanup must be an array');
  }

  const lines: string[] = [];

  if (directory !== '.') {
    const quotedDir =
      directory.includes(' ') || directory.includes('\t')
        ? `"${directory.replace(/"/g, '\\"')}"`
        : directory;
    lines.push(`cd /d ${quotedDir}`);
  }

  for (const item of coverage_cleanup) {
    // Outer parens required so subsequent ` & ` steps still run when the path is absent.
    lines.push(`(if exist "${item}" (rmdir /s /q "${item}" 2>nul & del /f /q "${item}" 2>nul))`);
  }

  lines.push(test_execution);

  return lines.join(' & ');
}

// ---------------------------------------------------------------------------
// Plan and detection helpers
// ---------------------------------------------------------------------------

function buildPlanFromSuites(suites: TestSuite[], projectRoot: string): TestPlan[] {
  const plan: TestPlan[] = [];
  const seen = new Set<string>();

  for (const suite of suites) {
    const resolved = resolveSuite(suite, projectRoot);
    const dedupeKey = `${resolved.directory}::${suite.framework}`;
    if (seen.has(dedupeKey)) {
      continue;
    }
    seen.add(dedupeKey);

    const configArgs = resolveConfigArgs(
      suite,
      resolved.frameworkConfig,
      resolved.absCwd,
      resolved.absConfig,
    );
    const { frameworkConfig } = resolved;
    // Version is used only while building scripts; not exposed on the plan schema.
    const version = detectFrameworkVersion(suite.framework, resolved.absCwd);
    // Path filter relative to absCwd so empty {files} still stays inside absRoot.
    const scope = toPosixRelative(resolved.absCwd, resolved.absRoot);

    plan.push({
      directory: resolved.directory,
      scope,
      framework: frameworkConfig.framework,
      coverage_format: frameworkConfig.coverage_format,
      coverage_output: frameworkConfig.coverage_output,
      mutation_framework: frameworkConfig.mutation_framework,
      mutation_config: null,
      mutation_score: suite.mutation?.score ?? null,
      script: {
        shell: generateShellScript(resolved.directory, frameworkConfig, configArgs, version),
        cmd: generateCmdScript(resolved.directory, frameworkConfig, configArgs, version),
      },
    });
  }

  return plan;
}

function detectFrameworksForFiles(
  filesToCheck: string[],
  projectRoot: string,
  resolvedSuites: ResolvedSuite[],
  isAutoScan: boolean,
  config: OpenSpecConfig,
): { detected: DetectedFile[] } {
  const detected: DetectedFile[] = [];

  for (const file of filesToCheck) {
    const relativePath = toForwardSlash(
      path.isAbsolute(file) ? path.relative(projectRoot, file) : file,
    );

    // Skip excluded files — they don't participate in framework detection
    if (isFileExcluded(relativePath, config) || isFileExcluded(file, config)) {
      continue;
    }

    let matched = false;

    // Array order priority: first matching suite wins
    for (const resolved of resolvedSuites) {
      if (isInSuiteScope(relativePath, resolved)) {
        detected.push({ file, framework: resolved.suite.framework });
        matched = true;
        break;
      }
    }

    if (!matched && !isAutoScan) {
      detected.push({ file, framework: 'unknown' });
    }
  }

  return { detected };
}

function resolveFilesToCheck(
  options: TestDetectFrameworksOptions,
  projectRoot: string,
): string[] | 'empty' {
  if (options.files && options.files.length > 0) {
    return options.files.map((f) => (path.isAbsolute(f) ? f : path.resolve(projectRoot, f)));
  }
  if (options.files !== undefined && options.files.length === 0) {
    return 'empty';
  }
  return collectFiles(projectRoot);
}

function buildNoSuitesResult(
  filesToCheck: string[],
  isAutoScan: boolean,
): TestDetectFrameworksResult {
  const detected: DetectedFile[] = isAutoScan
    ? []
    : filesToCheck.map((file) => ({
        file,
        framework: 'unknown',
      }));
  return { detected, plan: [] };
}

// ---------------------------------------------------------------------------
// Main handler
// ---------------------------------------------------------------------------

/**
 * Run `test_detect_frameworks`: detect which test framework(s) each file
 * belongs to, based on the `tests[]` suite mappings in config.json.
 *
 * When `files` is provided, match only those files.  When omitted,
 * auto-scan the project for files matching any of the configured suite scopes.
 */
export function runTestDetectFrameworks(
  options: TestDetectFrameworksOptions,
): TestDetectFrameworksResult {
  const projectRoot = options.projectRoot || getProjectDir();

  const config = readConfig(projectRoot);
  const suites = config.tests ?? [];
  const plan = buildPlanFromSuites(suites, projectRoot);
  const resolvedSuites = suites.map((suite) => resolveSuite(suite, projectRoot));

  const filesResult = resolveFilesToCheck(options, projectRoot);
  if (filesResult === 'empty') {
    return { detected: [], plan: [] };
  }
  const filesToCheck = filesResult;
  const isAutoScan = options.files === undefined;

  if (suites.length === 0) {
    return buildNoSuitesResult(filesToCheck, isAutoScan);
  }

  const { detected } = detectFrameworksForFiles(
    filesToCheck,
    projectRoot,
    resolvedSuites,
    isAutoScan,
    config,
  );

  return { detected, plan };
}
