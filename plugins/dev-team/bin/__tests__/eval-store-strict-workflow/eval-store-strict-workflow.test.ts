/**
 * 集成测试: 同一非法 workflow.json → 严格方抛错 / 容错方不崩溃
 *
 * 本关系验证「一个非法文件、两种失败语义」的跨模块契约：读 / 写关键路径必须硬失败
 * 并给出可操作指引，而列表路径必须软失败以保持 change 可见。集成测试在同一个真实
 * 临时 change 目录上，对同一份缺失 / 非法的 `workflow.json` 依次调用各入口，断言
 * 彼此不矛盾。
 *
 * 涉及模块:
 * - `plugins/dev-team/bin/src/lib/change-config.ts` — 严格类型读取
 * - `plugins/dev-team/bin/src/commands/phase-next.ts` — 严格消费者
 * - `plugins/dev-team/bin/src/commands/backtrack.ts` — 严格消费者
 * - `plugins/dev-team/bin/src/lib/eval-json.ts` — 写入前置校验
 * - `plugins/dev-team/bin/src/commands/change-list.ts` — 容错消费者
 *
 * 关联 AC: AC-13, AC-14
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { runBacktrack } from '../../src/commands/backtrack';
import { runChangeList } from '../../src/commands/change-list';
import { runPhaseLog } from '../../src/commands/phase-log';
import { runPhaseNext } from '../../src/commands/phase-next';
import { getProjectDir } from '../../src/lib/project-root';

// `getWorkflowType()` 经 `getProjectDir()` 解析工程根，优先级为 call-scoped →
// `CLAUDE_PROJECT_DIR` → `WORKSPACE_FOLDER_PATHS` → `process.cwd()`。这里把
// `getProjectDir` mock 成临时工程根，替代 `process.chdir`：Stryker 的 vitest runner
// 强制 worker 线程池，worker 中 `process.chdir` 会抛
// ERR_WORKER_UNSUPPORTED_OPERATION（与 run-static-analysis.test.ts 相同的处理）。
// 环境变量隔离仍然保留：被测命令之外的真实工程根不得介入。
vi.mock('../../src/lib/project-root');

const CHANGE = 'strict-workflow';
const RUN_ID = 'strict-run';

const ENV_KEYS = ['CLAUDE_PROJECT_DIR', 'WORKSPACE_FOLDER_PATHS'] as const;

function saveProjectEnv(): Record<(typeof ENV_KEYS)[number], string | undefined> {
  return {
    CLAUDE_PROJECT_DIR: process.env.CLAUDE_PROJECT_DIR,
    WORKSPACE_FOLDER_PATHS: process.env.WORKSPACE_FOLDER_PATHS,
  };
}

function restoreProjectEnv(saved: ReturnType<typeof saveProjectEnv>): void {
  for (const key of ENV_KEYS) {
    if (saved[key] === undefined) {
      delete process.env[key];
    } else {
      process.env[key] = saved[key];
    }
  }
}

let projectRoot: string;
let changeDir: string;
let savedProjectEnv: ReturnType<typeof saveProjectEnv>;

beforeEach(() => {
  savedProjectEnv = saveProjectEnv();
  delete process.env.CLAUDE_PROJECT_DIR;
  delete process.env.WORKSPACE_FOLDER_PATHS;
  projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'eval-strict-'));
  changeDir = path.join(projectRoot, 'openspec', 'changes', CHANGE);
  fs.mkdirSync(changeDir, { recursive: true });
  vi.mocked(getProjectDir).mockReturnValue(projectRoot);
});

afterEach(() => {
  restoreProjectEnv(savedProjectEnv);
  fs.rmSync(projectRoot, { recursive: true, force: true });
});

function writeRawWorkflow(raw: string): void {
  fs.writeFileSync(path.join(changeDir, 'workflow.json'), raw, 'utf-8');
}

function snapshotDir(): Record<string, string> {
  const snapshot: Record<string, string> = {};
  for (const name of fs.readdirSync(changeDir).sort()) {
    snapshot[name] = fs.readFileSync(path.join(changeDir, name), 'utf-8');
  }
  return snapshot;
}

function expectPhaseNext(): Error {
  try {
    runPhaseNext({ change: CHANGE, project_root: projectRoot, run_id: RUN_ID });
  } catch (e: unknown) {
    return e as Error;
  }
  throw new Error('expected runPhaseNext to throw, but it returned normally');
}

function expectBacktrack(): Error {
  try {
    runBacktrack({
      project_root: projectRoot,
      change: CHANGE,
      phase: 'proposal',
      backtrack_to: 'proposal',
      backtrack_reason: 'r',
    });
  } catch (e: unknown) {
    return e as Error;
  }
  throw new Error('expected runBacktrack to throw, but it returned normally');
}

function expectPhaseLog(): Error {
  try {
    runPhaseLog({
      project_root: projectRoot,
      change: CHANGE,
      phase: 'proposal',
      report: 'ok',
      checklist: [{ item: 'x', pass: true, evidence: 'ok' }],
    });
  } catch (e: unknown) {
    return e as Error;
  }
  throw new Error('expected runPhaseLog to throw, but it returned normally');
}

/** 容错方：始终返回结果而非抛错。 */
function changeEntry(): ReturnType<typeof runChangeList>['changes'][number] {
  const entry = runChangeList(projectRoot).changes.find((c) => c.name === CHANGE);
  expect(entry).toBeDefined();
  return entry!;
}

// ---------------------------------------------------------------------------
// 场景: workflow.json 缺失
// ---------------------------------------------------------------------------

describe('workflow.json 缺失', () => {
  it('runChangeList 仍列出该 change，workflow_done 为 false、latest_phase 为 null（AC-13 容错面）', () => {
    const entry = changeEntry();

    expect(entry.workflow_done).toBe(false);
    expect(entry.latest_phase).toBeNull();
  });

  it('runPhaseNext 抛错，message 含绝对路径与 change_create 指引，不返回 proposal（AC-13）', () => {
    const error = expectPhaseNext();

    expect(error.message).toContain('workflow.json 不存在');
    expect(error.message).toContain(path.join(changeDir, 'workflow.json'));
    expect(error.message).toContain('change_create');
  });

  it('runBacktrack 抛错，message 含绝对路径与 change_create 指引（AC-13）', () => {
    const error = expectBacktrack();

    expect(error.message).toContain('workflow.json 不存在');
    expect(error.message).toContain(path.join(changeDir, 'workflow.json'));
    expect(error.message).toContain('change_create');
  });

  it('runPhaseLog 抛错（workflow.json 缺失在前置读取处暴露），且目录内未新增 workflow.json / eval.json / 子目录（AC-13）', () => {
    const before = snapshotDir();

    const error = expectPhaseLog();

    expect(error.message).toContain('workflow.json 不存在');
    expect(error.message).toContain('change_create');
    expect(snapshotDir()).toEqual(before);
    expect(fs.readdirSync(changeDir)).toEqual([]);
  });

  it('四次调用后目录内容与调用前完全一致（严格方只读失败，无副作用）', () => {
    const before = snapshotDir();

    changeEntry();
    expectPhaseNext();
    expectBacktrack();
    expectPhaseLog();

    expect(snapshotDir()).toEqual(before);
  });
});

// ---------------------------------------------------------------------------
// 场景: workflow.json 格式非法
// ---------------------------------------------------------------------------

describe('workflow.json 格式非法', () => {
  const invalidDocuments: Record<string, string> = {
    '非法 JSON': '{ "workflow_type": "requirement" ',
    根为数组: JSON.stringify([{ phase: 'proposal', verdict: 'pass' }]),
    '根为 null': JSON.stringify(null),
    '缺 workflow_type': JSON.stringify({ created: '2026-09-11' }),
    'workflow_type 非枚举': JSON.stringify({ workflow_type: 'unknown-flow' }),
    'created 非 YYYY-MM-DD': JSON.stringify({
      workflow_type: 'requirement',
      created: '2026/09/11',
    }),
    'eval 为对象': JSON.stringify({ workflow_type: 'requirement', eval: {} }),
  };

  it('非法 JSON / 根为数组时 runPhaseNext、runBacktrack 抛错；磁盘文件内容未被改写（AC-14）', () => {
    for (const key of ['非法 JSON', '根为数组']) {
      writeRawWorkflow(invalidDocuments[key]);
      const before = fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8');

      expectPhaseNext();
      expectBacktrack();
      expect(fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8')).toBe(before);
    }
  });

  it('workflow_type 缺失 / 非枚举时抛错，message 含字段路径 workflow_type（AC-14）', () => {
    for (const key of ['缺 workflow_type', 'workflow_type 非枚举']) {
      writeRawWorkflow(invalidDocuments[key]);

      expect(expectPhaseNext().message).toContain('workflow_type');
      expect(expectBacktrack().message).toContain('workflow_type');
    }
  });

  it('created 非 YYYY-MM-DD 时抛错（AC-14）', () => {
    writeRawWorkflow(invalidDocuments['created 非 YYYY-MM-DD']);

    expect(expectPhaseNext().message).toContain('created');
    expect(expectBacktrack().message).toContain('created');
  });

  it('eval 为对象时 runPhaseLog 抛错且不把 eval 改写成合法数组（AC-12、AC-14）', () => {
    writeRawWorkflow(invalidDocuments['eval 为对象']);
    const before = fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8');

    const error = expectPhaseLog();

    expect(error.message).toContain('读取 workflow.json 失败');
    const doc = JSON.parse(
      fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8'),
    ) as Record<string, unknown>;
    expect(doc.eval).toEqual({});
    expect(fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8')).toBe(before);
  });

  it('根为 null / 缺 workflow_type 时 runPhaseLog 亦失败且不改写磁盘', () => {
    for (const key of ['根为 null', '缺 workflow_type']) {
      writeRawWorkflow(invalidDocuments[key]);
      const before = fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8');

      expectPhaseLog();
      expect(fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8')).toBe(before);
    }
  });

  it('同一非法文件下 runChangeList 均返回 workflow_done===false、latest_phase===null，change 仍出现（AC-14 容错面）', () => {
    for (const raw of Object.values(invalidDocuments)) {
      writeRawWorkflow(raw);

      let entry: ReturnType<typeof runChangeList>['changes'][number] | undefined;
      expect(() => {
        entry = runChangeList(projectRoot).changes.find((c) => c.name === CHANGE);
      }).not.toThrow();

      expect(entry).toBeDefined();
      expect(entry!.workflow_done).toBe(false);
      expect(entry!.latest_phase).toBeNull();
    }
  });

  it('非法 / 缺字段时不再按缺省 requirement 继续推进工作流（DEFAULT_WORKFLOW_TYPE 兜底语义已废弃）', () => {
    writeRawWorkflow(invalidDocuments['缺 workflow_type']);

    let result: ReturnType<typeof runPhaseNext> | null = null;
    try {
      result = runPhaseNext({ change: CHANGE, project_root: projectRoot, run_id: RUN_ID });
    } catch {
      result = null;
    }

    // 严格化后不允许返回任何 phase（尤其不得以 requirement 表返回 proposal）
    expect(result).toBeNull();
  });
});
