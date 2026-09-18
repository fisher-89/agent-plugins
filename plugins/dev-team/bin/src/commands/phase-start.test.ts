/**
 * 单元测试: commands/phase-start.ts — phase_start 核心逻辑
 *
 * 覆盖范围（openspec/changes/phase-lifecycle-file-log/test-design.md）:
 * - AC-2: 合法 change + phase 表内 phase → 返回 {started: true, phase, attempt,
 *   start_at}；workflow.json 落盘 active_phase{phase, attempt, start_at}
 * - attempt 由 eval 历史推导（computeAttempt）且 retry 后递增
 * - last-wins 重入：已有 active_phase（他 phase）时再次调用 → 覆盖，不报错
 * - bug-fix / test-only 按对应 phase 表校验归属（如 test-only 接受 code-analyze）
 * - 非法 phase → 报错且 workflow.json 逐字节不变
 * - change 不存在 / workflow.json 缺失 / JSON 非法 → 报错，不创建文件
 * - 输入 schema 边界：9 值枚举外的 phase / 空串 / 缺 project_root → schema 拒绝
 *
 * Mock 策略: 文件系统不 mock——mkdtempSync 临时项目 + runChangeCreate / 手写
 * workflow.json fixture 真盘。getWorkflowType 经 getProjectDir() 解析项目根，
 * 按 inventory-backtrack-preserve 既有模式 vi.stubEnv('CLAUDE_PROJECT_DIR', …)
 * 指向临时项目根（不用 process.chdir）。
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { readActivePhase } from '../modules/workflow';
import { phaseStartInputSchema } from '../schemas';
import { runChangeCreate } from './change-create';
import { runPhaseStart } from './phase-start';

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(prefix = 'phase-start-test-'): TempProject {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
  return {
    root,
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

function workflowPath(changeDir: string): string {
  return path.join(changeDir, 'workflow.json');
}

function readWorkflowJsonRaw(changeDir: string): string {
  return fs.readFileSync(workflowPath(changeDir), 'utf-8');
}

function changeDirOf(project: TempProject, name = 'my-change'): string {
  return path.join(project.root, 'openspec', 'changes', name);
}

/** change_create 建立 change 后注入 eval 历史条目。 */
function writeEvalEntries(changeDir: string, entries: Array<Record<string, unknown>>): void {
  const doc = JSON.parse(readWorkflowJsonRaw(changeDir)) as Record<string, unknown>;
  doc.eval = entries;
  fs.writeFileSync(workflowPath(changeDir), `${JSON.stringify(doc, null, 2)}\n`, 'utf-8');
}

function evalEntry(phase: string, verdict: 'pass' | 'fail', attempt = 1): Record<string, unknown> {
  return {
    phase,
    verdict,
    attempt,
    timestamp: '2026-09-18T00:00:00.000Z',
    report: 'r',
    checklist: [{ item: 'i', pass: verdict === 'pass', evidence: 'e' }],
    backtrack_to: null,
  };
}

let project: TempProject;

beforeEach(() => {
  project = createTempProject();
  vi.stubEnv('CLAUDE_PROJECT_DIR', project.root);
});

afterEach(() => {
  vi.unstubAllEnvs();
  vi.restoreAllMocks();
  project.cleanup();
});

// ===========================================================================
// runPhaseStart — 正向 (AC-2)
// ===========================================================================

describe('runPhaseStart — 正向 (AC-2)', () => {
  it('合法 change + phase 表内 phase → 返回 {started, phase, attempt, start_at}，workflow.json 落盘 active_phase', () => {
    const created = runChangeCreate('my-change', project.root, 'requirement');

    const result = runPhaseStart({
      change: 'my-change',
      phase: 'implement',
      project_root: project.root,
    });

    expect(result.started).toBe(true);
    expect(result.phase).toBe('implement');
    expect(result.attempt).toBe(1);
    expect(result.start_at).toMatch(/^\d{4}-\d{2}-\d{2}T/);

    expect(readActivePhase(created.path)).toEqual({
      phase: 'implement',
      attempt: 1,
      start_at: result.start_at,
    });
  });

  it('该 phase 既有 1 条 fail eval 条目 → attempt=2（computeAttempt 推导，retry 后递增）；无条目 → attempt=1', () => {
    const withFail = runChangeCreate('retry-change', project.root, 'requirement');
    writeEvalEntries(withFail.path, [evalEntry('implement', 'fail')]);

    const retried = runPhaseStart({
      change: 'retry-change',
      phase: 'implement',
      project_root: project.root,
    });
    expect(retried.attempt).toBe(2);

    const fresh = runChangeCreate('fresh-change', project.root, 'requirement');
    const first = runPhaseStart({
      change: 'fresh-change',
      phase: 'implement',
      project_root: project.root,
    });
    expect(first.attempt).toBe(1);
    expect(readActivePhase(fresh.path)?.attempt).toBe(1);
  });

  it('重入 last-wins：已有 active_phase（他 phase）时再次调用 → 覆盖为新 phase 与新 start_at，不报错', () => {
    runChangeCreate('my-change', project.root, 'requirement');
    runPhaseStart({ change: 'my-change', phase: 'implement', project_root: project.root });
    const firstStartAt = readActivePhase(changeDirOf(project))?.start_at;

    const second = runPhaseStart({
      change: 'my-change',
      phase: 'test-gen',
      project_root: project.root,
    });

    expect(second.phase).toBe('test-gen');
    const active = readActivePhase(changeDirOf(project));
    expect(active?.phase).toBe('test-gen');
    expect(active?.attempt).toBe(1);
    expect(active?.start_at).not.toBe(firstStartAt);
  });

  it('workflow_type=test-only → 按对应 phase 表校验归属：接受 code-analyze，落盘 active_phase', () => {
    const created = runChangeCreate('flow-test-only', project.root, 'test-only');

    const result = runPhaseStart({
      change: 'flow-test-only',
      phase: 'code-analyze',
      project_root: project.root,
    });

    expect(result.started).toBe(true);
    expect(readActivePhase(created.path)?.phase).toBe('code-analyze');
  });

  it('workflow_type=bug-fix → 按 bug-fix phase 表校验归属：implement 合法', () => {
    const created = runChangeCreate('flow-bugfix', project.root, 'bug-fix');

    const result = runPhaseStart({
      change: 'flow-bugfix',
      phase: 'implement',
      project_root: project.root,
    });

    expect(result.started).toBe(true);
    expect(readActivePhase(created.path)?.phase).toBe('implement');
  });
});

// ===========================================================================
// runPhaseStart — 异常 (AC-2)
// ===========================================================================

describe('runPhaseStart — 异常 (AC-2)', () => {
  it('phase 不属于该 workflow_type phase 表（requirement 传 code-analyze）→ 报错且 workflow.json 逐字节不变', () => {
    const created = runChangeCreate('my-change', project.root, 'requirement');
    const before = readWorkflowJsonRaw(created.path);

    expect(() =>
      runPhaseStart({ change: 'my-change', phase: 'code-analyze', project_root: project.root }),
    ).toThrow(/不属于 workflow_type/);
    expect(() =>
      runPhaseStart({ change: 'my-change', phase: 'code-analyze', project_root: project.root }),
    ).toThrow(/active_phase 未写入/);

    expect(readWorkflowJsonRaw(created.path)).toBe(before);
    expect(readActivePhase(created.path)).toBeNull();
  });

  it('change 不存在 → 报错含 change_create 指引，不创建任何文件', () => {
    expect(() =>
      runPhaseStart({ change: 'ghost-change', phase: 'implement', project_root: project.root }),
    ).toThrow(/workflow\.json 不存在/);
    expect(() =>
      runPhaseStart({ change: 'ghost-change', phase: 'implement', project_root: project.root }),
    ).toThrow(/change_create/);

    expect(fs.existsSync(changeDirOf(project, 'ghost-change'))).toBe(false);
  });

  it('workflow.json JSON 非法 → 报「解析失败」且磁盘不变', () => {
    const changeDir = changeDirOf(project);
    fs.mkdirSync(changeDir, { recursive: true });
    fs.writeFileSync(workflowPath(changeDir), '{broken', 'utf-8');

    expect(() =>
      runPhaseStart({ change: 'my-change', phase: 'implement', project_root: project.root }),
    ).toThrow(/解析失败/);
    expect(readWorkflowJsonRaw(changeDir)).toBe('{broken');
  });
});

// ===========================================================================
// runPhaseStart — 边界（输入 schema 校验与跨根隔离）
// ===========================================================================

describe('runPhaseStart — 边界（输入 schema 校验）', () => {
  it('phase 为 9 值枚举外的值 / 空串 → phaseStartInputSchema 拒绝', () => {
    for (const phase of ['integration-test', '', 'PROPOSAL']) {
      expect(
        phaseStartInputSchema.safeParse({ change: 'c', phase, project_root: '/tmp/x' }).success,
      ).toBe(false);
    }
  });

  it('change 为空串 / project_root 缺省 → schema 拒绝（change 必填、project_root 必填）', () => {
    expect(
      phaseStartInputSchema.safeParse({ change: '', phase: 'implement', project_root: '/tmp/x' })
        .success,
    ).toBe(false);
    expect(phaseStartInputSchema.safeParse({ change: 'c', phase: 'implement' }).success).toBe(
      false,
    );
  });

  it('project_root 指向另一项目根 → 以该根定位 change（跨根隔离；getWorkflowType 经 env 兜底同根解析）', () => {
    const other = createTempProject();
    try {
      runChangeCreate('same-name', project.root, 'requirement');
      vi.stubEnv('CLAUDE_PROJECT_DIR', other.root);
      const createdB = runChangeCreate('same-name', other.root, 'bug-fix');

      const result = runPhaseStart({
        change: 'same-name',
        phase: 'implement',
        project_root: other.root,
      });

      expect(result.started).toBe(true);
      expect(readActivePhase(createdB.path)?.phase).toBe('implement');
      // 原项目的同名 change 未被触碰
      const dirA = path.join(project.root, 'openspec', 'changes', 'same-name');
      expect(readActivePhase(dirA)).toBeNull();
    } finally {
      other.cleanup();
    }
  });

  it('phase_start 不改写 eval 历史（eval 数组原样保留）', () => {
    const created = runChangeCreate('eval-untouched', project.root, 'requirement');
    writeEvalEntries(created.path, [evalEntry('implement', 'fail')]);
    const before = JSON.parse(readWorkflowJsonRaw(created.path)) as Record<string, unknown>;

    runPhaseStart({ change: 'eval-untouched', phase: 'implement', project_root: project.root });

    const after = JSON.parse(readWorkflowJsonRaw(created.path)) as Record<string, unknown>;
    expect(after.eval).toEqual(before.eval);
  });
});
