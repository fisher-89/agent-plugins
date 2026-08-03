// ---------------------------------------------------------------------------
// Colocated test ↔ source path naming
//
// Shared naming conventions for colocated unit-test paths.
// Rules align with test-gen-generator colocated naming table.
// ---------------------------------------------------------------------------

import * as path from 'node:path';

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

/** Detect existing test files by naming convention. */
export function isTestFile(filePath: string): boolean {
  const base = path.posix.basename(filePath.replace(/\\/g, '/'));
  if (/\.test\./.test(base)) return true;
  if (/^test_.*\.py$/.test(base)) return true;
  if (/.*_test\.go$/.test(base)) return true;
  if (/.*_tests?\.rs$/.test(base)) return true;
  return false;
}

/** Return true when the file extension is in the testable source set (excludes test files). */
export function isSourceFile(filePath: string): boolean {
  if (isTestFile(filePath)) return false;
  const ext = path.posix.extname(filePath.replace(/\\/g, '/')).toLowerCase();
  return SOURCE_EXTENSIONS.has(ext);
}

/**
 * Derive the colocated unit test path for a source file.
 */
export function deriveUnitTestPath(sourcePath: string): string {
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

/**
 * Reverse of {@link deriveUnitTestPath}: infer the colocated source path from a
 * test file. Returns null when the path is not recognized as a test file.
 */
export function deriveSourcePathFromTestFile(testPath: string): string | null {
  const posix = testPath.replace(/\\/g, '/');
  if (!isTestFile(posix)) return null;

  const dir = path.posix.dirname(posix);
  const base = path.posix.basename(posix);

  let sourceBase: string;
  if (/\.test\./.test(base)) {
    sourceBase = base.replace(/\.test\./, '.');
  } else if (/^test_.*\.py$/.test(base)) {
    sourceBase = base.replace(/^test_/, '');
  } else if (/.*_test\.go$/.test(base)) {
    sourceBase = base.replace(/_test\.go$/, '.go');
  } else if (/.*_tests?\.rs$/.test(base)) {
    sourceBase = base.replace(/_tests?\.rs$/, '.rs');
  } else {
    return null;
  }

  return dir === '.' ? sourceBase : `${dir}/${sourceBase}`;
}
