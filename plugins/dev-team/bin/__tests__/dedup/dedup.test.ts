/**
 * 集成测试: test-resolve-paths 去重 (AC-6)
 *
 * 验证跨目录/跨 source 的扫描结果去重。
 *
 * @see openspec/changes/test-resolve-paths-config-dirs/test-design.md
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { describe, expect, it } from 'vite-plus/test';

import { runTestResolvePaths } from '../../src/commands/test-resolve-paths';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(configData: unknown): TempProject {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'dedup-'));
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

// ===========================================================================
// 去重 (AC-6)
// ===========================================================================

describe('跨目录/跨 source 的扫描结果去重', () => {
  it('当 test.overrides 中多个条目指向同一目录时，unit_tests 不包含重复条目 (AC-6)', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        overrides: [
          { file: 'src', framework: 'vitest' },
          { file: 'src', framework: 'jest' },
        ],
      },
    });
    try {
      writeFile(project.root, 'src/app.ts', '');

      const result = runTestResolvePaths({
        modules: [],
        project_root: project.root,
      });

      expect(result.unit_tests).toHaveLength(1);
      expect(result.unit_tests[0].source).toBe('src/app.ts');
    } finally {
      project.cleanup();
    }
  });

  it('config-driven 扫描的目录下文件重复出现同一条目时 unit_tests 应去重', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        overrides: [{ file: 'src', framework: 'vitest' }],
      },
    });
    try {
      writeFile(project.root, 'src/app.ts', '');

      const result = runTestResolvePaths({
        modules: [],
        project_root: project.root,
      });

      expect(result.unit_tests).toHaveLength(1);
    } finally {
      project.cleanup();
    }
  });
});
