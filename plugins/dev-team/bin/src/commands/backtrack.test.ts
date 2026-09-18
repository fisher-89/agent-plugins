/**
 * 单元测试: backtrack.ts — backtrack MCP 工具核心实现。
 *
 * 测试覆盖:
 * - 正常回溯操作（AC-2、AC-3）
 * - 目标合法性验证
 * - 严格前置条件（缺文件 / 格式非法即终止）
 * - 幂等性
 * - 文件清单中立（backtrack 不读不写 `workflow.json.files`）
 *
 * 评估历史存放在 `workflow.json.eval`；本命令只经 `writeEvalJson` 持久化，
 * 从不直接写 `eval.json`。
 */

import * as fs from 'fs';

import { describe, it, expect, vi, beforeEach } from 'vite-plus/test';

vi.mock('fs', async (importOriginal) => {
  const actual = await importOriginal<typeof fs>();
  return {
    ...actual,
    existsSync: vi.fn(),
    readFileSync: vi.fn(),
    writeFileSync: vi.fn(),
    mkdirSync: vi.fn(),
    unlinkSync: vi.fn(),
  };
});

vi.mock('../lib/eval-json', async () => {
  const actual = await vi.importActual('../lib/eval-json');
  return {
    ...actual,
    readEvalJson: vi.fn(),
    writeEvalJson: vi.fn(),
  };
});

vi.mock('../modules/workflow', async () => {
  const actual = await vi.importActual('../modules/workflow');
  return {
    ...actual,
    // 回归哨兵：barrel 的全部运行时导出（读通道 readFileInventory / getChangedFiles，
    // 写通道 appendFileOps / recordFileOps / setFileBuckets）都替换为 mock，
    // 下方断言捕获 backtrack 重新引入的清单调用
    appendFileOps: vi.fn(),
    getChangedFiles: vi.fn(),
    readFileInventory: vi.fn(),
    recordFileOps: vi.fn(),
    setFileBuckets: vi.fn(),
  };
});

vi.mock('../lib/change', () => ({
  resolveChangeDir: vi.fn(() => '/tmp/test-change'),
}));

vi.mock('../lib/change-config', () => ({
  getWorkflowType: vi.fn(() => 'requirement'),
}));

import { getWorkflowType } from '../lib/change-config';
import { readEvalJson, writeEvalJson, type EvalEntry } from '../lib/eval-json';
import {
  appendFileOps,
  getChangedFiles,
  readFileInventory,
  recordFileOps,
  setFileBuckets,
} from '../modules/workflow';
import { backtrackInputSchema } from '../schemas';
import { runBacktrack } from './backtrack';

const FIXTURE_PROJECT_ROOT = '/tmp/fixture-project';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makePassEntry(
  phase: EvalEntry['phase'],
  attempt: number = 1,
  overrides: Partial<EvalEntry> = {},
): EvalEntry {
  return {
    phase,
    verdict: 'pass',
    attempt,
    timestamp: new Date(Date.now() + attempt).toISOString(),
    backtrack_to: null,
    report: '',
    checklist: [],
    ...overrides,
  };
}

function makeFailEntry(
  phase: EvalEntry['phase'],
  attempt: number = 1,
  overrides: Partial<EvalEntry> = {},
): EvalEntry {
  return {
    phase,
    verdict: 'fail',
    attempt,
    timestamp: new Date(Date.now() + attempt).toISOString(),
    backtrack_to: null,
    report: '',
    checklist: [],
    ...overrides,
  };
}

const defaultEntries: EvalEntry[] = [
  makePassEntry('proposal', 1),
  makePassEntry('dev-design', 1),
  makePassEntry('test-design', 1),
  makePassEntry('test-gen', 1),
  makePassEntry('implement', 1),
  makeFailEntry('test-execution', 1),
];

/** 断言本命令没有直接写 / 删磁盘上的 `eval.json`（迁移由 writeEvalJson 负责）。 */
function expectNoLegacyWrite(): void {
  for (const call of vi.mocked(fs.writeFileSync).mock.calls) {
    expect(String(call[0]).endsWith('eval.json')).toBe(false);
  }
  expect(vi.mocked(fs.unlinkSync)).not.toHaveBeenCalled();
}

/** 断言 backtrack 全程未经 workflow barrel 触碰 files 清单（读 / 写通道零调用）。 */
function expectNoInventoryAccess(): void {
  expect(readFileInventory).not.toHaveBeenCalled();
  expect(getChangedFiles).not.toHaveBeenCalled();
  expect(appendFileOps).not.toHaveBeenCalled();
  expect(recordFileOps).not.toHaveBeenCalled();
  expect(setFileBuckets).not.toHaveBeenCalled();
}

/** 清空清单哨兵 mock 的调用记录（保留 beforeEach 铺设的返回值）。 */
function clearInventoryMocks(): void {
  vi.mocked(readFileInventory).mockClear();
  vi.mocked(getChangedFiles).mockClear();
  vi.mocked(appendFileOps).mockClear();
  vi.mocked(recordFileOps).mockClear();
  vi.mocked(setFileBuckets).mockClear();
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(readEvalJson).mockReset().mockReturnValue(defaultEntries);
  vi.mocked(writeEvalJson).mockReset();
  vi.mocked(getWorkflowType).mockReset().mockReturnValue('requirement');
  // files 清单 mock 仅作回归哨兵：backtrack 不消费清单，下方断言捕获重新引入的调用
  vi.mocked(readFileInventory).mockReset().mockReturnValue({ written: [], deleted: [] });
  vi.mocked(getChangedFiles).mockReset().mockReturnValue({ written: [], deleted: [] });
  vi.mocked(appendFileOps).mockReset().mockReturnValue({ written: [], deleted: [] });
  vi.mocked(recordFileOps).mockReset();
  vi.mocked(setFileBuckets).mockReset().mockReturnValue({ written: [], deleted: [] });
});

// ---------------------------------------------------------------------------
// 正常回溯操作
// ---------------------------------------------------------------------------

describe('runBacktrack — 正常回溯操作', () => {
  it('runBacktrack 修改指定 phase 最新 entry 的 backtrack_to 为 "test-gen"，backtrack_reason 为指定原因', () => {
    const result = runBacktrack({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'test-execution',
      backtrack_to: 'test-gen',
      backtrack_reason: '语法错误',
    });

    expect(result.modified).toBe(true);
    expect(writeEvalJson).toHaveBeenCalledTimes(1);

    const writtenEntries = vi.mocked(writeEvalJson).mock.calls[0][1];
    const testExecEntry = writtenEntries.find((e) => e.phase === 'test-execution')!;
    expect(testExecEntry.backtrack_to).toBe('test-gen');
    expect(testExecEntry.backtrack_reason).toBe('语法错误');
  });

  it('调用 writeEvalJson 一次且数组长度不变；不出现对 eval.json 的直接写入（AC-4）', () => {
    runBacktrack({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'test-execution',
      backtrack_to: 'test-gen',
      backtrack_reason: '语法错误',
    });

    expect(writeEvalJson).toHaveBeenCalledTimes(1);
    expect(vi.mocked(writeEvalJson).mock.calls[0][1]).toHaveLength(defaultEntries.length);
    expectNoLegacyWrite();
  });

  it('返回 { modified: true, phase, target }', () => {
    const result = runBacktrack({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'test-execution',
      backtrack_to: 'test-gen',
      backtrack_reason: '语法错误',
    });

    expect(result).toEqual({
      modified: true,
      phase: 'test-execution',
      target: 'test-gen',
    });
  });

  it('标记目标 phase test-gen 的最新 pass entry 为 stale: true', () => {
    runBacktrack({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'test-execution',
      backtrack_to: 'test-gen',
      backtrack_reason: '语法错误',
    });

    const writtenEntries = vi.mocked(writeEvalJson).mock.calls[0][1];
    const testGenEntry = writtenEntries.find((e) => e.phase === 'test-gen')!;
    expect(testGenEntry.stale).toBe(true);
  });

  it('stale 标记向下游传播到 test-execution 等依赖 phase', () => {
    const entries: EvalEntry[] = [
      makePassEntry('proposal', 1),
      makePassEntry('dev-design', 1),
      makePassEntry('test-design', 1),
      makePassEntry('test-gen', 1),
      makePassEntry('implement', 1),
      makePassEntry('test-execution', 1),
      makePassEntry('code-review', 1),
      makePassEntry('acceptance', 1),
    ];
    vi.mocked(readEvalJson).mockReturnValue(entries);

    runBacktrack({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'test-execution',
      backtrack_to: 'test-gen',
      backtrack_reason: '语法错误',
    });

    const writtenEntries = vi.mocked(writeEvalJson).mock.calls[0][1];

    // test-gen 被标记 stale
    expect(writtenEntries.find((e) => e.phase === 'test-gen')!.stale).toBe(true);
    // test-gen 的 downstream: test-execution, code-review 也应被标记 stale
    expect(writtenEntries.find((e) => e.phase === 'test-execution')!.stale).toBe(true);
    expect(writtenEntries.find((e) => e.phase === 'code-review')!.stale).toBe(true);
    // 非 downstream 不应被标记
    expect(writtenEntries.find((e) => e.phase === 'proposal')!.stale).toBeUndefined();
    expect(writtenEntries.find((e) => e.phase === 'dev-design')!.stale).toBeUndefined();
    expect(writtenEntries.find((e) => e.phase === 'implement')!.stale).toBeUndefined();
  });

  it('多 workflow 类型（requirement、test-only）均正常工作', () => {
    // requirement 已由默认 mock 覆盖，测试 test-only
    vi.mocked(getWorkflowType).mockReturnValue('test-only');

    const entries: EvalEntry[] = [
      makePassEntry('proposal', 1),
      makePassEntry('code-analyze', 1),
      makePassEntry('test-design', 1),
      makePassEntry('test-gen', 1),
      makeFailEntry('test-execution', 1),
    ];
    vi.mocked(readEvalJson).mockReturnValue(entries);

    const result = runBacktrack({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'test-execution',
      backtrack_to: 'test-gen',
      backtrack_reason: '测试失败需回溯',
    });

    expect(result.modified).toBe(true);
    expect(result.target).toBe('test-gen');

    const writtenEntries = vi.mocked(writeEvalJson).mock.calls[0][1];
    expect(writtenEntries.find((e) => e.phase === 'test-execution')!.backtrack_to).toBe('test-gen');
    expect(writtenEntries.find((e) => e.phase === 'test-gen')!.stale).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// 目标合法性验证
// ---------------------------------------------------------------------------

describe('runBacktrack — 目标合法性验证', () => {
  it('backtrack_to 等于当前 phase 时允许回溯（重新执行当前 phase）', () => {
    const entries: EvalEntry[] = [
      makePassEntry('proposal', 1),
      makePassEntry('dev-design', 1),
      makePassEntry('test-design', 1),
      makePassEntry('test-gen', 1),
      makePassEntry('implement', 1),
      makeFailEntry('test-execution', 1),
    ];
    vi.mocked(readEvalJson).mockReturnValue(entries);

    const result = runBacktrack({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'test-execution',
      backtrack_to: 'test-execution',
      backtrack_reason: '重新执行当前 phase',
    });

    expect(result.modified).toBe(true);
    expect(result.target).toBe('test-execution');

    const writtenEntries = vi.mocked(writeEvalJson).mock.calls[0][1];
    const testExecEntry = writtenEntries.find((e) => e.phase === 'test-execution')!;
    expect(testExecEntry.backtrack_to).toBe('test-execution');
    expect(testExecEntry.backtrack_reason).toBe('重新执行当前 phase');
  });

  it('backtrack_to 在当前 phase 之后时抛出错误且不写盘（AC-3）', () => {
    expect(() =>
      runBacktrack({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'test-gen',
        backtrack_to: 'acceptance',
        backtrack_reason: 'acceptance 在 test-gen 之后',
      }),
    ).toThrow(/无效的回溯目标 phase/);

    expect(writeEvalJson).not.toHaveBeenCalled();
  });

  it('backtrack_to 不在 phase 表中时抛出错误且不写盘', () => {
    expect(() =>
      runBacktrack({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'test-execution',
        backtrack_to: 'nonexistent-phase',
        backtrack_reason: '不存在的 phase',
      }),
    ).toThrow(/不包含 phase/);

    expect(writeEvalJson).not.toHaveBeenCalled();
  });

  it('phase 不在 phase 表中时抛出错误且不写盘', () => {
    expect(() =>
      runBacktrack({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'invalid-phase',
        backtrack_to: 'test-gen',
        backtrack_reason: '不存在的 phase',
      }),
    ).toThrow(/不包含 phase/);

    expect(writeEvalJson).not.toHaveBeenCalled();
  });

  it('该 phase 无任何评估条目时抛错，message 含「没有评估条目」且不含 eval.json（AC-4）', () => {
    vi.mocked(readEvalJson).mockReturnValue([makePassEntry('proposal', 1)]);

    let captured: Error | null = null;
    try {
      runBacktrack({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'test-execution',
        backtrack_to: 'proposal',
        backtrack_reason: 'no entry',
      });
    } catch (e: unknown) {
      captured = e as Error;
    }

    expect(captured).not.toBeNull();
    expect(captured!.message).toContain('没有评估条目');
    expect(captured!.message).not.toContain('eval.json');
    expect(writeEvalJson).not.toHaveBeenCalled();
  });

  it('目标 phase 无任何 entry 时不报错（markPhaseStale 是 no-op）', () => {
    vi.mocked(readEvalJson).mockReturnValue([
      makePassEntry('proposal', 1),
      makePassEntry('dev-design', 1),
      makeFailEntry('test-execution', 1),
    ]);

    // test-gen 无任何 entry — markPhaseStale 视为 no-op，不应报错
    const result = runBacktrack({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'test-execution',
      backtrack_to: 'test-gen',
      backtrack_reason: '目标无 entry',
    });

    expect(result.modified).toBe(true);

    const writtenEntries = vi.mocked(writeEvalJson).mock.calls[0][1];
    const testExecEntry = writtenEntries.find((e) => e.phase === 'test-execution')!;
    expect(testExecEntry.backtrack_to).toBe('test-gen');
    expect(testExecEntry.backtrack_reason).toBe('目标无 entry');
  });

  it('目标 phase 有 entry 但无 pass entry 时 markPhaseStale 为 no-op，不抛错', () => {
    const entries: EvalEntry[] = [
      makePassEntry('proposal', 1),
      makePassEntry('dev-design', 1),
      makeFailEntry('test-gen', 1), // test-gen 只有 fail entry
      makeFailEntry('test-execution', 1),
    ];
    vi.mocked(readEvalJson).mockReturnValue(entries);

    const result = runBacktrack({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'test-execution',
      backtrack_to: 'test-gen',
      backtrack_reason: '语法错误',
    });

    expect(result.modified).toBe(true);
    const writtenEntries = vi.mocked(writeEvalJson).mock.calls[0][1];
    const testGenEntry = writtenEntries.find((e) => e.phase === 'test-gen')!;
    // markPhaseStale 是 no-op，但不应抛错
    expect(testGenEntry.stale).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// 严格前置条件与错误包装
// ---------------------------------------------------------------------------

describe('runBacktrack — 严格前置条件 (AC-13, AC-14)', () => {
  it('getWorkflowType 因 workflow.json 缺失抛错时原样透出，message 含 change_create 指引且不写盘（AC-13）', () => {
    vi.mocked(getWorkflowType).mockImplementation(() => {
      throw new Error(
        'workflow.json 不存在: /tmp/test-change/workflow.json。该文件由 change_create 建立，是工作流的前置条件；请通过 change_create 或 workflow-* skill 创建 change，不要手写该文件。',
      );
    });

    let captured: Error | null = null;
    try {
      runBacktrack({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'test-execution',
        backtrack_to: 'test-gen',
        backtrack_reason: '语法错误',
      });
    } catch (e: unknown) {
      captured = e as Error;
    }

    expect(captured).not.toBeNull();
    expect(captured!.message).toContain('workflow.json 不存在');
    expect(captured!.message).toContain('change_create');
    expect(writeEvalJson).not.toHaveBeenCalled();
    expect(readEvalJson).not.toHaveBeenCalled();
  });

  it('getWorkflowType 因 JSON 非法 / 根非对象 / workflow_type 非枚举抛错时原样透出，不写盘（AC-14）', () => {
    for (const message of [
      'workflow.json 解析失败: Unexpected token',
      'workflow.json 根元素必须是对象，但实际类型为 object',
      'workflow.json 格式非法 (/tmp/test-change/workflow.json): workflow_type: Invalid input',
    ]) {
      vi.mocked(getWorkflowType).mockImplementation(() => {
        throw new Error(message);
      });

      expect(() =>
        runBacktrack({
          project_root: FIXTURE_PROJECT_ROOT,
          change: 'test-change',
          phase: 'test-execution',
          backtrack_to: 'test-gen',
          backtrack_reason: '语法错误',
        }),
      ).toThrow(message);
      expect(writeEvalJson).not.toHaveBeenCalled();
    }
  });

  it('readEvalJson 抛错时包装为「读取 workflow.json 失败:」+ 原因', () => {
    vi.mocked(readEvalJson).mockImplementation(() => {
      throw new Error('workflow.json.eval 必须是数组，但实际类型为 object');
    });

    expect(() =>
      runBacktrack({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'test-execution',
        backtrack_to: 'test-gen',
        backtrack_reason: '语法错误',
      }),
    ).toThrow(/读取 workflow\.json 失败: workflow\.json\.eval 必须是数组/);

    expect(writeEvalJson).not.toHaveBeenCalled();
  });

  it('writeEvalJson 抛错时包装为「写入 workflow.json 失败:」+ 原因', () => {
    vi.mocked(writeEvalJson).mockImplementation(() => {
      throw new Error('workflow.json 格式非法 (/tmp/test-change/workflow.json): eval: 未知');
    });

    expect(() =>
      runBacktrack({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'test-execution',
        backtrack_to: 'test-gen',
        backtrack_reason: '语法错误',
      }),
    ).toThrow(/写入 workflow\.json 失败: workflow\.json 格式非法/);
  });

  it('writeEvalJson 因文件缺失拒绝时抛错且不创建任何文件（AC-13）', () => {
    vi.mocked(writeEvalJson).mockImplementation(() => {
      throw new Error(
        'workflow.json 不存在: /tmp/test-change/workflow.json，无法写入评估记录。请先通过 change_create 创建 change。',
      );
    });

    let captured: Error | null = null;
    try {
      runBacktrack({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'test-execution',
        backtrack_to: 'test-gen',
        backtrack_reason: '语法错误',
      });
    } catch (e: unknown) {
      captured = e as Error;
    }

    expect(captured).not.toBeNull();
    expect(captured!.message).toContain('写入 workflow.json 失败:');
    expect(captured!.message).toContain('change_create');
    expect(vi.mocked(fs.writeFileSync)).not.toHaveBeenCalled();
    expect(vi.mocked(fs.mkdirSync)).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// 输入边界（schema 与命令层）
// ---------------------------------------------------------------------------

describe('runBacktrack — 输入边界', () => {
  it('backtrack_reason 为空串时 backtrackInputSchema 接受，命令层原样写出', () => {
    expect(
      backtrackInputSchema.safeParse({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'test-execution',
        backtrack_to: 'test-gen',
        backtrack_reason: '',
      }).success,
    ).toBe(true);

    runBacktrack({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'test-execution',
      backtrack_to: 'test-gen',
      backtrack_reason: '',
    });

    const writtenEntries = vi.mocked(writeEvalJson).mock.calls[0][1];
    expect(writtenEntries.find((e) => e.phase === 'test-execution')!.backtrack_reason).toBe('');
  });

  it('backtrack_reason 恰 500 字符时 schema 接受；501 字符被 schema 拒绝', () => {
    const base = {
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'test-execution',
      backtrack_to: 'test-gen',
    };
    expect(
      backtrackInputSchema.safeParse({ ...base, backtrack_reason: 'R'.repeat(500) }).success,
    ).toBe(true);
    expect(
      backtrackInputSchema.safeParse({ ...base, backtrack_reason: 'R'.repeat(501) }).success,
    ).toBe(false);
  });

  it('backtrack_to 为 null / undefined 时 schema min(1) 拒绝且命令层不写盘', () => {
    for (const backtrackTo of [null, undefined]) {
      expect(
        backtrackInputSchema.safeParse({
          project_root: FIXTURE_PROJECT_ROOT,
          change: 'test-change',
          phase: 'test-execution',
          backtrack_to: backtrackTo,
          backtrack_reason: 'r',
        }).success,
      ).toBe(false);

      expect(() =>
        runBacktrack({
          project_root: FIXTURE_PROJECT_ROOT,
          change: 'test-change',
          phase: 'test-execution',
          backtrack_to: backtrackTo as unknown as string,
          backtrack_reason: 'r',
        }),
      ).toThrow(/不包含 phase/);
    }

    expect(writeEvalJson).not.toHaveBeenCalled();
  });

  it('phase 为空串时 schema min(1) 拒绝且命令层不写盘', () => {
    expect(
      backtrackInputSchema.safeParse({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: '',
        backtrack_to: 'test-gen',
        backtrack_reason: 'r',
      }).success,
    ).toBe(false);

    expect(() =>
      runBacktrack({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: '',
        backtrack_to: 'test-gen',
        backtrack_reason: 'r',
      }),
    ).toThrow(/不包含 phase/);

    expect(writeEvalJson).not.toHaveBeenCalled();
  });

  it('无条目错误文案不再匹配「没有 eval.json 条目」，读失败文案不再匹配「读取 eval.json 失败」', () => {
    vi.mocked(readEvalJson).mockReturnValue([makePassEntry('proposal', 1)]);
    expect(() =>
      runBacktrack({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'test-execution',
        backtrack_to: 'proposal',
        backtrack_reason: 'r',
      }),
    ).not.toThrow(/没有 eval\.json 条目/);

    vi.mocked(readEvalJson).mockImplementation(() => {
      throw new Error('eval.json 根元素必须是数组，但实际类型为 object');
    });
    expect(() =>
      runBacktrack({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'test-execution',
        backtrack_to: 'test-gen',
        backtrack_reason: 'r',
      }),
    ).not.toThrow(/读取 eval\.json 失败/);
  });

  it('缺 workflow.json 时不再被「写时自动补建」掩盖：getWorkflowType 抛错即终止（AC-13）', () => {
    vi.mocked(getWorkflowType).mockImplementation(() => {
      throw new Error('workflow.json 不存在: /tmp/test-change/workflow.json');
    });

    expect(() =>
      runBacktrack({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'test-execution',
        backtrack_to: 'test-gen',
        backtrack_reason: 'r',
      }),
    ).toThrow(/workflow\.json 不存在/);

    expect(writeEvalJson).not.toHaveBeenCalled();
    expect(vi.mocked(fs.writeFileSync)).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// 幂等性
// ---------------------------------------------------------------------------

describe('runBacktrack — 幂等性', () => {
  it('连续调用两次 backtrack 结果一致（第二次覆盖第一次的设置）', () => {
    const opts = {
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'test-execution',
      backtrack_to: 'test-gen',
      backtrack_reason: '第一次回溯原因',
    };

    const firstResult = runBacktrack(opts);
    const firstWritten = vi.mocked(writeEvalJson).mock.calls[0][1];

    // 重置 mock 以捕获第二次写入
    vi.mocked(writeEvalJson).mockClear();

    // 第二次调用，使用不同的原因
    const secondResult = runBacktrack({
      ...opts,
      backtrack_reason: '第二次回溯原因（覆盖）',
    });

    expect(firstResult.modified).toBe(true);
    expect(secondResult.modified).toBe(true);

    const secondWritten = vi.mocked(writeEvalJson).mock.calls[0][1];
    const secondExecEntry = secondWritten.find((e) => e.phase === 'test-execution')!;

    // 第二次的 reason 覆盖了第一次
    expect(secondExecEntry.backtrack_reason).toBe('第二次回溯原因（覆盖）');
    // 两次写入的条目总数一致
    expect(firstWritten.length).toBe(secondWritten.length);
  });
});

// ---------------------------------------------------------------------------
// 文件清单中立（backtrack 不读不写 files）
// ---------------------------------------------------------------------------

describe('runBacktrack — 文件清单中立（不读不写 files）', () => {
  it('backtrack 到 implement → 清单读写均不被调用，eval 正常改写', () => {
    runBacktrack({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'test-execution',
      backtrack_to: 'implement',
      backtrack_reason: '实现方案推翻',
    });

    expectNoInventoryAccess();
    expect(writeEvalJson).toHaveBeenCalledTimes(1);

    const written = vi.mocked(writeEvalJson).mock.calls[0][1];
    expect(written.length).toBe(defaultEntries.length);
    const execEntry = written.find((e) => e.phase === 'test-execution')!;
    expect(execEntry.backtrack_to).toBe('implement');
    expect(execEntry.backtrack_reason).toBe('实现方案推翻');
  });

  it('任意回溯目标（proposal / dev-design / test-gen / test-execution）→ 清单读写均不被调用', () => {
    for (const target of ['proposal', 'dev-design', 'test-gen', 'test-execution'] as const) {
      clearInventoryMocks();
      vi.mocked(writeEvalJson).mockClear();

      runBacktrack({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'test-execution',
        backtrack_to: target,
        backtrack_reason: `回溯到 ${target}`,
      });

      expectNoInventoryAccess();
      expect(writeEvalJson).toHaveBeenCalledTimes(1);
    }
  });

  it('目标 phase 恰为 implement（索引相等，等号语义仍合法）→ eval 改写正常且清单零调用', () => {
    const entries: EvalEntry[] = [
      makePassEntry('proposal', 1),
      makePassEntry('dev-design', 1),
      makePassEntry('implement', 1),
      makeFailEntry('implement', 2),
    ];
    vi.mocked(readEvalJson).mockReturnValue(entries);

    runBacktrack({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'implement',
      backtrack_to: 'implement',
      backtrack_reason: '重做实现',
    });

    expect(writeEvalJson).toHaveBeenCalledTimes(1);
    expectNoInventoryAccess();
  });

  it('机制前旧 change（清单读取即抛「请重建」）→ backtrack 不消费清单，回溯成功（行为中立，不再硬报错）', () => {
    vi.mocked(readFileInventory).mockImplementation(() => {
      throw new Error(
        'workflow.json 缺少 files 字段 (/tmp/test-change/workflow.json)：该 change 创建于文件清单机制之前，请重建该 change（change_create）。',
      );
    });

    const result = runBacktrack({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'test-execution',
      backtrack_to: 'implement',
      backtrack_reason: '旧 change',
    });

    expect(result.modified).toBe(true);
    expect(writeEvalJson).toHaveBeenCalledTimes(1);
  });

  it('test-only 工作流（phase 表无 implement）→ 回溯成功且清单零调用', () => {
    vi.mocked(getWorkflowType).mockReturnValue('test-only');
    const entries: EvalEntry[] = [
      makePassEntry('proposal', 1),
      makePassEntry('code-analyze', 1),
      makePassEntry('test-design', 1),
      makePassEntry('test-gen', 1),
      makeFailEntry('test-execution', 1),
    ];
    vi.mocked(readEvalJson).mockReturnValue(entries);

    runBacktrack({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'test-execution',
      backtrack_to: 'test-gen',
      backtrack_reason: '测试需重写',
    });

    expectNoInventoryAccess();
  });
});
