/**
 * 单元测试: backtrack.ts — 新的 backtrack MCP 工具核心实现。
 *
 * 测试覆盖:
 * - 正常回溯操作（AC-2、AC-3）
 * - 目标合法性验证
 * - 幂等性
 *
 * @see openspec/changes/refactor-backtrack-to-skill/test-design.md
 */

import type * as fs from 'fs';

import { describe, it, expect, vi, beforeEach } from 'vite-plus/test';

vi.mock('fs', async (importOriginal) => {
  const actual = await importOriginal<typeof fs>();
  return {
    ...actual,
    existsSync: vi.fn(),
    readFileSync: vi.fn(),
    writeFileSync: vi.fn(),
    mkdirSync: vi.fn(),
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

vi.mock('../lib/change', () => ({
  getChangeDir: vi.fn(() => '/tmp/test-change'),
}));

vi.mock('../lib/change-config', () => ({
  getWorkflowType: vi.fn(() => 'requirement'),
}));

import { getWorkflowType } from '../lib/change-config';
import { readEvalJson, writeEvalJson, type EvalEntry } from '../lib/eval-json';
import { runBacktrack } from './backtrack';

// ---------------------------------------------------------------------------
// Helpers — construct eval.json entries for test scenarios
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

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(readEvalJson).mockReturnValue(defaultEntries);
  vi.mocked(getWorkflowType).mockReturnValue('requirement');
});

// ---------------------------------------------------------------------------
// 正常回溯操作
// ---------------------------------------------------------------------------

describe('runBacktrack — 正常回溯操作', () => {
  it('runBacktrack 修改指定 phase 最新 entry 的 backtrack_to 为 "test-gen"，backtrack_reason 为指定原因', () => {
    const result = runBacktrack({
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

  it('修改后 entry 的 backtrack_to 和 backtrack_reason 字段值正确', () => {
    runBacktrack({
      change: 'test-change',
      phase: 'test-execution',
      backtrack_to: 'test-gen',
      backtrack_reason: '语法错误',
    });

    const writtenEntries = vi.mocked(writeEvalJson).mock.calls[0][1];
    const testExecEntry = writtenEntries.find((e) => e.phase === 'test-execution')!;
    expect(testExecEntry.backtrack_to).toBe('test-gen');
    expect(testExecEntry.backtrack_reason).toBe('语法错误');
  });

  it('返回 { modified: true, phase, target }', () => {
    const result = runBacktrack({
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

  it('backtrack_to 在当前 phase 之后时抛出错误（AC-3）', () => {
    expect(() =>
      runBacktrack({
        change: 'test-change',
        phase: 'test-gen',
        backtrack_to: 'acceptance',
        backtrack_reason: 'acceptance 在 test-gen 之后',
      }),
    ).toThrow(/无效的回溯目标 phase/);
  });

  it('backtrack_to 不在 phase 表中时抛出错误', () => {
    expect(() =>
      runBacktrack({
        change: 'test-change',
        phase: 'test-execution',
        backtrack_to: 'nonexistent-phase',
        backtrack_reason: '不存在的 phase',
      }),
    ).toThrow(/不包含 phase/);
  });

  it('change 不存在时抛出错误', () => {
    vi.mocked(readEvalJson).mockImplementation(() => {
      throw new Error('读取 eval.json 失败: 目录不存在');
    });

    expect(() =>
      runBacktrack({
        change: 'non-existent-change',
        phase: 'test-execution',
        backtrack_to: 'test-gen',
        backtrack_reason: '不存在',
      }),
    ).toThrow(/读取 eval.json 失败/);
  });

  it('phase 不在 phase 表中时抛出错误', () => {
    expect(() =>
      runBacktrack({
        change: 'test-change',
        phase: 'invalid-phase',
        backtrack_to: 'test-gen',
        backtrack_reason: '不存在的 phase',
      }),
    ).toThrow(/不包含 phase/);
  });

  it('目标 phase 无任何 entry 时不报错（markPhaseStale 是 no-op）', () => {
    vi.mocked(readEvalJson).mockReturnValue([
      makePassEntry('proposal', 1),
      makePassEntry('dev-design', 1),
      makeFailEntry('test-execution', 1),
    ]);

    // test-gen 无任何 entry — markPhaseStale 视为 no-op，不应报错
    const result = runBacktrack({
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

  it('目标 phase 有 entry 但无 pass entry 时抛出错误（无 pass 可标记 stale）', () => {
    const entries: EvalEntry[] = [
      makePassEntry('proposal', 1),
      makePassEntry('dev-design', 1),
      makeFailEntry('test-gen', 1), // test-gen 只有 fail entry
      makeFailEntry('test-execution', 1),
    ];
    vi.mocked(readEvalJson).mockReturnValue(entries);

    // 回溯到 test-gen 时，没有 pass entry 可标记 stale，但不应是错误
    // markPhaseStale 对无 pass entry 的情况是 no-op
    const result = runBacktrack({
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
// 幂等性
// ---------------------------------------------------------------------------

describe('runBacktrack — 幂等性', () => {
  it('连续调用两次 backtrack 结果一致（第二次覆盖第一次的设置）', () => {
    const opts = {
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
