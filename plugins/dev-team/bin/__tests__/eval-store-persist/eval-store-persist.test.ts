/**
 * 集成测试: phase_log / backtrack → writeEvalJson → workflow.json.eval
 *
 * 命令层不直接 `writeFileSync` 评估文件，统一经 `appendEntry` / `writeEvalJson`
 * 落盘。本套件在真实临时 change 目录上调用 `runPhaseLog` 与 `runBacktrack`
 * （不 mock `eval-json`），验证 CLI / MCP 共用入口把条目写入 `workflow.json.eval`、
 * 保留 `workflow_type` / `created` / 未知键、成功后删除遗留 `eval.json`，且过程中
 * 永不创建或更新 `eval.json` 作为权威文件。
 *
 * 涉及模块:
 * - `plugins/dev-team/bin/src/commands/phase-log.ts` — 写入触发方
 * - `plugins/dev-team/bin/src/commands/backtrack.ts` — 写入触发方
 * - `plugins/dev-team/bin/src/lib/eval-json.ts` — 权威 IO
 *
 * 关联 AC: AC-1, AC-2, AC-4, AC-13
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { runBacktrack } from '../../src/commands/backtrack';
import { runPhaseLog } from '../../src/commands/phase-log';
import { type EvalEntry } from '../../src/lib/eval-json';
import { getProjectDir } from '../../src/lib/project-root';

// `getWorkflowType()` 经 `getProjectDir()` 解析工程根，优先级为 call-scoped →
// `CLAUDE_PROJECT_DIR` → `WORKSPACE_FOLDER_PATHS` → `process.cwd()`。这里把
// `getProjectDir` mock 成临时工程根，替代 `process.chdir`：Stryker 的 vitest runner
// 强制 worker 线程池，worker 中 `process.chdir` 会抛
// ERR_WORKER_UNSUPPORTED_OPERATION（与 run-static-analysis.test.ts 相同的处理）。
// 环境变量隔离仍然保留：被测命令之外的真实工程根不得介入。
vi.mock('../../src/lib/project-root');

const CHANGE = 'demo-change';
const VALID_ITEMS = [{ item: '检查项', pass: true, evidence: 'ok' }];
const FAILED_ITEMS = [{ item: '检查项', pass: false, evidence: 'broken' }];

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
  projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'eval-persist-'));
  changeDir = path.join(projectRoot, 'openspec', 'changes', CHANGE);
  fs.mkdirSync(changeDir, { recursive: true });
  // `getWorkflowType` 经 getProjectDir() 解析变更目录；把它指向临时工程根。
  vi.mocked(getProjectDir).mockReturnValue(projectRoot);
});

afterEach(() => {
  restoreProjectEnv(savedProjectEnv);
  fs.rmSync(projectRoot, { recursive: true, force: true });
});

function writeWorkflow(doc: unknown): void {
  fs.writeFileSync(path.join(changeDir, 'workflow.json'), JSON.stringify(doc, null, 2), 'utf-8');
}

function writeRawWorkflow(raw: string): void {
  fs.writeFileSync(path.join(changeDir, 'workflow.json'), raw, 'utf-8');
}

function writeLegacy(entries: unknown): void {
  fs.writeFileSync(path.join(changeDir, 'eval.json'), JSON.stringify(entries), 'utf-8');
}

function readWorkflow(): Record<string, unknown> {
  return JSON.parse(fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8')) as Record<
    string,
    unknown
  >;
}

function readEval(): EvalEntry[] {
  return readWorkflow().eval as EvalEntry[];
}

function legacyExists(): boolean {
  return fs.existsSync(path.join(changeDir, 'eval.json'));
}

function makeEntry(
  overrides: Partial<EvalEntry> & Pick<EvalEntry, 'phase' | 'verdict'>,
): EvalEntry {
  return {
    attempt: 1,
    timestamp: '2026-09-11T10:00:00.000Z',
    report: 'r',
    checklist: VALID_ITEMS,
    backtrack_to: null,
    ...overrides,
  };
}

/** 目录内的条目数（用于断言「未新增文件」）。 */
function dirEntries(): string[] {
  return fs.readdirSync(changeDir).sort();
}

// ---------------------------------------------------------------------------
// 场景: phase_log 追加到已有元数据文件
// ---------------------------------------------------------------------------

describe('phase_log 追加到已有元数据文件', () => {
  it('pass 追加后 workflow.json.eval[0].verdict 为 pass 且无 eval.json，元数据与未知键不变（AC-1、AC-2）', () => {
    writeWorkflow({ workflow_type: 'requirement', created: '2026-09-11', note: 'x' });

    const result = runPhaseLog({
      project_root: projectRoot,
      change: CHANGE,
      phase: 'proposal',
      report: '提案通过',
      checklist: VALID_ITEMS,
    });

    expect(result).toEqual({ written: true, phase: 'proposal', attempt: 1 });
    const doc = readWorkflow();
    expect(doc.workflow_type).toBe('requirement');
    expect(doc.created).toBe('2026-09-11');
    expect(doc.note).toBe('x');
    expect(readEval()).toHaveLength(1);
    expect(readEval()[0].verdict).toBe('pass');
    expect(legacyExists()).toBe(false);
  });

  it('同 phase 再 fail 一次后长度为 2，旧条目保留', () => {
    writeWorkflow({ workflow_type: 'requirement', created: '2026-09-11' });

    runPhaseLog({
      project_root: projectRoot,
      change: CHANGE,
      phase: 'proposal',
      report: '第一轮通过',
      checklist: VALID_ITEMS,
    });
    const second = runPhaseLog({
      project_root: projectRoot,
      change: CHANGE,
      phase: 'proposal',
      report: '第二轮失败',
      checklist: FAILED_ITEMS,
    });

    const entries = readEval();
    expect(entries).toHaveLength(2);
    expect(entries[0].verdict).toBe('pass');
    expect(entries[1].verdict).toBe('fail');
    expect(second.attempt).toBe(2);
  });

  it('预先写好的 eval 为对象时抛「读取 workflow.json 失败」，文件未被改写成合法数组', () => {
    writeWorkflow({ workflow_type: 'requirement', created: '2026-09-11', eval: {} });
    const before = fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8');

    expect(() =>
      runPhaseLog({
        project_root: projectRoot,
        change: CHANGE,
        phase: 'proposal',
        report: 'ok',
        checklist: VALID_ITEMS,
      }),
    ).toThrow(/读取 workflow\.json 失败/);

    expect(fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8')).toBe(before);
    expect(legacyExists()).toBe(false);
  });

  it('checklist 为空数组且未 skip 时仍追加一条 pass', () => {
    writeWorkflow({ workflow_type: 'requirement', created: '2026-09-11' });

    runPhaseLog({
      project_root: projectRoot,
      change: CHANGE,
      phase: 'proposal',
      report: '空 checklist',
      checklist: [],
    });

    expect(readEval()).toHaveLength(1);
    expect(readEval()[0].verdict).toBe('pass');
  });
});

// ---------------------------------------------------------------------------
// 场景: 双文件并存时写入迁移
// ---------------------------------------------------------------------------

describe('双文件并存时写入迁移', () => {
  it('phase_log 后遗留文件删除，新条目追加在 eval 数组末尾（AC-4）', () => {
    writeWorkflow({ workflow_type: 'requirement', created: '2026-09-11' });
    writeLegacy([
      makeEntry({ phase: 'proposal', verdict: 'pass', report: '遗留条目' }),
      makeEntry({ phase: 'dev-design', verdict: 'pass', report: '遗留条目 2' }),
    ]);

    runPhaseLog({
      project_root: projectRoot,
      change: CHANGE,
      phase: 'test-design',
      report: '新条目',
      checklist: VALID_ITEMS,
    });

    expect(legacyExists()).toBe(false);
    const reports = readEval().map((e) => e.report);
    expect(reports).toEqual(['遗留条目', '遗留条目 2', '新条目']);
  });

  it('backtrack 后遗留文件删除，eval 长度不变且最新条目含 backtrack_to（AC-4）', () => {
    writeWorkflow({
      workflow_type: 'requirement',
      created: '2026-09-11',
      files: { written: [], deleted: [] },
    });
    writeLegacy([
      makeEntry({ phase: 'proposal', verdict: 'pass' }),
      makeEntry({ phase: 'dev-design', verdict: 'fail' }),
    ]);

    const result = runBacktrack({
      project_root: projectRoot,
      change: CHANGE,
      phase: 'dev-design',
      backtrack_to: 'proposal',
      backtrack_reason: '设计不充分',
    });

    expect(result).toEqual({ modified: true, phase: 'dev-design', target: 'proposal' });
    expect(legacyExists()).toBe(false);
    const entries = readEval();
    expect(entries).toHaveLength(2);
    const modified = entries.find((e) => e.phase === 'dev-design')!;
    expect(modified.backtrack_to).toBe('proposal');
    expect(modified.backtrack_reason).toBe('设计不充分');
    // 回退目标的最新 pass 被标记 stale
    expect(entries.find((e) => e.phase === 'proposal')!.stale).toBe(true);
  });

  it('workflow.json 改成非法 JSON 再 backtrack 时抛错，eval.json 仍在，workflow.json 内容未改写', () => {
    const broken = '{ "workflow_type": "requirement" ';
    writeRawWorkflow(broken);
    writeLegacy([makeEntry({ phase: 'proposal', verdict: 'pass' })]);

    expect(() =>
      runBacktrack({
        project_root: projectRoot,
        change: CHANGE,
        phase: 'proposal',
        backtrack_to: 'proposal',
        backtrack_reason: 'r',
      }),
    ).toThrow();

    expect(fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8')).toBe(broken);
    expect(legacyExists()).toBe(true);
  });

  it('权威已是 [] 时 phase_log 只写入新条目，不把旧 eval.json 内容合并进来（AC-5）', () => {
    writeWorkflow({ workflow_type: 'requirement', created: '2026-09-11', eval: [] });
    writeLegacy([
      makeEntry({ phase: 'proposal', verdict: 'pass', report: '旧遗留 1' }),
      makeEntry({ phase: 'dev-design', verdict: 'pass', report: '旧遗留 2' }),
    ]);

    runPhaseLog({
      project_root: projectRoot,
      change: CHANGE,
      phase: 'proposal',
      report: '唯一新条目',
      checklist: VALID_ITEMS,
    });

    const entries = readEval();
    expect(entries).toHaveLength(1);
    expect(entries[0].report).toBe('唯一新条目');
    expect(legacyExists()).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// 场景: 缺 workflow.json 时写入失败且不补建
// ---------------------------------------------------------------------------

describe('缺 workflow.json 时写入失败且不补建', () => {
  it('runPhaseLog 抛「写入 workflow.json 失败」且内层含 change_create 指引，目录条目数不变（AC-13）', () => {
    const before = dirEntries();

    let captured: Error | null = null;
    try {
      runPhaseLog({
        project_root: projectRoot,
        change: CHANGE,
        phase: 'proposal',
        report: 'ok',
        checklist: VALID_ITEMS,
      });
    } catch (e: unknown) {
      captured = e as Error;
    }

    expect(captured).not.toBeNull();
    expect(captured!.message).toContain('写入 workflow.json 失败');
    expect(captured!.message).toContain('change_create');
    expect(dirEntries()).toEqual(before);
  });

  it('runBacktrack 抛错且不创建文件（AC-13）', () => {
    const before = dirEntries();

    expect(() =>
      runBacktrack({
        project_root: projectRoot,
        change: CHANGE,
        phase: 'proposal',
        backtrack_to: 'proposal',
        backtrack_reason: 'r',
      }),
    ).toThrow(/workflow\.json 不存在/);

    expect(dirEntries()).toEqual(before);
  });

  it('目录本身不存在（未 mkdir）时抛错且不创建该目录（AC-13）', () => {
    const ghostName = 'ghost-change';
    const ghostDir = path.join(projectRoot, 'openspec', 'changes', ghostName);

    expect(() =>
      runPhaseLog({
        project_root: projectRoot,
        change: ghostName,
        phase: 'proposal',
        report: 'ok',
        checklist: VALID_ITEMS,
      }),
    ).toThrow(/写入 workflow\.json 失败/);

    expect(fs.existsSync(ghostDir)).toBe(false);
  });
});
