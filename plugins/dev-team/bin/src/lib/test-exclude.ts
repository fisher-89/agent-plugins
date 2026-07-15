// ---------------------------------------------------------------------------
// Test Exclude Filter
//
// Shared utility for determining whether a source file should be excluded
// from the test pipeline based on `test.exclude` and `test.overrides[].exclude`
// in the project configuration.
//
// Consumers: test-detect-frameworks, test-resolve-paths, test-runner.
// ---------------------------------------------------------------------------

import path from 'node:path';

import type { OpenSpecConfig } from '../schemas';
import { matchGlob } from './glob';

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Determine whether a file path is excluded by `test.exclude` or
 * `test.overrides[].exclude`.
 *
 * Global exclude globs are matched directly against the file path.
 * Override-level exclude globs only apply when the file also matches the
 * override's `file` glob.
 *
 * @param filePath - Absolute or relative file path (POSIX or Windows).
 * @param config   - Parsed `OpenSpecConfig` from openspec/config.json.
 * @returns `true` if the file matches any applicable exclude glob.
 */
export function isFileExcluded(filePath: string, config: OpenSpecConfig): boolean {
  const posixPath = filePath.replace(/\\/g, '/');
  const globs = getExcludeGlobs(config);
  return globs.some((glob) => matchGlob(posixPath, glob));
}

/**
 * Collect all exclude glob strings from both `test.exclude` and
 * `test.overrides[].exclude`, deduplicated.
 *
 * Use this for diagnostic/display purposes (e.g. listing which glob
 * patterns are active in the project).
 *
 * @public
 * @param config - Parsed `OpenSpecConfig` from openspec/config.json.
 * @returns A flat, deduplicated array of exclude glob patterns.
 */
export function getExcludeGlobs(config: OpenSpecConfig): string[] {
  const seen = new Set<string>();

  // 1. Global exclude
  const globalExcludes =
    config.test?.exclude?.map((pattern) =>
      needWildcardPrefix(pattern) ? path.posix.join('**', pattern) : pattern,
    ) ?? [];
  for (const glob of globalExcludes) {
    seen.add(glob);
  }

  // 2. Override-level exclude
  const overrides = config.test?.overrides ?? [];
  for (const override of overrides) {
    const overridePrefix = needWildcardPrefix(override.file)
      ? path.posix.join('**', override.file)
      : override.file;
    const overrideExcludes =
      override.exclude?.map((pattern) => path.posix.join(overridePrefix, pattern)) ?? [];
    for (const glob of overrideExcludes) {
      seen.add(glob);
    }
  }

  return [...seen];
}

function needWildcardPrefix(globPattern: string): boolean {
  if (path.isAbsolute(globPattern)) {
    return false;
  }
  return !globPattern.startsWith('*');
}
