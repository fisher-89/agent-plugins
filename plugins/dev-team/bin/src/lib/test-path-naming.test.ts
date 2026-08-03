/**
 * 单元测试: lib/test-path-naming — colocated test ↔ source 命名约定
 *
 * @see plugins/dev-team/bin/src/lib/test-path-naming.ts
 */

import { describe, expect, it } from 'vite-plus/test';

import {
  deriveSourcePathFromTestFile,
  deriveUnitTestPath,
  isSourceFile,
  isTestFile,
} from './test-path-naming';

describe('isTestFile / isSourceFile', () => {
  it('识别 JS/TS、Python、Go、Rust 测试命名约定', () => {
    expect(isTestFile('src/foo.test.ts')).toBe(true);
    expect(isTestFile('src/test_foo.py')).toBe(true);
    expect(isTestFile('src/foo_test.go')).toBe(true);
    expect(isTestFile('src/foo_test.rs')).toBe(true);
    expect(isTestFile('src/foo_tests.rs')).toBe(true);
    expect(isTestFile('src/foo.ts')).toBe(false);
    expect(isTestFile('src/testX.py')).toBe(false);
  });

  it('isSourceFile 排除测试文件并校验扩展名', () => {
    expect(isSourceFile('src/foo.ts')).toBe(true);
    expect(isSourceFile('src/foo.py')).toBe(true);
    expect(isSourceFile('src/foo.go')).toBe(true);
    expect(isSourceFile('src/foo.rs')).toBe(true);
    expect(isSourceFile('src/foo.test.ts')).toBe(false);
    expect(isSourceFile('src/test_foo.py')).toBe(false);
    expect(isSourceFile('src/foo_test.go')).toBe(false);
    expect(isSourceFile('src/foo_tests.rs')).toBe(false);
    expect(isSourceFile('docs/a.md')).toBe(false);
  });
});

describe('deriveUnitTestPath ↔ deriveSourcePathFromTestFile', () => {
  const cases: Array<{ src: string; test: string }> = [
    { src: 'src/a.ts', test: 'src/a.test.ts' },
    { src: 'src/a.tsx', test: 'src/a.test.tsx' },
    { src: 'src/a.js', test: 'src/a.test.js' },
    { src: 'src/a.jsx', test: 'src/a.test.jsx' },
    { src: 'src/a.mjs', test: 'src/a.test.mjs' },
    { src: 'src/a.cjs', test: 'src/a.test.cjs' },
    { src: 'src/a.py', test: 'src/test_a.py' },
    { src: 'src/a.go', test: 'src/a_test.go' },
    { src: 'src/a.rs', test: 'src/a_test.rs' },
    { src: 'foo.ts', test: 'foo.test.ts' },
  ];

  it('正向派生与反向推断对支持扩展名可往返', () => {
    for (const { src, test } of cases) {
      expect(deriveUnitTestPath(src)).toBe(test);
      expect(deriveSourcePathFromTestFile(test)).toBe(src);
    }
  });

  it('foo_tests.rs 反向推断为 foo.rs', () => {
    expect(deriveSourcePathFromTestFile('src/foo_tests.rs')).toBe('src/foo.rs');
  });

  it('非测试文件反向推断返回 null；绝对路径与反斜杠可正确反推', () => {
    expect(deriveSourcePathFromTestFile('src/foo.ts')).toBeNull();
    expect(deriveSourcePathFromTestFile('src/testX.py')).toBeNull();
    expect(deriveSourcePathFromTestFile('D:/proj/src/foo.test.ts')).toBe('D:/proj/src/foo.ts');
    expect(deriveSourcePathFromTestFile('D:\\proj\\src\\foo.test.ts')).toBe('D:/proj/src/foo.ts');
  });
});
