import * as path from 'path';

import { getProjectDir } from '../utils';

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
  modules: string[];
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
  modules: string[];
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

/**
 * Resolve unit and integration test paths from a module list.
 * Errors are collected per module; processing continues for remaining entries.
 */
function resolveTestPaths(params: ResolveTestPathsParams): ResolveTestPathsResult {
  const projectRoot = path.resolve(params.projectRoot);
  const unitTestMap = new Map<string, UnitTestEntry>();
  const errors: ResolveError[] = [];
  const collectedSources: string[] = [];

  for (const moduleEntry of params.modules) {
    const posix = moduleEntry.replace(/\\/g, '/');

    if (!isWithinProjectRoot(projectRoot, moduleEntry)) {
      errors.push({ path: posix, message: 'Path is outside project root' });
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
