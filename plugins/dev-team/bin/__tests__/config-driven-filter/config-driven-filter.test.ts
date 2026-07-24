/**
 * 集成测试: test-resolve-paths config-driven 过滤 (AC-3)
 *
 * 验证非空 modules 时经 test_detect_frameworks 过滤后再推导。
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
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-filter-'));
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
// config-driven 过滤 (AC-3)
// ===========================================================================

describe('config-driven 过滤 — 非空 modules 经 test_detect_frameworks 过滤后推导', () => {
  it('modules: ["src/commands/test-resolve-paths.ts", "node_modules/some-dep/index.ts"] 时仅前者进入 unit_tests (AC-3)', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'src',
          framework: 'vite-plus',
          includes: ['**/*.{ts,tsx}'],
        },
      ],
    });
    try {
      const result = runTestResolvePaths({
        modules: ['src/commands/test-resolve-paths.ts', 'node_modules/some-dep/index.ts'],
        project_root: project.root,
      });

      // src/commands/test-resolve-paths.ts 在 src/ 范围内，应被检测并进入 unit_tests
      expect(result.unit_tests).toContainEqual({
        source: 'src/commands/test-resolve-paths.ts',
        test_file: 'src/commands/test-resolve-paths.test.ts',
      });

      // node_modules/some-dep/index.ts 不在 src/ 范围内
      // （被标记为 unknown 但仍进入 detectedSet，故不出现在 errors）
      expect(result.errors.filter((e) => e.path === 'node_modules/some-dep/index.ts').length).toBe(
        0,
      );
    } finally {
      project.cleanup();
    }
  });
});
