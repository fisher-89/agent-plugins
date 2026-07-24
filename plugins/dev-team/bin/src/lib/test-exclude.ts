// ---------------------------------------------------------------------------
// Test Exclude Filter
//
// Shared utility for determining whether a source file should be excluded
// from the test pipeline based on `tests[].excludes` (relative to each
// suite's `root`) in the project configuration.
//
// Consumers: test-detect-frameworks, test-resolve-paths, test-runner.
// ---------------------------------------------------------------------------

import path from 'node:path';

import type { OpenSpecConfig, TestSuite } from '../schemas';
import { matchGlob, toForwardSlash } from './glob';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Join a suite root with a relative glob and normalize `.` / `..` segments.
 * Result is a projectRoot-relative POSIX pattern.
 */
function joinRootScoped(root: string, pattern: string): string {
  const normalizedRoot = path.posix.normalize(toForwardSlash(root));
  const normalizedPattern = toForwardSlash(pattern);
  return path.posix.normalize(path.posix.join(normalizedRoot, normalizedPattern));
}

function isAbsolutePosix(posixPath: string): boolean {
  return posixPath.startsWith('/') || /^[A-Za-z]:\//.test(posixPath);
}

/**
 * Return the path of `filePath` relative to `root` when the file is under that
 * root tree; otherwise null. Accepts project-relative or absolute paths.
 *
 * Uses a true path-prefix (segment boundary): `root` must be an exact prefix of
 * the project-relative path (`src` does not match `apps/src/foo`). Absolute
 * paths may include a project-root prefix before `suite.root`.
 */
function relativeToRoot(filePath: string, root: string): string | null {
  const posixPath = toForwardSlash(filePath);
  const normRoot = path.posix.normalize(toForwardSlash(root)).replace(/\/$/, '');

  if (posixPath === normRoot) {
    return '.';
  }
  if (posixPath.startsWith(normRoot + '/')) {
    return posixPath.slice(normRoot.length + 1);
  }

  // Absolute paths may be prefixed by the project root directory.
  // Do not apply substring search to relative paths (avoids `apps/src/foo`
  // falsely matching root `src`).
  if (isAbsolutePosix(posixPath)) {
    const marker = '/' + normRoot + '/';
    const idx = posixPath.indexOf(marker);
    if (idx !== -1) {
      return posixPath.slice(idx + marker.length);
    }
    if (posixPath.endsWith('/' + normRoot)) {
      return '.';
    }
  }

  return null;
}

function suiteExcludeGlobs(suite: TestSuite): string[] {
  if (!suite.excludes?.length) {
    return [];
  }
  return suite.excludes.map((pattern) => joinRootScoped(suite.root, pattern));
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Whether a file is excluded by a single suite's `excludes` (relative to that
 * suite's `root`). Other suites' excludes are ignored.
 *
 * @param filePath - Absolute or relative file path (POSIX or Windows).
 * @param suite    - One entry from `config.tests`.
 */
export function isExcludedBySuite(filePath: string, suite: TestSuite): boolean {
  if (!suite.excludes?.length) {
    return false;
  }

  const posixPath = toForwardSlash(filePath);
  const relToRoot = relativeToRoot(posixPath, suite.root);
  if (relToRoot === null) {
    return false;
  }

  for (const exclude of suite.excludes) {
    const scoped = joinRootScoped(suite.root, exclude);
    const projectRel =
      relToRoot === '.'
        ? path.posix.normalize(toForwardSlash(suite.root))
        : path.posix.normalize(path.posix.join(toForwardSlash(suite.root), relToRoot));

    if (matchGlob(projectRel, scoped) || matchGlob(relToRoot, exclude)) {
      return true;
    }
  }

  return false;
}

/**
 * Determine whether a file path is excluded by any suite's `excludes`.
 *
 * A suite's excludes apply only when the file falls under that suite's `root`.
 * Exclude globs are interpreted relative to the suite root and matched against
 * the project-relative (or equivalent) path.
 *
 * @param filePath - Absolute or relative file path (POSIX or Windows).
 * @param config   - Parsed `OpenSpecConfig` from openspec/config.json.
 * @returns `true` if the file matches any applicable suite-scoped exclude glob.
 */
export function isFileExcluded(filePath: string, config: OpenSpecConfig): boolean {
  for (const suite of config.tests ?? []) {
    if (isExcludedBySuite(filePath, suite)) {
      return true;
    }
  }
  return false;
}

/**
 * Collect all suite-scoped exclude glob strings derived from `tests[].excludes`,
 * joined to each suite `root` and deduplicated.
 *
 * Use this for diagnostic/display purposes (e.g. listing which glob
 * patterns are active in the project).
 *
 * @public
 * @param config - Parsed `OpenSpecConfig` from openspec/config.json.
 * @returns A flat, deduplicated array of project-relative exclude glob patterns.
 */
export function getExcludeGlobs(config: OpenSpecConfig): string[] {
  const seen = new Set<string>();

  for (const suite of config.tests ?? []) {
    for (const glob of suiteExcludeGlobs(suite)) {
      seen.add(glob);
    }
  }

  return [...seen];
}
