/**
 * Glob matching utilities — stub pending use-fast-glob implementation.
 *
 * TODO(use-fast-glob): Replace with fast-glob/picomatch per design.md
 *
 * @see openspec/changes/use-fast-glob/design.md
 */

export interface ScanProjectFilesOptions {
  ignore?: string[];
}

export function matchGlob(_filePath: string, _pattern: string): boolean {
  throw new Error('matchGlob not implemented (use-fast-glob)');
}

export function scanProjectFiles(
  _rootDir: string,
  _patterns: string[],
  _options?: ScanProjectFilesOptions,
): string[] {
  throw new Error('scanProjectFiles not implemented (use-fast-glob)');
}
