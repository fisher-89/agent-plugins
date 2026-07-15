// ---------------------------------------------------------------------------
// Test Exclude Filter
//
// Shared utility for determining whether a source file should be excluded
// from the test pipeline based on `test.exclude` and `test.overrides[].exclude`
// in the project configuration.
//
// Consumers: test-detect-frameworks, test-resolve-paths, test-runner.
// ---------------------------------------------------------------------------

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

  // 1. Global exclude
  const globalExcludes = config.test?.exclude ?? [];
  for (const glob of globalExcludes) {
    if (matchGlob(posixPath, glob)) {
      return true;
    }
  }

  // 2. Override-level exclude (only if file matches the override's file glob)
  const overrides = config.test?.overrides ?? [];
  for (const override of overrides) {
    const overrideExcludes = override.exclude ?? [];
    if (overrideExcludes.length === 0) continue;

    if (!matchGlob(posixPath, override.file)) continue;

    for (const glob of overrideExcludes) {
      if (matchGlob(posixPath, glob)) {
        return true;
      }
    }
  }

  return false;
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
  const globalExcludes = config.test?.exclude ?? [];
  for (const glob of globalExcludes) {
    seen.add(glob);
  }

  // 2. Override-level exclude
  const overrides = config.test?.overrides ?? [];
  for (const override of overrides) {
    const overrideExcludes = override.exclude ?? [];
    for (const glob of overrideExcludes) {
      seen.add(glob);
    }
  }

  return [...seen];
}
