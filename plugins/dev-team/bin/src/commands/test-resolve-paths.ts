import { execSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';

import { type z } from 'zod/v4';

import { readConfig } from '../lib/config';
import { matchGlob, toForwardSlash } from '../lib/glob';
import { isFileExcluded } from '../lib/test-exclude';
import { getFrameworkConfig } from '../lib/test-framework';
import type { OpenSpecConfig, TestSuite, unitTestEntrySchema } from '../schemas';
import { getProjectDir } from '../utils';
import { runTestDetectFrameworks } from './test-detect-frameworks';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type UnitTestEntry = z.output<typeof unitTestEntrySchema>;

interface ResolveError {
  path: string;
  message: string;
}

interface ResolveTestPathsParams {
  projectRoot: string;
  modules: string[] | 'git-change';
}

export interface ResolveTestPathsResult {
  unit_tests: UnitTestEntry[];
  errors: ResolveError[];
}

export interface TestResolvePathsInput {
  modules: string[] | 'git-change';
  project_root?: string | null;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const SOURCE_EXTENSIONS = new Set([
  '.ts',
  '.tsx',
  '.js',
  '.jsx',
  '.mjs',
  '.cjs',
  '.py',
  '.go',
  '.rs',
]);

const JS_TS_EXTENSIONS = new Set(['.ts', '.tsx', '.js', '.jsx', '.mjs', '.cjs']);

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

/**
 * Type predicate: narrow an unknown value to an object with a `stderr` property.
 * Safe alternative to `as` type assertions when handling ExecSyncError.
 */
function isObjectWithStderr(e: unknown): e is { stderr: unknown; message?: unknown } {
  return e !== null && typeof e === 'object' && 'stderr' in e;
}

/**
 * Extract a human-readable message from a caught error, with specific handling
 * for Node.js ExecSyncError (which carries a `stderr` property on the thrown object).
 */
function extractErrorMessage(e: unknown, fallback: string): string {
  if (e instanceof Error) {
    return e.message;
  }
  if (isObjectWithStderr(e)) {
    const { stderr } = e;
    if (typeof stderr === 'string') {
      return stderr.trim() || fallback;
    }
    try {
      return String(stderr).trim() || fallback;
    } catch {
      // fall through
    }
  }
  return fallback;
}

/** Return true when resolved inputPath stays within projectRoot (path-traversal guard). */
function isWithinProjectRoot(projectRoot: string, inputPath: string): boolean {
  const resolvedRoot = path.resolve(projectRoot);
  const resolvedPath = path.isAbsolute(inputPath)
    ? path.resolve(inputPath)
    : path.resolve(resolvedRoot, inputPath);
  const relative = path.relative(resolvedRoot, resolvedPath);
  return !relative.startsWith('..') && !path.isAbsolute(relative);
}

/** Detect existing test files by naming convention. */
function isTestFile(filePath: string): boolean {
  const base = path.posix.basename(filePath.replace(/\\/g, '/'));
  if (/\.test\./.test(base)) return true;
  if (/^test_.*\.py$/.test(base)) return true;
  if (/.*_test\.go$/.test(base)) return true;
  if (/.*_tests?\.rs$/.test(base)) return true;
  return false;
}

/** Return true when the file extension is in the testable source set (excludes test files). */
function isSourceFile(filePath: string): boolean {
  if (isTestFile(filePath)) return false;
  const ext = path.posix.extname(filePath.replace(/\\/g, '/')).toLowerCase();
  return SOURCE_EXTENSIONS.has(ext);
}

/**
 * Derive the colocated unit test path for a source file.
 * Rules align with test-gen-generator colocated naming table.
 */
function deriveUnitTestPath(sourcePath: string): string {
  const posix = sourcePath.replace(/\\/g, '/');
  const dir = path.posix.dirname(posix);
  const base = path.posix.basename(posix);
  const ext = path.posix.extname(base);
  const basename = base.slice(0, base.length - ext.length);

  let testName: string;
  if (JS_TS_EXTENSIONS.has(ext)) {
    testName = `${basename}.test${ext}`;
  } else if (ext === '.py') {
    testName = `test_${basename}.py`;
  } else if (ext === '.go') {
    testName = `${basename}_test.go`;
  } else if (ext === '.rs') {
    testName = `${basename}_test.rs`;
  } else {
    testName = base;
  }

  return dir === '.' ? testName : `${dir}/${testName}`;
}

// ---------------------------------------------------------------------------
// collectFiles — recursive file discovery (local implementation, per D5)
// ---------------------------------------------------------------------------

/** Recursively collect all file paths under a root directory, excluding
 * common non-source directories (node_modules, .git, .claude, dist, build,
 * target, .vp, coverage, .nyc_output). */
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
// Core resolver
// ---------------------------------------------------------------------------

function addUnitTest(unitTests: Map<string, UnitTestEntry>, sourcePath: string): void {
  const posix = sourcePath.replace(/\\/g, '/');
  if (!unitTests.has(posix)) {
    unitTests.set(posix, {
      source: posix,
      test_file: deriveUnitTestPath(posix),
    });
  }
}

// ---------------------------------------------------------------------------
// resolveTestPaths — Step helpers
// ---------------------------------------------------------------------------

/**
 * Step 1 of resolveTestPaths: determine effective modules list.
 *
 * Returns either the resolved module list or an early-return result (when
 * git-change mode produces no changes or encounters an error).
 */
function resolveEffectiveModules(
  params: ResolveTestPathsParams,
  projectRoot: string,
  errors: ResolveError[],
): { modules: string[] } | { earlyReturn: ResolveTestPathsResult } {
  let effectiveModules: string[];

  if (params.modules === 'git-change') {
    try {
      const stdout = execSync('git diff HEAD --name-only', {
        cwd: projectRoot,
        encoding: 'utf-8',
        timeout: 10000,
        stdio: 'pipe',
      });
      effectiveModules = stdout
        .split('\n')
        .map((s) => s.trim())
        .filter(Boolean);
      if (effectiveModules.length === 0) {
        return {
          earlyReturn: { unit_tests: [], errors },
        };
      }
    } catch (e: unknown) {
      const msg = extractErrorMessage(e, 'git diff HEAD --name-only failed');
      errors.push({ path: 'git', message: msg });
      return {
        earlyReturn: { unit_tests: [], errors },
      };
    }
  } else {
    effectiveModules = params.modules;
  }

  return { modules: effectiveModules };
}

/**
 * Step 2a of resolveTestPaths: process non-empty modules through test config.
 * Each module is validated (within project root, in test scope, not a test
 * file, is a source file) before adding a unit-test entry.
 */
function processNonEmptyModules(
  effectiveModules: string[],
  projectRoot: string,
  unitTestMap: Map<string, UnitTestEntry>,
  errors: ResolveError[],
  collectedSources: string[],
  config: OpenSpecConfig,
): void {
  const detectedResult = runTestDetectFrameworks({
    files: effectiveModules,
    projectRoot,
  });

  const detectedSet = new Set(detectedResult.detected.map((d) => d.file));

  for (const moduleEntry of effectiveModules) {
    processModuleEntry(
      moduleEntry,
      projectRoot,
      unitTestMap,
      errors,
      collectedSources,
      config,
      detectedSet,
    );
  }
}

/**
 * Validate a single module entry and add its unit-test entry if valid.
 */
function processModuleEntry(
  moduleEntry: string,
  projectRoot: string,
  unitTestMap: Map<string, UnitTestEntry>,
  errors: ResolveError[],
  collectedSources: string[],
  config: OpenSpecConfig,
  detectedSet: Set<string>,
): void {
  const posix = moduleEntry.replace(/\\/g, '/');

  if (!isWithinProjectRoot(projectRoot, moduleEntry)) {
    errors.push({ path: posix, message: 'Path is outside project root' });
    return;
  }

  const absPath = path.isAbsolute(moduleEntry)
    ? path.resolve(moduleEntry)
    : path.resolve(projectRoot, moduleEntry);

  if (!detectedSet.has(absPath)) {
    errors.push({ path: posix, message: 'Not in test config scope' });
    return;
  }

  // Silently skip excluded files (no error added)
  if (isFileExcluded(absPath, config)) {
    return;
  }

  if (isTestFile(posix)) {
    errors.push({ path: posix, message: 'Path is already a test file' });
    return;
  }

  if (!isSourceFile(posix)) {
    errors.push({ path: posix, message: 'Not a testable source file' });
    return;
  }

  collectedSources.push(posix);
  addUnitTest(unitTestMap, posix);
}

/**
 * Whether a project-relative source file falls in a suite's coverage scope
 * for empty-module scanning: under(root) ∧ match(includesEffective) ∧ ¬excludes,
 * where includesEffective = suite.includes ?? framework.default_glob.
 */
function isInSuiteSourceScope(
  relativePath: string,
  suite: TestSuite,
  config: OpenSpecConfig,
): boolean {
  const posix = toForwardSlash(relativePath);
  const root = path.posix.normalize(toForwardSlash(suite.root)).replace(/\/$/, '');

  if (posix !== root && !posix.startsWith(root + '/')) {
    return false;
  }

  if (isFileExcluded(posix, config)) {
    return false;
  }

  const includePatterns = suite.includes?.length
    ? suite.includes
    : [getFrameworkConfig(suite.framework).default_glob];

  return includePatterns.some((pattern) => {
    const scoped = path.posix.normalize(
      path.posix.join(toForwardSlash(suite.root), toForwardSlash(pattern)),
    );
    return matchGlob(posix, scoped);
  });
}

/**
 * Step 2b of resolveTestPaths: config-driven directory scan when no modules
 * are specified. Scans each suite `root` (not plan.directory / absCwd) so
 * `cwd: ".."` does not widen or shrink the coverage scope incorrectly.
 */
function processEmptyModules(
  projectRoot: string,
  unitTestMap: Map<string, UnitTestEntry>,
  errors: ResolveError[],
  collectedSources: string[],
  config: OpenSpecConfig,
): void {
  const detectedResult = runTestDetectFrameworks({ projectRoot });
  const plan = detectedResult.plan;
  const suites = config.tests ?? [];

  if (plan.length === 0 || suites.length === 0) {
    errors.push({
      path: 'config',
      message: "No test configuration found. Please configure 'tests' in openspec/config.json",
    });
    return;
  }

  const sourceFiles = new Set<string>();

  for (const suite of suites) {
    const absRoot = path.resolve(projectRoot, suite.root);
    const allFiles = collectFiles(absRoot);
    for (const file of allFiles) {
      const relPath = path.relative(projectRoot, file);
      const posix = toForwardSlash(relPath);
      if (!isSourceFile(posix)) {
        continue;
      }
      if (!isInSuiteSourceScope(posix, suite, config)) {
        continue;
      }
      sourceFiles.add(posix);
    }
  }

  for (const sourceFile of sourceFiles) {
    addUnitTest(unitTestMap, sourceFile);
    collectedSources.push(sourceFile);
  }
}

/**
 * Resolve unit test paths from a module list.
 *
 * Three modes:
 * 1. modules === "git-change" → run git diff HEAD --name-only to discover files
 * 2. modules is non-empty array → filter through test config via runTestDetectFrameworks
 * 3. modules is empty array → config-driven directory scan via runTestDetectFrameworks plan
 *
 * Errors are collected per module; processing continues for remaining entries.
 */
function resolveTestPaths(params: ResolveTestPathsParams): ResolveTestPathsResult {
  const projectRoot = path.resolve(params.projectRoot);
  const unitTestMap = new Map<string, UnitTestEntry>();
  const errors: ResolveError[] = [];
  const collectedSources: string[] = [];

  // Read config early so both process functions can use exclude filtering
  const config = readConfig(projectRoot);

  // Step 1: Determine effective modules
  const step1 = resolveEffectiveModules(params, projectRoot, errors);
  if ('earlyReturn' in step1) {
    return step1.earlyReturn;
  }

  // Step 2: Process by mode
  if (step1.modules.length > 0) {
    processNonEmptyModules(
      step1.modules,
      projectRoot,
      unitTestMap,
      errors,
      collectedSources,
      config,
    );
  } else {
    processEmptyModules(projectRoot, unitTestMap, errors, collectedSources, config);
  }

  // Step 3: Build and return result
  const unit_tests = Array.from(unitTestMap.values()).sort((a, b) =>
    a.source.localeCompare(b.source),
  );

  errors.sort((a, b) => a.path.localeCompare(b.path));

  return { unit_tests, errors };
}

/**
 * MCP command entry: resolve project_root then delegate to resolveTestPaths.
 */
export function runTestResolvePaths(args: TestResolvePathsInput): ResolveTestPathsResult {
  const projectRoot = args.project_root || getProjectDir();
  return resolveTestPaths({
    projectRoot,
    modules: args.modules,
  });
}
