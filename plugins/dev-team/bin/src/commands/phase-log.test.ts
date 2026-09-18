/**
 * Tests for phase-log.ts — input validation and entry appending.
 *
 * The evaluation history lives in `workflow.json.eval`: `runPhaseLog` reads it
 * through `readEvalJson` and appends through `appendEntry`, which also deletes a
 * leftover legacy `eval.json`. `runPhaseLog` MUST NOT write `eval.json` itself.
 *
 * backtrack_to/backtrack_reason are not accepted by phase_log; backtrack state
 * is managed exclusively by the standalone backtrack tool.
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
  };
});

vi.mock('../lib/eval-json', async () => {
  const actual = await vi.importActual('../lib/eval-json');
  return {
    ...actual,
    readEvalJson: vi.fn(() => []),
    appendEntry: vi.fn(),
    writeEvalJson: vi.fn(),
  };
});

vi.mock('../lib/change', () => ({
  resolveChangeDir: vi.fn(() => '/tmp/test-change'),
}));

vi.mock('../modules/workflow', async () => {
  const actual = await vi.importActual('../modules/workflow');
  return {
    ...actual,
    readActivePhase: vi.fn(() => null),
    clearActivePhase: vi.fn(),
  };
});

import { appendEntry, readEvalJson, writeEvalJson } from '../lib/eval-json';
import { clearActivePhase, readActivePhase } from '../modules/workflow';
import { runPhaseLog } from './phase-log';

const FIXTURE_PROJECT_ROOT = '/tmp/fixture-project';
const VALID_ITEMS = [{ item: 'test', pass: true, evidence: 'ok' }];
const FAILED_ITEMS = [{ item: 'test', pass: false, evidence: 'ok' }];

/** Assert that nothing was written to a legacy `eval.json`. */
function expectNoLegacyWrite(): void {
  const writeCalls = vi.mocked(fs.writeFileSync).mock.calls;
  for (const call of writeCalls) {
    expect(String(call[0]).endsWith('eval.json')).toBe(false);
  }
}

beforeEach(() => {
  vi.clearAllMocks();
  // mockReset 复位实现，避免上一个用例的 mockImplementation 泄漏到下一个用例
  vi.mocked(readEvalJson).mockReset().mockReturnValue([]);
  vi.mocked(appendEntry).mockReset();
  vi.mocked(writeEvalJson).mockReset();
  vi.mocked(readActivePhase).mockReset().mockReturnValue(null);
  vi.mocked(clearActivePhase).mockReset();
});

// ===========================================================================
// 正向
// ===========================================================================

describe('runPhaseLog — 正向', () => {
  it('checklist 全 pass 时调用 appendEntry 一次，返回 { written, phase, attempt }，不调用 writeEvalJson（AC-1）', () => {
    const result = runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'proposal',
      report: 'ok',
      checklist: VALID_ITEMS,
    });

    expect(appendEntry).toHaveBeenCalledTimes(1);
    expect(writeEvalJson).not.toHaveBeenCalled();
    expect(result).toEqual({ written: true, phase: 'proposal', attempt: 1 });
  });

  it('不把 eval.json 作为 appendEntry 之外的写入目标', () => {
    runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'proposal',
      report: 'ok',
      checklist: VALID_ITEMS,
    });

    expect(appendEntry).toHaveBeenCalledTimes(1);
    expectNoLegacyWrite();
  });

  it('skipped=true 且 checklist 全 pass 时 entry 含 skipped:true 且 appendEntry 仍被调用一次', () => {
    runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'test-execution',
      report: 'No tests found',
      checklist: [],
      skipped: true,
    });

    expect(appendEntry).toHaveBeenCalledTimes(1);
    const entry = vi.mocked(appendEntry).mock.calls[0][1];
    expect(entry.skipped).toBe(true);
    expect(entry.verdict).toBe('pass');
  });

  it('同一 phase 已有一条 fail 时 attempt 为 2', () => {
    vi.mocked(readEvalJson).mockReturnValue([
      {
        phase: 'proposal',
        verdict: 'fail',
        attempt: 1,
        timestamp: '2026-09-11T10:00:00.000Z',
        report: '',
        checklist: [],
        backtrack_to: null,
      },
    ]);

    const result = runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'proposal',
      report: 'retry',
      checklist: VALID_ITEMS,
    });

    expect(result.attempt).toBe(2);
    expect(vi.mocked(appendEntry).mock.calls[0][1].attempt).toBe(2);
  });

  it('传入 backtrack_to 时 schema 层静默忽略（Zod stripUnknown 行为）', () => {
    runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'proposal',
      report: 'ok',
      checklist: VALID_ITEMS,
      // @ts-expect-error — backtrack_to 不在 PhaseLogOptions 类型中，但运行时传入
      backtrack_to: 'proposal',
    });

    expect(appendEntry).toHaveBeenCalledTimes(1);
    const entry = vi.mocked(appendEntry).mock.calls[0][1];
    expect(entry.backtrack_to).toBeUndefined();
    expect(entry.backtrack_reason).toBeUndefined();
  });

  it('identical pass calls invoke appendEntry twice (idempotency)', () => {
    const opts = {
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'proposal' as const,
      report: 'ok',
      checklist: VALID_ITEMS,
    };

    runPhaseLog(opts);
    runPhaseLog(opts);

    expect(appendEntry).toHaveBeenCalledTimes(2);
  });
});

// ===========================================================================
// 异常
// ===========================================================================

describe('runPhaseLog — 异常', () => {
  it('readEvalJson 抛错时抛出「读取 workflow.json 失败:」+ 原因', () => {
    vi.mocked(readEvalJson).mockImplementation(() => {
      throw new Error('workflow.json.eval 必须是数组，但实际类型为 object');
    });

    expect(() =>
      runPhaseLog({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'proposal',
        report: 'ok',
        checklist: VALID_ITEMS,
      }),
    ).toThrow(/读取 workflow\.json 失败: .*workflow\.json\.eval 必须是数组/);

    expect(appendEntry).not.toHaveBeenCalled();
  });

  it('appendEntry 抛错（缺 workflow.json）时抛出「写入 workflow.json 失败:」，message 含内层 change_create 指引且未写盘（AC-13）', () => {
    vi.mocked(appendEntry).mockImplementation(() => {
      throw new Error(
        'workflow.json 不存在: /tmp/test-change/workflow.json，无法写入评估记录。请先通过 change_create 创建 change。',
      );
    });

    let captured: Error | null = null;
    try {
      runPhaseLog({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'proposal',
        report: 'ok',
        checklist: VALID_ITEMS,
      });
    } catch (e: unknown) {
      captured = e as Error;
    }

    expect(captured).not.toBeNull();
    expect(captured!.message).toContain('写入 workflow.json 失败:');
    expect(captured!.message).toContain('change_create');
    expectNoLegacyWrite();
  });

  it('checklist 含任一项 fail 时 verdict 为 fail，appendEntry 入参 verdict 为 fail', () => {
    runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'proposal',
      report: 'has issues',
      checklist: FAILED_ITEMS,
    });

    const entry = vi.mocked(appendEntry).mock.calls[0][1];
    expect(entry.verdict).toBe('fail');
  });

  it('report 为 501 字符时抛长度错误且不调用 appendEntry', () => {
    expect(() =>
      runPhaseLog({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'proposal',
        report: 'x'.repeat(501),
        checklist: VALID_ITEMS,
      }),
    ).toThrow(/500 字符/);

    expect(appendEntry).not.toHaveBeenCalled();
  });

  it('options 缺 checklist 等必填（运行时传入残缺对象）时在写入前抛错', () => {
    expect(() =>
      runPhaseLog({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'proposal',
        report: 'ok',
      } as Parameters<typeof runPhaseLog>[0]),
    ).toThrow();

    expect(appendEntry).not.toHaveBeenCalled();
  });
});

// ===========================================================================
// 边界
// ===========================================================================

describe('runPhaseLog — 边界', () => {
  it('report 为空串时通过长度校验并 append', () => {
    runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'proposal',
      report: '',
      checklist: VALID_ITEMS,
    });

    expect(appendEntry).toHaveBeenCalledTimes(1);
    expect(vi.mocked(appendEntry).mock.calls[0][1].report).toBe('');
  });

  it('report 恰 500 字符时通过，501 字符仍抛错', () => {
    expect(() =>
      runPhaseLog({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'proposal',
        report: 'x'.repeat(500),
        checklist: VALID_ITEMS,
      }),
    ).not.toThrow();

    expect(appendEntry).toHaveBeenCalledTimes(1);
  });

  it('checklist=[] 且未 skip 时 verdict 为 pass（every 空数组）并 append', () => {
    runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'proposal',
      report: 'empty checklist',
      checklist: [],
    });

    expect(vi.mocked(appendEntry).mock.calls[0][1].verdict).toBe('pass');
  });

  it('checklist 单元素 pass 时 verdict 为 pass；超大列表（>100 全 pass）仍只 append 一次', () => {
    runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'proposal',
      report: 'single',
      checklist: VALID_ITEMS,
    });
    expect(vi.mocked(appendEntry).mock.calls[0][1].verdict).toBe('pass');

    vi.mocked(appendEntry).mockClear();
    runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'proposal',
      report: 'huge',
      checklist: Array.from({ length: 120 }, (_, i) => ({
        item: `item-${i}`,
        pass: true,
        evidence: 'ok',
      })),
    });

    expect(appendEntry).toHaveBeenCalledTimes(1);
    expect(vi.mocked(appendEntry).mock.calls[0][1].verdict).toBe('pass');
  });

  it('attempt 显式传 0 / -1 时抛「attempt 必须为正整数」且不写盘', () => {
    for (const attempt of [0, -1]) {
      vi.mocked(appendEntry).mockClear();
      expect(() =>
        runPhaseLog({
          project_root: FIXTURE_PROJECT_ROOT,
          change: 'test-change',
          phase: 'proposal',
          report: 'ok',
          checklist: VALID_ITEMS,
          attempt,
        }),
      ).toThrow(/attempt 必须为正整数/);
      expect(appendEntry).not.toHaveBeenCalled();
    }
  });

  it('phase 为非法枚举（integration-test）时 buildEntry 抛错且不写盘', () => {
    expect(() =>
      runPhaseLog({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'integration-test' as Parameters<typeof runPhaseLog>[0]['phase'],
        report: 'ok',
        checklist: VALID_ITEMS,
      }),
    ).toThrow();

    expect(appendEntry).not.toHaveBeenCalled();
  });

  it('change 为空串时不崩溃，读路径失败被包装为「读取 workflow.json 失败:」', () => {
    vi.mocked(readEvalJson).mockImplementation(() => {
      throw new Error('workflow.json 不存在: /openspec/changes/workflow.json');
    });

    expect(() =>
      runPhaseLog({
        project_root: FIXTURE_PROJECT_ROOT,
        change: '',
        phase: 'proposal',
        report: 'ok',
        checklist: VALID_ITEMS,
      }),
    ).toThrow(/读取 workflow\.json 失败:/);
  });

  it('change 超长（>1000 chars）/ 含 emoji 时不崩溃，错误被包装而非进程退出', () => {
    vi.mocked(appendEntry).mockImplementation(() => {
      throw new Error('workflow.json 不存在');
    });

    expect(() =>
      runPhaseLog({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'a'.repeat(1001),
        phase: 'proposal',
        report: 'ok',
        checklist: VALID_ITEMS,
      }),
    ).toThrow(/写入 workflow\.json 失败:/);

    expect(() =>
      runPhaseLog({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'chg-🧪',
        phase: 'proposal',
        report: 'ok',
        checklist: VALID_ITEMS,
      }),
    ).toThrow(/写入 workflow\.json 失败:/);
  });
});

// ===========================================================================
// 盖章与清场 — active_phase 运行态消费 (AC-3)
// ===========================================================================

describe('runPhaseLog — 盖章与清场 (AC-3)', () => {
  it('active_phase.phase === 本次 phase → 落盘条目含 start_at === active_phase.start_at，且 active_phase 被清空（clearActivePhase 恰一次）', () => {
    vi.mocked(readActivePhase).mockReturnValue({
      phase: 'implement',
      attempt: 2,
      start_at: '2026-09-18T08:00:00.000Z',
    });

    const result = runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'implement',
      report: 'ok',
      checklist: VALID_ITEMS,
    });

    const entry = vi.mocked(appendEntry).mock.calls[0][1];
    expect(entry.start_at).toBe('2026-09-18T08:00:00.000Z');
    expect(entry.timestamp).toMatch(/^\d{4}-\d{2}-\d{2}T/);
    expect(clearActivePhase).toHaveBeenCalledTimes(1);
    expect(result).toEqual({ written: true, phase: 'implement', attempt: 1 });
  });

  it('无 active_phase（null）→ 条目无 start_at 键且清场不被调用', () => {
    vi.mocked(readActivePhase).mockReturnValue(null);

    runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'proposal',
      report: 'ok',
      checklist: VALID_ITEMS,
    });

    const entry = vi.mocked(appendEntry).mock.calls[0][1];
    expect(Object.prototype.hasOwnProperty.call(entry, 'start_at')).toBe(false);
    expect(clearActivePhase).not.toHaveBeenCalled();
  });

  it('active_phase.phase 不匹配（他 phase 运行态）→ 条目无 start_at 且 active_phase 原样保留（不清场）', () => {
    vi.mocked(readActivePhase).mockReturnValue({
      phase: 'test-gen',
      attempt: 1,
      start_at: '2026-09-18T08:00:00.000Z',
    });

    runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'implement',
      report: 'ok',
      checklist: VALID_ITEMS,
    });

    const entry = vi.mocked(appendEntry).mock.calls[0][1];
    expect(Object.prototype.hasOwnProperty.call(entry, 'start_at')).toBe(false);
    expect(clearActivePhase).not.toHaveBeenCalled();
  });

  it('attempt 推导不受盖章影响：匹配 active_phase 时条目 attempt 仍来自 eval 历史', () => {
    vi.mocked(readActivePhase).mockReturnValue({
      phase: 'implement',
      attempt: 3,
      start_at: '2026-09-18T08:00:00.000Z',
    });
    vi.mocked(readEvalJson).mockReturnValue([
      {
        phase: 'implement',
        verdict: 'fail',
        attempt: 1,
        timestamp: '2026-09-11T10:00:00.000Z',
        report: '',
        checklist: [],
        backtrack_to: null,
      },
    ]);

    const result = runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'implement',
      report: 'retry',
      checklist: VALID_ITEMS,
    });

    expect(result.attempt).toBe(2);
    const entry = vi.mocked(appendEntry).mock.calls[0][1];
    expect(entry.attempt).toBe(2);
    expect(entry.start_at).toBe('2026-09-18T08:00:00.000Z');
  });
});

// ===========================================================================
// 清场时序与吞错（无双录，AC-3/design 决议）
// ===========================================================================

describe('runPhaseLog — 清场时序与吞错', () => {
  it('appendEntry 失败 → 不清空 active_phase（盖章在落盘前、清场在成功后的时序防回归），错误照常包装抛出', () => {
    vi.mocked(readActivePhase).mockReturnValue({
      phase: 'implement',
      attempt: 1,
      start_at: '2026-09-18T08:00:00.000Z',
    });
    vi.mocked(appendEntry).mockImplementation(() => {
      throw new Error('workflow.json 不存在: /tmp/test-change/workflow.json');
    });

    expect(() =>
      runPhaseLog({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'implement',
        report: 'ok',
        checklist: VALID_ITEMS,
      }),
    ).toThrow(/写入 workflow\.json 失败:/);

    expect(clearActivePhase).not.toHaveBeenCalled();
  });

  it('清场（clearActivePhase）抛错 → 仅 stderr 诊断，不抛错，仍返回 {written, phase, attempt}，条目已落盘（吞错收尾无双录）', () => {
    vi.mocked(readActivePhase).mockReturnValue({
      phase: 'implement',
      attempt: 1,
      start_at: '2026-09-18T08:00:00.000Z',
    });
    vi.mocked(clearActivePhase).mockImplementation(() => {
      throw new Error('EACCES: 模拟清场失败');
    });
    const stderrSpy = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);

    let result: { written: boolean; phase: string; attempt: number } | undefined;
    expect(() => {
      result = runPhaseLog({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'implement',
        report: 'ok',
        checklist: VALID_ITEMS,
      });
    }).not.toThrow();

    expect(appendEntry).toHaveBeenCalledTimes(1);
    expect(result).toEqual({ written: true, phase: 'implement', attempt: 1 });
    const stderrText = stderrSpy.mock.calls.map((c) => String(c[0])).join('');
    expect(stderrText).toContain('phase-log:');
    expect(stderrText).toContain('清理 active_phase 失败');
    expect(stderrText).toContain('EACCES');
    stderrSpy.mockRestore();
  });

  it('输出形状契约：返回对象键恰为 written / phase / attempt（回归）', () => {
    vi.mocked(readActivePhase).mockReturnValue({
      phase: 'implement',
      attempt: 1,
      start_at: '2026-09-18T08:00:00.000Z',
    });

    const result = runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'implement',
      report: 'ok',
      checklist: VALID_ITEMS,
    });

    expect(Object.keys(result).sort()).toEqual(['attempt', 'phase', 'written']);
  });
});
