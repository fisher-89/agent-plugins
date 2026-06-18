/**
 * 集成测试: test_detect_frameworks 首匹配规则（AC-6）
 *
 * 验证文件同时匹配多个 glob 时 frameworks 数组中第一个匹配的框架生效。
 *
 * @see openspec/changes/use-fast-glob/test-design.md
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { describe, expect, it } from 'vite-plus/test';

import { runTestDetectFrameworks } from '../../src/commands/test-detect-frameworks';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(configData: unknown): TempProject {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'first-match-test-'));
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

// ===========================================================================
// test_detect_frameworks — 首匹配规则（AC-6）
// ===========================================================================

describe('test_detect_frameworks — 首匹配规则', () => {
  it('tests/e2e/test_app.ts 同时匹配 **/*.ts 与 **/e2e/** 时应归属数组首项 vitest', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        overrides: [
          { file: '**/*.ts', framework: 'vitest' },
          { file: '**/e2e/**', framework: 'vite-plus' },
        ],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['tests/e2e/test_app.ts'],
        projectRoot: project.root,
      });
      expect(result.detected[0].framework).toBe('vitest');
    } finally {
      project.cleanup();
    }
  });

  it('交换 override 顺序后框架归属应随之改变', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        overrides: [
          { file: '**/e2e/**', framework: 'vite-plus' },
          { file: '**/*.ts', framework: 'vitest' },
        ],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['tests/e2e/test_app.ts'],
        projectRoot: project.root,
      });
      expect(result.detected[0].framework).toBe('vite-plus');
    } finally {
      project.cleanup();
    }
  });
});
