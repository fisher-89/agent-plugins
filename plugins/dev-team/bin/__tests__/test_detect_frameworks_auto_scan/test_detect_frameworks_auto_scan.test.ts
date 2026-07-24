/**
 * 集成测试: test_detect_frameworks 自动扫描行为（tests[]）
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { describe, expect, it } from 'vite-plus/test';

import { runTestDetectFrameworks } from '../../src/commands/test-detect-frameworks';

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(configData: unknown): TempProject {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'auto-scan-test-'));
  const openspecDir = path.join(tmpDir, 'openspec');
  fs.mkdirSync(openspecDir, { recursive: true });
  fs.writeFileSync(
    path.join(openspecDir, 'config.json'),
    JSON.stringify(configData, null, 2),
    'utf-8',
  );
  return {
    root: tmpDir,
    cleanup: () => fs.rmSync(tmpDir, { recursive: true, force: true }),
  };
}

function writeFile(projectRoot: string, relativePath: string, content = ''): void {
  const fullPath = path.join(projectRoot, relativePath);
  fs.mkdirSync(path.dirname(fullPath), { recursive: true });
  fs.writeFileSync(fullPath, content, 'utf-8');
}

describe('test_detect_frameworks — 自动扫描匹配文件集合', () => {
  it('省略 files 参数时应检测 src/app.test.ts 与 tests/auth.rs 并归属正确框架', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        { root: 'src', framework: 'vitest', includes: ['**/*.{test,spec}.{ts,tsx,js,jsx}'] },
        { root: 'tests', framework: 'rust', includes: ['**/*.rs'] },
      ],
    });
    try {
      writeFile(project.root, 'src/app.test.ts');
      writeFile(project.root, 'tests/auth.rs');

      const result = runTestDetectFrameworks({ projectRoot: project.root });

      expect(result.detected).toHaveLength(2);
      const tsEntry = result.detected.find((d) => d.file.endsWith('app.test.ts'));
      const rsEntry = result.detected.find((d) => d.file.endsWith('auth.rs'));
      expect(tsEntry?.framework).toBe('vitest');
      expect(rsEntry?.framework).toBe('rust');
    } finally {
      project.cleanup();
    }
  });

  it('仅含 readme.md 非测试文件时 detected 应为空', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'src', framework: 'vitest' }],
    });
    try {
      writeFile(project.root, 'readme.md', '# readme');

      const result = runTestDetectFrameworks({ projectRoot: project.root });

      expect(result.detected).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('node_modules/pkg/index.test.ts 不应被自动扫描纳入 detected', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'src', framework: 'vitest' }],
    });
    try {
      writeFile(project.root, 'node_modules/pkg/index.test.ts');
      writeFile(project.root, 'src/valid.test.ts');

      const result = runTestDetectFrameworks({ projectRoot: project.root });

      expect(result.detected.every((d) => !d.file.includes('node_modules'))).toBe(true);
      expect(result.detected.some((d) => d.file.endsWith('valid.test.ts'))).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('自动扫描结果不应含 unknown 非测试文件', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'src', framework: 'vitest' }],
    });
    try {
      writeFile(project.root, 'src/app.test.ts');
      writeFile(project.root, 'src/helper.ts');

      const result = runTestDetectFrameworks({ projectRoot: project.root });

      expect(result.detected.every((d) => d.framework !== 'unknown')).toBe(true);
      expect(result.detected).toHaveLength(1);
      expect(result.detected[0].file.endsWith('app.test.ts')).toBe(true);
    } finally {
      project.cleanup();
    }
  });
});
