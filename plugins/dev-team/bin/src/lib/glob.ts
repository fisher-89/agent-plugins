import * as path from 'node:path';

import fg from 'fast-glob';
import picomatch from 'picomatch';

export interface ScanProjectFilesOptions {
  ignore?: string[];
}

const WILDCARD_CHARS = /[*?{[]/;

const DEFAULT_IGNORE = [
  '**/node_modules/**',
  '**/.git/**',
  '**/.claude/**',
  '**/dist/**',
  '**/build/**',
  '**/target/**',
  '**/.vp/**',
  '**/coverage/**',
  '**/.nyc_output/**',
];

function toForwardSlash(p: string): string {
  return p.replace(/\\/g, '/');
}

/**
 * Check whether a file path matches a glob pattern.
 *
 * When `pattern` contains no wildcard characters it is treated as a directory
 * prefix: the file matches if it equals the pattern or starts with `pattern/`.
 */
export function matchGlob(filePath: string, pattern: string): boolean {
  const normPath = toForwardSlash(filePath);
  const normPattern = toForwardSlash(pattern);

  if (!WILDCARD_CHARS.test(normPattern)) {
    return normPath === normPattern || normPath.startsWith(normPattern + '/');
  }

  return picomatch.isMatch(normPath, normPattern, { dot: true });
}

/**
 * Scan a project directory for files matching one or more glob patterns.
 *
 * Returns sorted, deduplicated absolute paths using native path separators.
 * Directories in `DEFAULT_IGNORE` are always excluded; additional patterns
 * can be supplied via `options.ignore`.
 *
 * A pattern without wildcard characters is treated as a directory prefix and
 * automatically expanded to `<pattern>/**`.
 */
export function scanProjectFiles(
  rootDir: string,
  patterns: string[],
  options?: ScanProjectFilesOptions,
): string[] {
  if (patterns.length === 0) return [];

  const expanded = patterns.map((p) => {
    const norm = toForwardSlash(p);
    return WILDCARD_CHARS.test(norm) ? norm : norm + '/**';
  });

  const ignore = [...DEFAULT_IGNORE, ...(options?.ignore ?? [])];

  const files = fg.sync(expanded, {
    cwd: rootDir,
    absolute: true,
    onlyFiles: true,
    dot: false,
    ignore,
  });

  return [...new Set(files.map((f) => path.normalize(f)))].sort();
}
