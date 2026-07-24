/**
 * 集成测试: test-resolve-paths 无 tests 配置
 *
 * @see openspec/changes/tests-array-cwd-config/test-design.md
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { describe, expect, it } from 'vite-plus/test';

import { runTestResolvePaths } from '../../src/commands/test-resolve-paths';

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(configData: unknown): TempProject {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'no-test-config-'));
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

describe('无 tests 配置 — modules: [] 时 errors 包含指导性消息', () => {
  it('config.json 仅有 schema 字段时，errors 应包含配置提示且 unit_tests 为空', () => {
    const project = createTempProject({
      schema: 'spec-driven',
    });
    try {
      const result = runTestResolvePaths({
        modules: [],
        project_root: project.root,
      });

      expect(result.errors.length).toBeGreaterThan(0);
      expect(result.errors[0].path).toBe('config');
      expect(result.errors[0].message).toContain('tests');
      expect(result.unit_tests).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('config.json 存在但 tests 为空数组时 errors 应包含配置提示', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [],
    });
    try {
      const result = runTestResolvePaths({
        modules: [],
        project_root: project.root,
      });

      expect(result.errors.length).toBeGreaterThan(0);
      expect(result.errors[0].message).toContain('tests');
      expect(result.unit_tests).toEqual([]);
    } finally {
      project.cleanup();
    }
  });
});
