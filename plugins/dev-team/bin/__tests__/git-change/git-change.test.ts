/**
 * 集成测试: test-resolve-paths git-change 模式 (AC-5)
 *
 * 验证 modules: "git-change" 时通过 git diff 发现变更文件并推导测试路径。
 *
 * @see openspec/changes/test-resolve-paths-config-dirs/test-design.md
 */

import { execSync } from 'node:child_process';
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
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'git-change-'));
  // 初始化 git 仓库
  execSync('git init', { cwd: tmpDir, stdio: 'pipe' });
  execSync('git config user.name test && git config user.email test@test.com', {
    cwd: tmpDir,
    stdio: 'pipe',
  });
  // 创建 openspec/config.json
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

function gitCommit(projectRoot: string, message: string): void {
  execSync('git add -A', { cwd: projectRoot, stdio: 'pipe' });
  execSync(`git commit -m "${message}"`, { cwd: projectRoot, stdio: 'pipe' });
}

// ===========================================================================
// git-change 模式 (AC-5)
// ===========================================================================

describe('git-change 模式 — git diff 变更文件推导', () => {
  it('在工作树中有未暂存变更时调用 modules: "git-change"，unit_tests 包含变更文件的推导结果 (AC-5)', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        overrides: [{ file: 'src', framework: 'vite-plus' }],
      },
    });
    // 创建初始提交
    writeFile(project.root, 'src/unchanged.ts', '');
    writeFile(project.root, 'src/changed.ts', '// original');
    gitCommit(project.root, 'initial');

    // 做未暂存变更 — 修改已有的 tracked 文件
    writeFile(project.root, 'src/changed.ts', '// modified');

    const result = runTestResolvePaths({
      modules: 'git-change',
      project_root: project.root,
    });

    expect(result.unit_tests).toContainEqual({
      source: 'src/changed.ts',
      test_file: 'src/changed.test.ts',
    });
    expect(result.unit_tests).not.toContainEqual(
      expect.objectContaining({ source: 'src/unchanged.ts' }),
    );
    project.cleanup();
  });

  it('干净的工作树中调用 modules: "git-change"，unit_tests 为空 (AC-5)', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        overrides: [{ file: 'src', framework: 'vite-plus' }],
      },
    });
    writeFile(project.root, 'src/foo.ts', '');
    gitCommit(project.root, 'all clean');

    const result = runTestResolvePaths({
      modules: 'git-change',
      project_root: project.root,
    });

    expect(result.unit_tests).toEqual([]);
    project.cleanup();
  });

  it('非 git 目录中调用 modules: "git-change"，errors 包含 git 错误 (AC-5)', () => {
    // 不使用 createTempProject（它创建 git 仓库），手动创建非 git 目录
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'non-git-'));
    try {
      const result = runTestResolvePaths({
        modules: 'git-change',
        project_root: tmpDir,
      });

      expect(result.errors.length).toBeGreaterThan(0);
      expect(result.errors[0].path).toBe('git');
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });
});
