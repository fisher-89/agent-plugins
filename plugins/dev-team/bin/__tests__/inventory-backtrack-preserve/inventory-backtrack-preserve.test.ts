/**
 * 集成测试: backtrack → workflow.json.file_log 保持不变（清单中立）
 *
 * 在真实临时工程上调用 runBacktrack（eval-json / file-inventory / phase 表全部
 * 真实实现，不 mock），验证清单中立语义：
 * - 任意回溯目标（implement / proposal / test-gen）→ file_log 逐条不变（含 scope/attempt）；
 * - file_log 缺失的机制前旧 change → 回溯成功（backtrack 不消费清单，不静默补建）；
 * - test-only 工作流（phase 表无 implement）→ 回溯成功且清单不变；
 * - 回溯后磁盘上的 file_log 逐条原样（记录器在原 log 上继续追加的前置）。
 *
 * @see openspec/specs/workflow-file-inventory/spec.md —「backtrack 保持清单，遗留由 redo 轮对冲」
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { runBacktrack } from '../../src/commands/backtrack';
import { type EvalEntry } from '../../src/lib/eval-json';

const CHANGE = 'inventory-change';

// runBacktrack 的 project_root 显式传入；但 getWorkflowType(change) 内部经
// getProjectDir() 解析工程根，优先级为 call-scoped → CLAUDE_PROJECT_DIR →
// WORKSPACE_FOLDER_PATHS → process.cwd()。这里通过 CLAUDE_PROJECT_DIR 把真实
// 解析链指向临时工程根：不 mock 任何模块，也不用 process.chdir（Stryker 的
// vitest runner 强制 worker 线程池，worker 中 process.chdir 会抛
// ERR_WORKER_UNSUPPORTED_OPERATION，与 eval-store-persist.test.ts 相同的约束）。
const ENV_KEYS = ['CLAUDE_PROJECT_DIR', 'WORKSPACE_FOLDER_PATHS'] as const;

let projectRoot: string;
let changeDir: string;
let savedEnv: Record<(typeof ENV_KEYS)[number], string | undefined>;

beforeEach(() => {
  savedEnv = {
    CLAUDE_PROJECT_DIR: process.env.CLAUDE_PROJECT_DIR,
    WORKSPACE_FOLDER_PATHS: process.env.WORKSPACE_FOLDER_PATHS,
  };
  projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'inventory-backtrack-preserve-'));
  changeDir = path.join(projectRoot, 'openspec', 'changes', CHANGE);
  fs.mkdirSync(changeDir, { recursive: true });
  vi.stubEnv('CLAUDE_PROJECT_DIR', projectRoot);
  vi.stubEnv('WORKSPACE_FOLDER_PATHS', '');
});

afterEach(() => {
  vi.unstubAllEnvs();
  for (const key of ENV_KEYS) {
    if (savedEnv[key] === undefined) {
      delete process.env[key];
    } else {
      process.env[key] = savedEnv[key];
    }
  }
  fs.rmSync(projectRoot, { recursive: true, force: true });
});

/** 混合 phase / workflow scope 的 file_log fixture（含 phase 审计条目）。 */
const FILE_LOG = [
  { op: 'write', scope: 'implement', attempt: 1, path: 'src/a.ts', at: '2026-09-17T00:00:00.000Z' },
  { op: 'delete', scope: 'workflow', path: 'src/b.ts', at: '2026-09-17T00:00:00.000Z' },
];

function makeEntry(
  overrides: Partial<EvalEntry> & Pick<EvalEntry, 'phase' | 'verdict'>,
): EvalEntry {
  return {
    attempt: 1,
    timestamp: '2026-09-17T00:00:00.000Z',
    report: 'r',
    checklist: [{ item: 'i', pass: true, evidence: 'e' }],
    backtrack_to: null,
    ...overrides,
  };
}

/** requirement 工作流的三条条目：proposal pass、implement pass、test-execution fail。 */
function requirementEntries(): EvalEntry[] {
  return [
    makeEntry({ phase: 'proposal', verdict: 'pass', timestamp: '2026-09-17T00:00:01.000Z' }),
    makeEntry({ phase: 'implement', verdict: 'pass', timestamp: '2026-09-17T00:00:03.000Z' }),
    makeEntry({ phase: 'test-execution', verdict: 'fail', timestamp: '2026-09-17T00:00:05.000Z' }),
  ];
}

function writeWorkflowJson(options: {
  workflowType: string;
  entries: EvalEntry[];
  fileLog?: unknown[];
}): void {
  const doc: Record<string, unknown> = {
    workflow_type: options.workflowType,
    created: '2026-09-17',
    ...(options.fileLog !== undefined ? { file_log: options.fileLog } : {}),
    eval: options.entries,
  };
  fs.writeFileSync(
    path.join(changeDir, 'workflow.json'),
    `${JSON.stringify(doc, null, 2)}\n`,
    'utf-8',
  );
}

function readWorkflowJson(): Record<string, unknown> {
  return JSON.parse(fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8')) as Record<
    string,
    unknown
  >;
}

function readEval(doc: Record<string, unknown>): Array<Record<string, unknown>> {
  return doc.eval as Array<Record<string, unknown>>;
}

function entryOfPhase(doc: Record<string, unknown>, phase: string): Record<string, unknown> {
  const found = readEval(doc).find((e) => e.phase === phase);
  expect(found).toBeDefined();
  return found!;
}

function backtrackTo(target: string): Parameters<typeof runBacktrack>[0] {
  return {
    project_root: projectRoot,
    change: CHANGE,
    phase: 'test-execution',
    backtrack_to: target,
    backtrack_reason: `回溯到 ${target}`,
  };
}

describe('backtrack → workflow.json.file_log 保持不变（清单中立）', () => {
  it('正向：回溯目标恰为 implement（等号语义）→ eval 条目写入 backtrack_to 且 implement 标 stale，file_log 逐条不变（含 scope/attempt）', () => {
    writeWorkflowJson({
      workflowType: 'requirement',
      fileLog: FILE_LOG,
      entries: requirementEntries(),
    });

    const result = runBacktrack(backtrackTo('implement'));

    expect(result).toEqual({ modified: true, phase: 'test-execution', target: 'implement' });
    const doc = readWorkflowJson();
    const execution = entryOfPhase(doc, 'test-execution');
    expect(execution.backtrack_to).toBe('implement');
    expect(execution.backtrack_reason).toBe('回溯到 implement');
    // 下游（含被回溯的 test-execution 本身）随 implement 传播 stale
    expect(execution.stale).toBe(true);
    expect(entryOfPhase(doc, 'implement').stale).toBe(true);
    expect(doc.file_log).toEqual(FILE_LOG);
  });

  it('正向：回溯到 implement 之前的 phase（proposal）→ file_log 同样逐条不变', () => {
    writeWorkflowJson({
      workflowType: 'requirement',
      fileLog: FILE_LOG,
      entries: requirementEntries(),
    });

    const result = runBacktrack(backtrackTo('proposal'));

    expect(result).toEqual({ modified: true, phase: 'test-execution', target: 'proposal' });
    const doc = readWorkflowJson();
    expect(entryOfPhase(doc, 'proposal').stale).toBe(true);
    expect(doc.file_log).toEqual(FILE_LOG);
  });

  it('正向：回溯到 implement 之后的 phase（test-gen）→ file_log 保持原内容不清空', () => {
    writeWorkflowJson({
      workflowType: 'requirement',
      fileLog: FILE_LOG,
      entries: requirementEntries(),
    });

    const result = runBacktrack(backtrackTo('test-gen'));

    expect(result).toEqual({ modified: true, phase: 'test-execution', target: 'test-gen' });
    const doc = readWorkflowJson();
    expect(entryOfPhase(doc, 'test-execution').backtrack_to).toBe('test-gen');
    expect(doc.file_log).toEqual(FILE_LOG);
  });

  it('边界：机制前旧 change（无 file_log）→ 回溯成功（不消费清单），eval 正常改写且 MUST NOT 静默补建 file_log', () => {
    writeWorkflowJson({ workflowType: 'requirement', entries: requirementEntries() });

    const result = runBacktrack(backtrackTo('implement'));

    expect(result).toEqual({ modified: true, phase: 'test-execution', target: 'implement' });
    const doc = readWorkflowJson();
    expect(entryOfPhase(doc, 'test-execution').backtrack_to).toBe('implement');
    expect(Object.hasOwn(doc, 'file_log')).toBe(false);
  });

  it('边界：test-only 工作流（phase 表无 implement）回溯成功且 file_log 不变', () => {
    const fileLog = [
      { op: 'write', scope: 'workflow', path: 'src/gen.ts', at: '2026-09-17T00:00:00.000Z' },
    ];
    writeWorkflowJson({
      workflowType: 'test-only',
      fileLog,
      entries: [
        makeEntry({
          phase: 'code-analyze',
          verdict: 'pass',
          timestamp: '2026-09-17T00:00:01.000Z',
        }),
        makeEntry({
          phase: 'test-execution',
          verdict: 'fail',
          timestamp: '2026-09-17T00:00:02.000Z',
        }),
      ],
    });

    const result = runBacktrack(backtrackTo('test-gen'));

    expect(result).toEqual({ modified: true, phase: 'test-execution', target: 'test-gen' });
    const doc = readWorkflowJson();
    expect(entryOfPhase(doc, 'test-execution').backtrack_to).toBe('test-gen');
    expect(doc.file_log).toEqual(fileLog);
  });

  it('正向：回溯后磁盘 file_log 重读 → 逐条原样（记录器在原 log 上继续追加的前置通路）', () => {
    writeWorkflowJson({
      workflowType: 'requirement',
      fileLog: FILE_LOG,
      entries: requirementEntries(),
    });

    runBacktrack(backtrackTo('implement'));

    expect(readWorkflowJson().file_log).toEqual(FILE_LOG);
  });
});
