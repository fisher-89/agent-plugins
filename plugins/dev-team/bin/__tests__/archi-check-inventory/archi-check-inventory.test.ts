/**
 * 集成测试: change 文件清单 → archi_check 被查文件集（清单直通）
 *
 * 真实链路（模型解析与清单读取全为真实实现，不 mock）:
 * - `bin/src/lib/c4-cross-ref.ts` — 检查方（被查文件集组装与交叉引用）
 * - `bin/src/modules/workflow.ts` — 清单来源（真实 workflow.json 读盘）
 * - `bin/src/lib/c4-parser.ts` — 模型参照系（真实 openspec/architecture/models/*.c4 解析管线）
 * - `bin/src/commands/change-create.ts` — change fixture 的真实创建通道
 *
 * 只 mock 边界: node:child_process 与 child_process 的 execSync / execFileSync
 * （委托真实实现，仅作 staged 废弃用例的 not-called 断言）。project_root 经
 * runCrossRefCheck 首参显式传入临时工程根（与 inventory-backtrack-preserve 同策略，
 * 不经 getProjectDir 解析链，亦不使用 process.chdir）。
 *
 * 覆盖 test-design「集成测试」场景: 清单直通检查。关联 AC: AC-9。
 *
 * @see openspec/changes/workflow-file-inventory/test-design.md — 集成测试「change 清单 → archi_check 被查文件集」
 */

import type * as LegacyChildProcess from 'child_process';
import * as fs from 'fs';
import type * as NodeChildProcess from 'node:child_process';
import * as os from 'os';
import * as path from 'path';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { runChangeCreate } from '../../src/commands/change-create';
import { runCrossRefCheck } from '../../src/lib/c4-cross-ref';
import { writeFileInventory } from '../../src/modules/workflow';

// ---------------------------------------------------------------------------
// git 退场 / staged 废弃防回归：child_process 全程 spy（仅作 not-called 断言）
// ---------------------------------------------------------------------------

vi.mock('node:child_process', async () => {
  const actual = await vi.importActual<typeof NodeChildProcess>('node:child_process');
  return { ...actual, execSync: vi.fn(actual.execSync), execFileSync: vi.fn(actual.execFileSync) };
});

vi.mock('child_process', async () => {
  const actual = await vi.importActual<typeof LegacyChildProcess>('child_process');
  return { ...actual, execSync: vi.fn(actual.execSync), execFileSync: vi.fn(actual.execFileSync) };
});

import { execSync as spiedExecSync } from 'node:child_process';

// ---------------------------------------------------------------------------
// Fixture：真实 .c4 模型 + 真实源文件 + runChangeCreate 创建的真实 change
// ---------------------------------------------------------------------------

const CHANGE = 'inv-change';

/** 两元素以上、声明 Frontend -> Backend 一条关系的模型（走真实 LikeC4 解析管线）。 */
const MODEL_DSL = `specification {
  element package
}
model {
  package Frontend {
    metadata { path './src/frontend/' }
  }
  package Backend {
    metadata { path './src/backend/' }
  }
  package Auth {
    metadata { path './src/auth/' }
  }
  Frontend -> Backend "calls API"
}`;

const SOURCE_FILES: Record<string, string> = {
  // 已建模文件：import Backend（关系已声明，合法）+ import Auth（关系未声明 → violation）
  'src/frontend/app.ts':
    "import { api } from '../backend/api';\nimport { guard } from '../auth/guard';",
  'src/backend/api.ts': 'export const api = () => {};',
  'src/auth/guard.ts': 'export const guard = () => {};',
  // 未建模文件（测试文件——清单直通不做 test config 过滤的可见证据）
  'src/unmapped/app.test.ts': 'export const smoke = 1;',
};

let projectRoot: string;

function writeModelFixture(): void {
  const modelsDir = path.join(projectRoot, 'openspec', 'architecture', 'models');
  fs.mkdirSync(modelsDir, { recursive: true });
  fs.writeFileSync(path.join(modelsDir, 'spec.c4'), MODEL_DSL, 'utf-8');
}

function writeSourceFiles(): void {
  for (const [rel, content] of Object.entries(SOURCE_FILES)) {
    const full = path.join(projectRoot, rel);
    fs.mkdirSync(path.dirname(full), { recursive: true });
    fs.writeFileSync(full, content, 'utf-8');
  }
}

/** runChangeCreate 真实创建 change，inventory 缺省即初始空净状态。 */
function createChange(inventory?: { written: string[]; deleted: string[] }): string {
  const created = runChangeCreate(CHANGE, projectRoot, 'requirement');
  if (inventory) {
    writeFileInventory(created.path, inventory);
  }
  return created.path;
}

/** 机制前旧 change：手工写入无 files 字段的 workflow.json。 */
function createLegacyChange(): string {
  const changeDir = path.join(projectRoot, 'openspec', 'changes', CHANGE);
  fs.mkdirSync(changeDir, { recursive: true });
  fs.writeFileSync(
    path.join(changeDir, 'workflow.json'),
    JSON.stringify({ workflow_type: 'requirement', created: '2026-09-17' }),
    'utf-8',
  );
  return changeDir;
}

beforeEach(() => {
  projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'archi-check-inv-'));
  writeModelFixture();
  writeSourceFiles();
});

afterEach(() => {
  fs.rmSync(projectRoot, { recursive: true, force: true });
});

// ===========================================================================
// 场景: 清单直通检查
// ===========================================================================

describe('场景: 清单直通检查（change 清单 → archi_check 被查文件集）', () => {
  it('正向：change 清单模式 → files.written 全量参与被查（测试文件直通不过滤），matched / unmatched_files / violations 反映直通集合 (AC-9)', async () => {
    createChange({ written: ['src/frontend/app.ts', 'src/unmapped/app.test.ts'], deleted: [] });

    const result = await runCrossRefCheck(projectRoot, { change: CHANGE });

    // 直通集合整体进入被查：已建模文件按模型映射匹配 Frontend
    expect(result.matched).toEqual([{ element_id: 'Frontend', files: ['src/frontend/app.ts'] }]);
    // 测试文件未被过滤，原样进 unmatched_files（直通的可见后果）
    expect(result.unmatched_files).toEqual(['src/unmapped/app.test.ts']);
    // app.ts import Backend（关系已声明，合法）+ import Auth（关系未声明）→ 恰一条 unmodeled_dependency
    expect(result.violations).toHaveLength(1);
    expect(result.violations[0]).toMatchObject({
      type: 'unmodeled_dependency',
      source: 'Frontend',
      target: 'Auth',
      file: 'src/frontend/app.ts',
    });
    expect(result.warnings).toEqual([]);
    expect(result.status).toBe('violations_found');
  });

  it('正向：显式 files 与 change 同传 → files 优先，清单不参与被查 (AC-9)', async () => {
    // 清单 written 本会匹配 Frontend；显式 files 胜出后被查集合只有显式列表
    createChange({ written: ['src/frontend/app.ts'], deleted: [] });

    const result = await runCrossRefCheck(projectRoot, {
      change: CHANGE,
      files: ['src/unmapped/app.test.ts'],
    });

    expect(result.matched).toEqual([]);
    expect(result.unmatched_files).toEqual(['src/unmapped/app.test.ts']);
    expect(result.status).toBe('clean');
  });

  it('边界：written 为空数组 → status no_changes（不报错、不产出违规）', async () => {
    createChange(); // runChangeCreate 初始 files = { written: [], deleted: [] }

    const result = await runCrossRefCheck(projectRoot, { change: CHANGE });

    expect(result.status).toBe('no_changes');
    expect(result.violations).toEqual([]);
    expect(result.matched).toEqual([]);
    expect(result.unmatched_files).toEqual([]);
  });

  it('异常：staged: true → 显式抛错（清单模式替代），且全程无任何 git 子进程调用 (AC-9)', async () => {
    createChange({ written: ['src/frontend/app.ts'], deleted: [] });

    await expect(runCrossRefCheck(projectRoot, { staged: true })).rejects.toThrow(
      /staged 模式已由清单模式替代/,
    );
    expect(vi.mocked(spiedExecSync)).not.toHaveBeenCalled();
  });

  it('异常：无 files 的机制前旧 change → 硬报错指引重建（模型在场，非静默 skipped，AC-9）', async () => {
    createLegacyChange();

    await expect(runCrossRefCheck(projectRoot, { change: CHANGE })).rejects.toThrow(
      /该 change 创建于文件清单机制之前，请重建/,
    );
  });
});
