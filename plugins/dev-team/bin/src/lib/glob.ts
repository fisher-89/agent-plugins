import picomatch from 'picomatch';

const WILDCARD_CHARS = /[*?{[]/;

export function toForwardSlash(p: string): string {
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
