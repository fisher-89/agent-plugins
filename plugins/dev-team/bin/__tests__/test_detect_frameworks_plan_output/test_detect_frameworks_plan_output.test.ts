/**
 * 集成测试: test_detect_frameworks plan 输出结构（AC-8）
 *
 * 验证 detected、frameworks、plan 输出结构与 schema 不变。
 *
 * @see openspec/changes/use-fast-glob/test-design.md
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { describe, expect, it } from 'vite-plus/test';

import { runTestDetectFrameworks } from '../../src/commands/test-detect-frameworks';
import { testDetectFrameworksOutputSchema } from '../../src/schemas/test-detect-frameworks.schema';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(configData: unknown): TempProject {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'plan-output-test-'));
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
// test_detect_frameworks — plan 输出结构（AC-8）
// ===========================================================================

describe('test_detect_frameworks — plan 输出结构', () => {
  it('字符串简写 "vitest" 配置时 plan 应含完整字段且通过 schema 校验', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: { framework: 'vitest' },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/app.test.ts'],
        projectRoot: project.root,
      });

      expect(result.plan).toHaveLength(1);
      expect(result.plan[0]).toMatchObject({
        directory: '.',
        framework: 'vitest',
        coverage_format: 'istanbul',
      });
      expect(result.plan[0]).toHaveProperty('coverage_output');
      expect(result.plan[0]).toHaveProperty('coverage_artifacts');
      expect(result.plan[0]).toHaveProperty('script');

      const parsed = testDetectFrameworksOutputSchema.safeParse(result);
      expect(parsed.success).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('无 test.framework 配置时 plan 应为空数组且 detected 均为 unknown', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {},
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/app.test.ts', 'readme.md'],
        projectRoot: project.root,
      });

      expect(result.plan).toEqual([]);
      expect(result.detected.every((d) => d.framework === 'unknown')).toBe(true);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// plan 双脚本内容验证 (AC-2, AC-5)
// ===========================================================================

describe('test_detect_frameworks — plan 双脚本内容验证', () => {
  it('script.shell 包含 `rm -rf` 和 bash 语法命令', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: { framework: 'vitest' },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/app.test.ts'],
        projectRoot: project.root,
      });
      const shell = (result.plan[0].script as { shell: string; cmd: string }).shell;
      expect(shell).toContain('rm -rf');
      expect(shell).toContain('npx vitest run');
    } finally {
      project.cleanup();
    }
  });

  it('script.cmd 包含 `rmdir /s /q`、`cd /d` 和 cmd.exe 兼容语法', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ file: 'plugins/dev-team/bin/**', framework: 'vitest' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['plugins/dev-team/bin/app.test.ts'],
        projectRoot: project.root,
      });
      const cmd = (result.plan[1].script as { shell: string; cmd: string }).cmd;
      expect(cmd).toContain('rmdir /s /q');
      expect(cmd).toContain('cd /d');
      expect(cmd).toContain('\r\n');
    } finally {
      project.cleanup();
    }
  });

  it('脚本双输出通过 schema 校验', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: { framework: 'vitest' },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/app.test.ts'],
        projectRoot: project.root,
      });
      const parsed = testDetectFrameworksOutputSchema.safeParse(result);
      expect(parsed.success).toBe(true);
      expect(parsed.data!.plan[0].script).toHaveProperty('shell');
      expect(parsed.data!.plan[0].script).toHaveProperty('cmd');
    } finally {
      project.cleanup();
    }
  });
});
