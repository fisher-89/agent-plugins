import { execSync } from 'child_process';
import * as fs from 'fs';
import * as path from 'path';

import { getProjectDir } from '../utils';
import { runTestDetectFrameworks } from './test-detect-frameworks';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface UnitTestEntry {
  source: string;
  test_file: string;
}

interface IntegrationTestEntry {
  scenario: string;
  test_file: string;
}

interface ResolveError {
  path: string;
  message: string;
}

interface ResolveTestPathsParams {
  projectRoot: string;
  modules: string[] | 'git-change';
  integrationScenarios?: string[];
  extension?: string;
  integrationRoot?: string;
}

export interface ResolveTestPathsResult {
  unit_tests: UnitTestEntry[];
  integration_tests: IntegrationTestEntry[];
  errors: ResolveError[];
}

export interface TestResolvePathsInput {
  modules: string[] | 'git-change';
  integration_scenarios?: string[];
  extension?: string;
  integration_root?: string;
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

/** Normalize integration_root: POSIX slashes, strip trailing slash; "." / "" → no prefix. */
function normalizeIntegrationRoot(integrationRoot?: string): string | undefined {
  if (integrationRoot === undefined || integrationRoot === '') {
    return undefined;
  }

  const posix = integrationRoot.replace(/\\/g, '/').replace(/\/+$/, '');
  if (posix === '' || posix === '.') {
    return undefined;
  }

  return posix;
}

/** Return true when integrationRoot has no path-traversal segments. */
function isValidIntegrationRoot(integrationRoot: string): boolean {
  return !integrationRoot.split('/').some((segment) => segment === '..');
}

/** Derive integration test path: __tests__/<scenario>/<scenario>.test.<ext> */
function deriveIntegrationTestPath(
  scenario: string,
  ext: string,
  integrationRoot?: string,
): string {
  const normalized = normalizeExtension(ext);
  const basePath = `__tests__/${scenario}/${scenario}.test.${normalized}`;
  const root = normalizeIntegrationRoot(integrationRoot);
  if (!root) {
    return basePath;
  }
  return `${root}/${basePath}`;
}

/** Strip leading dot and lower-case an extension string. */
function normalizeExtension(ext: string): string {
  const trimmed = ext.startsWith('.') ? ext.slice(1) : ext;
  return trimmed.toLowerCase();
}

/**
 * Resolve integration test extension: explicit > mode of source extensions > "ts".
 */
function inferExtension(sourceFiles: string[], explicitExtension?: string): string {
  if (explicitExtension !== undefined && explicitExtension !== '') {
    return normalizeExtension(explicitExtension);
  }

  if (sourceFiles.length === 0) {
    return 'ts';
  }

  const counts = new Map<string, number>();
  for (const file of sourceFiles) {
    const ext = path.posix.extname(file.replace(/\\/g, '/'));
    if (!ext) continue;
    const normalized = ext.slice(1).toLowerCase();
    counts.set(normalized, (counts.get(normalized) ?? 0) + 1);
  }

  if (counts.size === 0) {
    return 'ts';
  }

  let bestExt = 'ts';
  let bestCount = -1;
  for (const [ext, count] of counts) {
    if (count > bestCount || (count === bestCount && ext < bestExt)) {
      bestCount = count;
      bestExt = ext;
    }
  }
  return bestExt;
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

function resolveIntegrationTests(
  params: ResolveTestPathsParams,
  collectedSources: string[],
  errors: ResolveError[],
): IntegrationTestEntry[] {
  const integration_tests: IntegrationTestEntry[] = [];
  const scenarios = params.integrationScenarios;
  if (scenarios && scenarios.length > 0) {
    const ext = inferExtension(collectedSources, params.extension);
    const normalizedRoot = normalizeIntegrationRoot(params.integrationRoot);
    const invalidRoot = normalizedRoot !== undefined && !isValidIntegrationRoot(normalizedRoot);

    if (invalidRoot) {
      errors.push({
        path: normalizedRoot.replace(/\\/g, '/'),
        message: 'integration_root path is outside project root',
      });
    } else {
      for (const scenario of [...scenarios].sort((a, b) => a.localeCompare(b))) {
        integration_tests.push({
          scenario,
          test_file: deriveIntegrationTestPath(scenario, ext, params.integrationRoot),
        });
      }
    }
  }
  return integration_tests;
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
      });
      effectiveModules = stdout
        .split('\n')
        .map((s) => s.trim())
        .filter(Boolean);
      if (effectiveModules.length === 0) {
        return {
          earlyReturn: { unit_tests: [], integration_tests: [], errors },
        };
      }
    } catch (e: unknown) {
      const msg = extractErrorMessage(e, 'git diff HEAD --name-only failed');
      errors.push({ path: 'git', message: msg });
      return {
        earlyReturn: { unit_tests: [], integration_tests: [], errors },
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
): void {
  const detectedResult = runTestDetectFrameworks({
    files: effectiveModules,
    projectRoot,
  });

  const detectedSet = new Set(detectedResult.detected.map((d) => d.file));

  for (const moduleEntry of effectiveModules) {
    const posix = moduleEntry.replace(/\\/g, '/');

    if (!isWithinProjectRoot(projectRoot, moduleEntry)) {
      errors.push({ path: posix, message: 'Path is outside project root' });
      continue;
    }

    const absPath = path.isAbsolute(moduleEntry)
      ? path.resolve(moduleEntry)
      : path.resolve(projectRoot, moduleEntry);

    if (!detectedSet.has(absPath)) {
      errors.push({ path: posix, message: 'Not in test config scope' });
      continue;
    }

    if (isTestFile(posix)) {
      errors.push({ path: posix, message: 'Path is already a test file' });
      continue;
    }

    if (!isSourceFile(posix)) {
      errors.push({ path: posix, message: 'Not a testable source file' });
      continue;
    }

    collectedSources.push(posix);
    addUnitTest(unitTestMap, posix);
  }
}

/**
 * Step 2b of resolveTestPaths: config-driven directory scan when no modules
 * are specified. Uses runTestDetectFrameworks to discover source directories
 * and scans each for source files.
 */
function processEmptyModules(
  projectRoot: string,
  unitTestMap: Map<string, UnitTestEntry>,
  errors: ResolveError[],
  collectedSources: string[],
): void {
  const detectedResult = runTestDetectFrameworks({ projectRoot });
  const plan = detectedResult.plan;

  if (plan.length === 0) {
    errors.push({
      path: 'config',
      message:
        "No test configuration found. Please configure 'test.framework' or 'test.overrides' in openspec/config.json",
    });
    return;
  }

  const directories = [...new Set(plan.map((p) => p.directory))] as string[];
  const sourceFiles = new Set<string>();

  for (const dir of directories) {
    const absDir = path.resolve(projectRoot, dir);
    const allFiles = collectFiles(absDir);
    for (const file of allFiles) {
      const relPath = path.relative(projectRoot, file);
      const posix = relPath.replace(/\\/g, '/');
      if (isSourceFile(posix)) {
        sourceFiles.add(posix);
      }
    }
  }

  for (const sourceFile of sourceFiles) {
    addUnitTest(unitTestMap, sourceFile);
    collectedSources.push(sourceFile);
  }
}

/**
 * Resolve unit and integration test paths from a module list.
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

  // Step 1: Determine effective modules
  const step1 = resolveEffectiveModules(params, projectRoot, errors);
  if ('earlyReturn' in step1) {
    return step1.earlyReturn;
  }

  // Step 2: Process by mode
  if (step1.modules.length > 0) {
    processNonEmptyModules(step1.modules, projectRoot, unitTestMap, errors, collectedSources);
  } else {
    processEmptyModules(projectRoot, unitTestMap, errors, collectedSources);
  }

  // Step 3: Build and return result
  const unit_tests = Array.from(unitTestMap.values()).sort((a, b) =>
    a.source.localeCompare(b.source),
  );

  const integration_tests = resolveIntegrationTests(params, collectedSources, errors);

  errors.sort((a, b) => a.path.localeCompare(b.path));

  return { unit_tests, integration_tests, errors };
}

/**
 * MCP command entry: resolve project_root then delegate to resolveTestPaths.
 */
export function runTestResolvePaths(args: TestResolvePathsInput): ResolveTestPathsResult {
  const projectRoot = args.project_root || getProjectDir();
  return resolveTestPaths({
    projectRoot,
    modules: args.modules,
    integrationScenarios: args.integration_scenarios,
    extension: args.extension,
    integrationRoot: args.integration_root,
  });
}
