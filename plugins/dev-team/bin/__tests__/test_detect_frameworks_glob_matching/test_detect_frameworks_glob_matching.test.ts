/**
 * 集成测试: test_detect_frameworks glob 模式端到端匹配（tests[]）
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
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'glob-matching-test-'));
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

function findDetected(
  detected: { file: string; framework: string }[],
  suffix: string,
): { file: string; framework: string } | undefined {
  const normalized = suffix.replace(/\\/g, '/');
  return detected.find((d) => d.file.replace(/\\/g, '/').endsWith(normalized));
}

describe('test_detect_frameworks — glob 模式匹配', () => {
  it('应正确检测 {} 花括号、** 跨目录与无通配符目录前缀三类 pattern', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        { root: 'lib', framework: 'jest', includes: ['*.test.ts'] },
        { root: 'src', framework: 'vitest', includes: ['**/nested/**/*.test.ts'] },
        {
          root: 'plugins/dev-team/bin',
          framework: 'vite-plus',
          includes: ['**/*.{test,spec}.{ts,tsx,js,jsx}'],
        },
      ],
    });
    try {
      const result = runTestDetectFrameworks({
        files: [
          'lib/utils.test.ts',
          'src/nested/deep/helper.test.ts',
          'plugins/dev-team/bin/src/foo.test.ts',
        ],
        projectRoot: project.root,
      });

      const libEntry = findDetected(result.detected, 'lib/utils.test.ts');
      const nestedEntry = findDetected(result.detected, 'src/nested/deep/helper.test.ts');
      const binEntry = findDetected(result.detected, 'plugins/dev-team/bin/src/foo.test.ts');

      expect(libEntry?.framework).toBe('jest');
      expect(nestedEntry?.framework).toBe('vitest');
      expect(binEntry?.framework).toBe('vite-plus');
    } finally {
      project.cleanup();
    }
  });
});

describe('test_detect_frameworks — Windows 路径', () => {
  it('以反斜杠形式传入 files 参数时检测结果应与正斜杠形式一致', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'src', framework: 'vitest', includes: ['**/*.{test,spec}.ts'] }],
    });
    try {
      const posixResult = runTestDetectFrameworks({
        files: ['src/utils/helper.test.ts'],
        projectRoot: project.root,
      });
      const windowsResult = runTestDetectFrameworks({
        files: ['src\\utils\\helper.test.ts'],
        projectRoot: project.root,
      });

      expect(windowsResult.detected[0].framework).toBe(posixResult.detected[0].framework);
      expect(windowsResult.detected[0].framework).toBe('vitest');
    } finally {
      project.cleanup();
    }
  });
});
