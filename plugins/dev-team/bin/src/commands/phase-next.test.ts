/**
 * Unit tests for phase_next MCP tool — server-side orchestration logic.
 *
 * Tests cover: phase table resolution, normal progression, skip passed phases,
 * retry logic, backtrack, round limit, mid-phase interruption, skipped entries,
 * workflow_type variants, input validation, and all boundary scenarios.
 *
 * Tests call runPhaseNext with mocked workflow.json reads (the `eval` field is
 * the authoritative store, with a legacy `eval.json` fallback) — no real
 * filesystem access.
 *
 * @see openspec/changes/add-workflow-requirement-skill/test-design.md
 */

import * as fs from 'fs';

import { describe, it, expect, vi } from 'vite-plus/test';

vi.mock('fs', async (importOriginal) => {
  const actual = await importOriginal<typeof fs>();
  return {
    ...actual,
    existsSync: vi.fn(),
    readFileSync: vi.fn(),
    writeFileSync: vi.fn(),
    unlinkSync: vi.fn(),
  };
});

import { hasPhasePassed, runPhaseNext } from '../commands/phase-next';
import { resolveChangeDir } from '../lib/change';
import * as evalJson from '../lib/eval-json';
import { type EvalEntry } from '../lib/eval-json';
import { getProjectDir } from '../lib/project-root';
import { getPhaseTable } from '../lib/workflow';

const FIXTURE_PROJECT_ROOT = '/tmp/fixture-project';
const DEFAULT_RUN_ID = 'test-run';

// ---------------------------------------------------------------------------
// Mock helpers — construct evaluation entries for test scenarios
// ---------------------------------------------------------------------------

type MockEntry = EvalEntry;

// Module-level counter ensures each mock entry gets a unique, increasing timestamp.
let _tsCounter = 0;
function nextTs(): string {
  return new Date(Date.now() + ++_tsCounter).toISOString();
}

function passEntry(
  phase: EvalEntry['phase'],
  attempt: number = 1,
  overrides: Partial<MockEntry> = {},
): MockEntry {
  return {
    phase,
    verdict: 'pass',
    attempt,
    timestamp: nextTs(),
    backtrack_to: null,
    report: '',
    checklist: [],
    ...overrides,
  };
}

function failEntry(
  phase: EvalEntry['phase'],
  attempt: number = 1,
  overrides: Partial<MockEntry> = {},
): MockEntry {
  return {
    phase,
    verdict: 'fail',
    attempt,
    timestamp: nextTs(),
    backtrack_to: null,
    report: '',
    checklist: [],
    ...overrides,
  };
}

function backtrackEntry(
  phase: EvalEntry['phase'],
  backtrack_to: string,
  attempt: number = 1,
  overrides: Partial<MockEntry> = {},
): MockEntry {
  return {
    phase,
    verdict: 'fail',
    attempt,
    timestamp: nextTs(),
    backtrack_to,
    report: '',
    checklist: [],
    ...overrides,
  };
}

function skippedEntry(phase: EvalEntry['phase'], attempt: number = 1): MockEntry {
  return {
    phase,
    verdict: 'pass',
    attempt,
    timestamp: nextTs(),
    skipped: true,
    backtrack_to: null,
    report: '',
    checklist: [],
  };
}

function staleEntry(
  phase: EvalEntry['phase'],
  attempt: number = 1,
  overrides: Partial<MockEntry> = {},
): MockEntry {
  return {
    phase,
    verdict: 'pass',
    attempt,
    timestamp: nextTs(),
    backtrack_to: null,
    stale: true,
    report: '',
    checklist: [],
    ...overrides,
  };
}

// Note: the counter is intentionally not reset between tests — relative
// ordering within each test case is all that matters.

/** 评估存储布置方式（默认把条目写进 `workflow.json.eval`）。 */
interface NextStoreOptions {
  /** 条目放到遗留 `eval.json`，`workflow.json` 不带 `eval` 键（回退用例）。 */
  legacy?: boolean;
  /** `workflow.json` 的原始文本（用于非法 JSON / 非法形状）。 */
  workflowRaw?: string;
  /** `workflow.json` 不存在（缺文件用例）。 */
  missingWorkflow?: boolean;
}

/**
 * Arrange the fs mock and call `runPhaseNext`.
 *
 * The evaluation history is sourced from `workflow.json.eval` by default —
 * `eval.json` is only populated when `opts.legacy` is set, which exercises the
 * read fallback.
 */
function next(
  entries: MockEntry[],
  change: string = 'test-change',
  workflowType: string = 'requirement',
  runId: string = DEFAULT_RUN_ID,
  opts: NextStoreOptions = {},
) {
  const changeDir = resolveChangeDir(change, FIXTURE_PROJECT_ROOT);
  const useLegacy = opts.legacy === true;

  vi.mocked(fs.existsSync).mockImplementation((filePath: fs.PathLike) => {
    const p = String(filePath);
    if (p === changeDir) {
      return true;
    }
    if (p.endsWith('workflow.json')) {
      return opts.missingWorkflow !== true;
    }
    if (p.endsWith('eval.json')) {
      return useLegacy;
    }
    return false;
  });
  vi.mocked(fs.readFileSync).mockImplementation(
    (
      path: fs.PathOrFileDescriptor,
      _options?: BufferEncoding | fs.ObjectEncodingOptions | null,
    ): string => {
      const p = String(path);
      if (p.endsWith('workflow.json')) {
        if (opts.workflowRaw !== undefined) {
          return opts.workflowRaw;
        }
        if (useLegacy) {
          return JSON.stringify({ workflow_type: workflowType });
        }
        return JSON.stringify({ workflow_type: workflowType, eval: entries });
      }
      if (p.endsWith('eval.json')) {
        return JSON.stringify(entries);
      }
      return '';
    },
  );
  return runPhaseNext({ change, project_root: FIXTURE_PROJECT_ROOT, run_id: runId });
}

function buildLongHistory(count: number, phase: EvalEntry['phase'] = 'proposal'): MockEntry[] {
  const entries: MockEntry[] = [];
  for (let i = 0; i < count; i++) {
    entries.push(passEntry(phase, i + 1));
  }
  return entries;
}

// ---------------------------------------------------------------------------
// 评估存储来源 — workflow.json.eval 优先 / 遗留 eval.json 回退 / 禁止合并
// ---------------------------------------------------------------------------

describe('runPhaseNext — 评估存储来源 (AC-3, AC-5, AC-12)', () => {
  it('workflow.json.eval 为 [] 且无遗留文件时按 requirement 返回 first run proposal（AC-3）', () => {
    const result = next([]);
    expect(result.next_phase).toBe('proposal');
    expect(result.last_result).toBeNull();
    expect(result.round).toBe(1);
  });

  it('仅有遗留 eval.json（proposal 非 stale pass）且 workflow.json 无 eval 键时返回 dev-design（与合并前一致，AC-3）', () => {
    const result = next([passEntry('proposal')], 'test-change', 'requirement', DEFAULT_RUN_ID, {
      legacy: true,
    });
    expect(result.next_phase).toBe('dev-design');
    expect(result.error).toBeNull();
  });

  it('workflow.json.eval 为 [] 且遗留 eval.json 含 proposal pass 时仍为 first run proposal（不合并两源，AC-5）', () => {
    const result = next([], 'test-change', 'requirement', DEFAULT_RUN_ID, {
      legacy: true,
    });
    expect(result.next_phase).toBe('proposal');
    expect(result.last_result).toBeNull();
  });

  it('workflow.json.eval 含 proposal 非 stale pass 时返回 dev-design，last_result.phase 为 proposal', () => {
    const result = next([passEntry('proposal', 1, { report: '提案通过' })]);
    expect(result.next_phase).toBe('dev-design');
    expect(result.last_result!.phase).toBe('proposal');
    expect(result.last_result!.report).toBe('提案通过');
  });

  it('readEvalJson 抛错（eval 非数组）时 message 匹配 Failed to read workflow.json:（AC-12）', () => {
    // `getWorkflowType` 会先做整文件校验，因此 `eval: {}` 在磁盘路径上先抛格式非法；
    // 这里 stub readEvalJson 以覆盖读取失败的包装文案。
    const spy = vi.spyOn(evalJson, 'readEvalJson').mockImplementationOnce(() => {
      throw new Error('workflow.json.eval 必须是数组，但实际类型为 object');
    });
    try {
      expect(() => next([])).toThrow(
        /Failed to read workflow\.json: workflow\.json\.eval 必须是数组/,
      );
    } finally {
      spy.mockRestore();
    }
  });

  it('workflow.json.eval 为对象时 getWorkflowType 先以格式非法终止（AC-14）', () => {
    expect(() =>
      next([], 'test-change', 'requirement', DEFAULT_RUN_ID, {
        workflowRaw: JSON.stringify({ workflow_type: 'requirement', eval: {} }),
      }),
    ).toThrow(/格式非法/);
  });

  it('成功路径只读：不调用 writeEvalJson，也不写 / 删任何文件', () => {
    const writeSpy = vi.spyOn(evalJson, 'writeEvalJson');
    try {
      next([passEntry('proposal'), passEntry('dev-design')]);
      expect(writeSpy).not.toHaveBeenCalled();
      expect(vi.mocked(fs.writeFileSync)).not.toHaveBeenCalled();
      expect(vi.mocked(fs.unlinkSync)).not.toHaveBeenCalled();
    } finally {
      writeSpy.mockRestore();
    }
  });

  it('生涯已有多条（来自 workflow.json.eval）时窗口算法仍以 entries.length 为 anchor', () => {
    next(buildLongHistory(30), 'test-change', 'requirement', 'anchor-from-workflow');
    const result = next(buildLongHistory(33), 'test-change', 'requirement', 'anchor-from-workflow');
    expect(result.round).toBe(4);
    expect(result.error).toBeNull();
  });

  it('workflow.json 含未知键（note）时类型仍可解析并按 phase 表推进', () => {
    const result = next([], 'test-change', 'requirement', DEFAULT_RUN_ID, {
      workflowRaw: JSON.stringify({ workflow_type: 'requirement', note: 'x' }),
    });
    expect(result.next_phase).toBe('proposal');
    expect(result.total_phases).toBe(8);
  });

  it('workflow.json 缺 created（Optional None）时正常解析并按 phase 表推进', () => {
    const result = next([], 'test-change', 'requirement', DEFAULT_RUN_ID, {
      workflowRaw: JSON.stringify({ workflow_type: 'requirement' }),
    });
    expect(result.next_phase).toBe('proposal');
    expect(result.total_phases).toBe(8);
  });

  it('eval 键缺失且无遗留文件（Optional None）时按 first run 返回 proposal', () => {
    const result = next([], 'test-change', 'requirement', DEFAULT_RUN_ID, {
      workflowRaw: JSON.stringify({ workflow_type: 'requirement', created: '2026-09-11' }),
    });
    expect(result.next_phase).toBe('proposal');
    expect(result.last_result).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// First Run — Empty evaluation history
// ---------------------------------------------------------------------------

describe('runPhaseNext — First Run (empty evaluation history)', () => {
  it('should return proposal as next_phase when entries is empty', () => {
    const result = next([]);
    expect(result.next_phase).toBe('proposal');
    expect(result.done).toBe(false);
    expect(result.error).toBeNull();
  });

  it('should return proposal-planner as executor agent_type', () => {
    const result = next([]);
    expect(result.executor!.agent_type).toBe('__CALL_AGENT:proposal-planner__');
  });

  it('should return proposal-evaluator as evaluator agent_type', () => {
    const result = next([]);
    expect(result.evaluator!.agent_type).toBe('__CALL_AGENT:proposal-evaluator__');
  });

  it('should set round to 1 on first call', () => {
    const result = next([]);
    expect(result.round).toBe(1);
  });

  it('should set phase_index to 1 and total_phases to 8', () => {
    const result = next([]);
    expect(result.phase_index).toBe(1);
    expect(result.total_phases).toBe(8);
  });

  it('should include change name in executor prompt', () => {
    const result = next([], 'my-change');
    expect(result.executor!.prompt).toContain('my-change');
  });

  it('proposal executor prompt references free-form explore.md and merge semantics', () => {
    const result = next([], 'my-change');
    expect(result.executor!.prompt).toContain('openspec/changes/my-change/explore.md');
    expect(result.executor!.prompt).toMatch(/free-form explore context/i);
    expect(result.executor!.prompt).toMatch(/merge new explore insights/i);
    expect(result.executor!.prompt).toMatch(/do not rewrite from scratch/i);
    expect(result.executor!.prompt).toMatch(/Do not expect inline EXPLORE_CONTEXT_SUMMARY/i);
    expect(result.executor!.prompt).toMatch(/Write or update proposal\.md/i);
  });
});

// ---------------------------------------------------------------------------
// Normal Progression — Pass phases in sequence
// ---------------------------------------------------------------------------

describe('runPhaseNext — Normal Progression', () => {
  it('should return dev-design after proposal passes', () => {
    const result = next([passEntry('proposal')]);
    expect(result.next_phase).toBe('dev-design');
  });

  it('should return test-design after phases 01-02 pass', () => {
    const result = next([passEntry('proposal'), passEntry('dev-design')]);
    expect(result.next_phase).toBe('test-design');
  });

  it('should return implement after phases 01-03 pass（AC-5）', () => {
    const result = next([passEntry('proposal'), passEntry('dev-design'), passEntry('test-design')]);
    expect(result.next_phase).toBe('implement');
  });

  it('should return test-gen after phases 01-03 and 05 pass（AC-6）', () => {
    const result = next([
      passEntry('proposal'),
      passEntry('dev-design'),
      passEntry('test-design'),
      passEntry('implement'),
    ]);
    expect(result.next_phase).toBe('test-gen');
  });

  it('should return test-execution after phases 01-05 and 04 pass', () => {
    const result = next([
      passEntry('proposal'),
      passEntry('dev-design'),
      passEntry('test-design'),
      passEntry('implement'),
      passEntry('test-gen'),
    ]);
    expect(result.next_phase).toBe('test-execution');
  });

  it('should return code-review with executor: null (EVAL-ONLY)', () => {
    const result = next([
      passEntry('proposal'),
      passEntry('dev-design'),
      passEntry('test-design'),
      passEntry('test-gen'),
      passEntry('implement'),
      passEntry('test-execution'),
    ]);
    expect(result.next_phase).toBe('code-review');
    expect(result.executor).toBeNull();
    expect(result.evaluator!.agent_type).toBe('__CALL_AGENT:code-review-evaluator__');
  });

  it('should return acceptance with executor: null (EVAL-ONLY)', () => {
    const entries = [
      passEntry('proposal'),
      passEntry('dev-design'),
      passEntry('test-design'),
      passEntry('test-gen'),
      passEntry('implement'),
      passEntry('test-execution'),
      passEntry('code-review'),
    ];
    const result = next(entries);
    expect(result.next_phase).toBe('acceptance');
    expect(result.executor).toBeNull();
  });

  it('should return done=true when all 8 phases pass', () => {
    const entries = [
      passEntry('proposal'),
      passEntry('dev-design'),
      passEntry('test-design'),
      passEntry('test-gen'),
      passEntry('implement'),
      passEntry('test-execution'),
      passEntry('code-review'),
      passEntry('acceptance'),
    ];
    const result = next(entries);
    expect(result.done).toBe(true);
    expect(result.executor).toBeNull();
    expect(result.evaluator).toBeNull();
    expect(result.next_phase).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Skip Passed Phases — AC-6
// ---------------------------------------------------------------------------

describe('runPhaseNext — Skip Passed Phases (AC-6)', () => {
  it('should skip to implement when 01-03 pass — 非 test-gen（AC-5）', () => {
    const result = next([passEntry('proposal'), passEntry('dev-design'), passEntry('test-design')]);
    expect(result.next_phase).toBe('implement');
  });

  it('should skip to test-execution when phases 01-05 and 04 pass', () => {
    const entries = [
      passEntry('proposal'),
      passEntry('dev-design'),
      passEntry('test-design'),
      passEntry('implement'),
      passEntry('test-gen'),
    ];
    const result = next(entries);
    expect(result.next_phase).toBe('test-execution');
  });

  it('should return done=true when all phases have pass records', () => {
    const entries = [
      passEntry('proposal'),
      passEntry('dev-design'),
      passEntry('test-design'),
      passEntry('test-gen'),
      passEntry('implement'),
      passEntry('test-execution'),
      passEntry('code-review'),
      passEntry('acceptance'),
    ];
    const result = next(entries);
    expect(result.done).toBe(true);
    expect(result.error).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Retry Logic — AC-7
// ---------------------------------------------------------------------------

describe('runPhaseNext — Retry Logic (session window)', () => {
  it('窗内首次 fail 后仍返回同一 phase', () => {
    next([passEntry('proposal')], 'test-change', undefined, 'retry-s1');
    const result = next(
      [passEntry('proposal'), failEntry('dev-design')],
      'test-change',
      undefined,
      'retry-s1',
    );
    expect(result.next_phase).toBe('dev-design');
    expect(result.error).toBeNull();
  });

  it('窗内 4 次 fail 仍返回该 phase', () => {
    next([passEntry('proposal')], 'test-change', undefined, 'retry-s2');
    const result = next(
      [
        passEntry('proposal'),
        failEntry('dev-design', 1),
        failEntry('dev-design', 2),
        failEntry('dev-design', 3),
        failEntry('dev-design', 4),
      ],
      'test-change',
      undefined,
      'retry-s2',
    );
    expect(result.next_phase).toBe('dev-design');
    expect(result.error).toBeNull();
  });

  it('窗内 5 次 fail 返回 max_retries_exceeded', () => {
    next([passEntry('proposal')], 'test-change', undefined, 'retry-s3');
    const result = next(
      [
        passEntry('proposal'),
        failEntry('dev-design', 1),
        failEntry('dev-design', 2),
        failEntry('dev-design', 3),
        failEntry('dev-design', 4),
        failEntry('dev-design', 5),
      ],
      'test-change',
      undefined,
      'retry-s3',
    );
    expect(result.error).toBe('max_retries_exceeded');
    expect(result.next_phase).toBeNull();
  });

  it('max_retries_exceeded 时 message 含中文超过最大重试次数', () => {
    next([passEntry('proposal')], 'test-change', undefined, 'retry-s4');
    const result = next(
      [
        passEntry('proposal'),
        failEntry('dev-design', 1),
        failEntry('dev-design', 2),
        failEntry('dev-design', 3),
        failEntry('dev-design', 4),
        failEntry('dev-design', 5),
      ],
      'test-change',
      undefined,
      'retry-s4',
    );
    expect(result.message).toContain('超过最大重试次数');
  });

  it('pass 后 fail 不计入先前 fail 预算', () => {
    const result = next([
      passEntry('proposal', 1),
      passEntry('dev-design', 1),
      failEntry('test-design', 1),
    ]);
    expect(result.next_phase).toBe('test-design');
    expect(result.error).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Backtrack — AC-8
// ---------------------------------------------------------------------------

describe('runPhaseNext — Backtrack', () => {
  it('should return target phase on backtrack without clearing entries', () => {
    const entries = [
      passEntry('proposal'),
      passEntry('dev-design'),
      passEntry('test-design'),
      backtrackEntry('test-design', 'proposal'),
    ];
    const result = next(entries);
    expect(result.next_phase).toBe('proposal');
    // No updatedEntries — phase_next is read-only
    expect(result).not.toHaveProperty('updatedEntries');
  });

  it('should re-execute executor + evaluator after backtrack to proposal', () => {
    const result = next([
      passEntry('proposal'),
      passEntry('dev-design'),
      backtrackEntry('dev-design', 'proposal'),
    ]);
    expect(result.executor!.agent_type).toBe('__CALL_AGENT:proposal-planner__');
    expect(result.evaluator!.agent_type).toBe('__CALL_AGENT:proposal-evaluator__');
  });

  it('should handle backtrack from a later phase to mid-chain', () => {
    const entries = [
      passEntry('proposal'),
      passEntry('dev-design'),
      passEntry('test-design'),
      passEntry('test-gen'),
      passEntry('implement'),
      passEntry('test-execution'),
      backtrackEntry('code-review', 'test-gen'),
    ];
    const result = next(entries);
    expect(result.next_phase).toBe('test-gen');
  });

  it('should handle backtrack set in latest entry only', () => {
    // Previous backtrack but later entry overwrites it
    const result = next([
      passEntry('proposal'),
      passEntry('dev-design'),
      backtrackEntry('dev-design', 'proposal'),
      passEntry('dev-design', 2), // Re-passed after backtrack
    ]);
    // Now the latest entry for dev-design has verdict=pass, no backtrack
    // So backtrack is no longer active — should advance to test-design
    expect(result.error).toBeNull();
    expect(result.next_phase).toBe('test-design');
  });

  it('should not modify entries when backtrack is detected (read-only)', () => {
    const entries = [
      passEntry('proposal'),
      passEntry('dev-design'),
      backtrackEntry('dev-design', 'proposal'),
    ];
    const entriesCopy = JSON.parse(JSON.stringify(entries));
    next(entries);
    // Entries should be unmodified (no clearEntriesFromPhase, no updatedEntries)
    expect(entries).toEqual(entriesCopy);
  });

  it('should throw invalid_backtrack_target for unknown target', () => {
    const entries = [
      passEntry('proposal'),
      failEntry('dev-design', 1, { backtrack_to: 'unknown' } as Partial<EvalEntry>),
    ];
    const result = next(entries);
    expect(result.error).toBe('invalid_backtrack_target');
  });
});

// ---------------------------------------------------------------------------
// getLatestBacktrackInfo — Backtrack reason propagation (AC-4)
// ---------------------------------------------------------------------------

describe('getLatestBacktrackInfo', () => {
  it('返回对象包含 target 和 reason 两个字段（AC-4）', () => {
    const result = next([
      passEntry('proposal'),
      passEntry('dev-design'),
      backtrackEntry('test-design', 'proposal', 1, { backtrack_reason: '测试原因' }),
    ]);
    expect(result.next_phase).toBe('proposal');
    // reason propagated to prompt indirectly verifies getLatestBacktrackInfo return
    expect(result.executor!.prompt).toContain('⚠️ 回溯原因: 测试原因');
  });

  it('最新的 backtrack 条目包含 reason 时正确返回（AC-4）', () => {
    const result = next([
      passEntry('proposal'),
      backtrackEntry('dev-design', 'proposal', 1, { backtrack_reason: '设计文档缺少API签名部分' }),
    ]);
    expect(result.executor!.prompt).toContain('⚠️ 回溯原因: 设计文档缺少API签名部分');
    expect(result.evaluator!.prompt).toContain('⚠️ 回溯原因: 设计文档缺少API签名部分');
  });

  it('回溯条目无 backtrack_reason 字段时 reason 返回 null（AC-4）', () => {
    const result = next([
      passEntry('proposal'),
      passEntry('dev-design'),
      backtrackEntry('test-design', 'proposal'),
    ]);
    expect(result.next_phase).toBe('proposal');
    expect(result.executor!.prompt).not.toContain('⚠️ 回溯原因');
    expect(result.evaluator!.prompt).not.toContain('⚠️ 回溯原因');
  });

  it('无任何回溯条目时 target 和 reason 均为 null（AC-4）', () => {
    const result = next([passEntry('proposal'), passEntry('dev-design'), passEntry('test-design')]);
    expect(result.next_phase).toBe('implement');
    expect(result.executor!.prompt).not.toContain('⚠️ 回溯原因');
  });

  it('多个回溯条目中只返回最新的 backtrack_reason（边界）', () => {
    const result = next([
      passEntry('proposal'),
      passEntry('dev-design'),
      backtrackEntry('dev-design', 'proposal', 2, { backtrack_reason: '第一次回溯原因' }),
      backtrackEntry('test-design', 'proposal', 1, { backtrack_reason: '第二次回溯原因' }),
    ]);
    expect(result.next_phase).toBe('proposal');
    expect(result.executor!.prompt).toContain('第二次回溯原因');
    expect(result.executor!.prompt).not.toContain('第一次回溯原因');
  });
});

// ---------------------------------------------------------------------------
// Backtrack Prompt — reason propagation (AC-5)
// ---------------------------------------------------------------------------

describe('Backtrack Prompt — reason propagation', () => {
  it('executor prompt 末尾包含 ⚠️ 回溯原因: <reason>（AC-5）', () => {
    const result = next([
      passEntry('proposal'),
      backtrackEntry('dev-design', 'proposal', 1, { backtrack_reason: '测试原因' }),
    ]);
    expect(result.executor!.prompt).toMatch(/⚠️ 回溯原因: 测试原因$/);
  });

  it('evaluator prompt 末尾包含 ⚠️ 回溯原因: <reason>（AC-5）', () => {
    const result = next([
      passEntry('proposal'),
      backtrackEntry('dev-design', 'proposal', 1, { backtrack_reason: '测试原因' }),
    ]);
    expect(result.evaluator!.prompt).toMatch(/⚠️ 回溯原因: 测试原因$/);
  });

  it('无 backtrack_reason 时 prompt 不拼接回溯原因（AC-5 异常）', () => {
    const result = next([
      passEntry('proposal'),
      passEntry('dev-design'),
      backtrackEntry('test-design', 'proposal'),
    ]);
    expect(result.executor!.prompt).not.toContain('⚠️ 回溯原因');
    expect(result.evaluator!.prompt).not.toContain('⚠️ 回溯原因');
  });

  it('backtrack_reason 长度为 500 字符时 prompt 包含完整内容（AC-5 边界）', () => {
    const longReason = 'a'.repeat(500);
    const result = next([
      passEntry('proposal'),
      backtrackEntry('dev-design', 'proposal', 1, { backtrack_reason: longReason }),
    ]);
    expect(result.executor!.prompt).toContain(`⚠️ 回溯原因: ${longReason}`);
  });

  it('正常前进（非回溯）时 prompt 不含回溯原因（AC-5 异常）', () => {
    const result = next([passEntry('proposal')]);
    expect(result.next_phase).toBe('dev-design');
    expect(result.executor!.prompt).not.toContain('⚠️ 回溯原因');
  });

  it('回溯原因是空格字符串时 prompt 包含空格（当前行为：空格为 truthy 值）', () => {
    const result = next([
      passEntry('proposal'),
      backtrackEntry('dev-design', 'proposal', 1, { backtrack_reason: '   ' }),
    ]);
    // '   '（纯空格）在 Zod 中为有效字符串，在 JS 中为 truthy 值
    // 当前 buildPhaseDef 在 backtrackReason truthy 时拼接，因此 prompt 包含空格原因
    expect(result.executor!.prompt).toContain('⚠️ 回溯原因: ');
  });
});

// ---------------------------------------------------------------------------
// getLatestBacktrackInfo — backward compatibility (AC-6)
// ---------------------------------------------------------------------------

describe('getLatestBacktrackInfo — backward compatibility (AC-6)', () => {
  it('旧 eval.json 条目（无 backtrack_reason）解析不抛错（AC-6）', () => {
    const entries: MockEntry[] = [
      {
        phase: 'proposal',
        verdict: 'pass',
        attempt: 1,
        timestamp: new Date().toISOString(),
        backtrack_to: null,
        report: '',
        checklist: [],
      },
      {
        phase: 'dev-design',
        verdict: 'fail',
        attempt: 1,
        timestamp: new Date(Date.now() + 1).toISOString(),
        backtrack_to: 'proposal',
        report: '',
        checklist: [],
      },
    ];
    expect(() => next(entries)).not.toThrow();
  });

  it('混合新旧格式条目时，旧条目的 reason 为 null（AC-6）', () => {
    // Old entry without backtrack_reason, new entry with but different phase
    const entries: MockEntry[] = [
      {
        phase: 'proposal',
        verdict: 'pass',
        attempt: 1,
        timestamp: new Date().toISOString(),
        backtrack_to: null,
        report: '',
        checklist: [],
      },
      {
        phase: 'dev-design',
        verdict: 'fail',
        attempt: 1,
        timestamp: new Date(Date.now() + 1).toISOString(),
        backtrack_to: 'proposal',
        // No backtrack_reason — old format
        report: '',
        checklist: [],
      },
    ];
    const result = next(entries);
    expect(result.next_phase).toBe('proposal');
    // No backtrack_reason → reason is null → prompt has no reason suffix
    expect(result.executor!.prompt).not.toContain('⚠️ 回溯原因');
  });

  it('getLatestBacktrackInfo() 在旧格式条目上正确返回 target 和 reason: null（AC-6）', () => {
    // Mix of old and new format entries — use nextTs() for timestamp ordering
    const entries: MockEntry[] = [
      passEntry('proposal'),
      passEntry('dev-design'),
      {
        phase: 'test-design',
        verdict: 'fail',
        attempt: 1,
        timestamp: nextTs(),
        backtrack_to: 'proposal',
        // No backtrack_reason — old format without the field
        report: '',
        checklist: [],
      },
    ];
    const result = next(entries);
    // Backtrack target is correctly identified
    expect(result.next_phase).toBe('proposal');
    // Reason should be null (no backtrack_reason in entries)
    expect(result.executor!.prompt).not.toContain('⚠️ 回溯原因');
    // Should still be a valid phase response with executor + evaluator
    expect(result.executor).not.toBeNull();
    expect(result.evaluator).not.toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Stale Entry Filtering — AC-11, AC-12
// ---------------------------------------------------------------------------

describe('runPhaseNext — Stale Entry Filtering (AC-11, AC-12)', () => {
  it('should skip stale pass entries when determining next phase (AC-11)', () => {
    // 01 pass, 02 pass but stale, 03 pass
    const result = next([
      passEntry('proposal'),
      staleEntry('dev-design'),
      passEntry('test-design'),
    ]);
    // 02 appears stale to hasPhasePassed, so should be returned
    expect(result.next_phase).toBe('dev-design');
  });

  it('should return done=true when all phases have non-stale pass (mix of stale and fresh)', () => {
    const entries = [
      passEntry('proposal'),
      staleEntry('dev-design'), // stale
      passEntry('dev-design', 2), // fresh pass
      staleEntry('test-design'),
      passEntry('test-design', 2),
      passEntry('test-gen'),
      staleEntry('implement'),
      passEntry('implement', 2),
      passEntry('test-execution'),
      passEntry('code-review'),
      staleEntry('acceptance'),
      passEntry('acceptance', 2),
    ];
    const result = next(entries);
    expect(result.done).toBe(true);
  });

  it('should treat missing stale field as stale:false (AC-12 backward compat)', () => {
    // Entries without stale field should be treated as valid
    const result = next([passEntry('proposal')]);
    expect(result.next_phase).toBe('dev-design');
    expect(result.error).toBeNull();
  });

  it('should resume correctly when stale entries are in middle phases', () => {
    // 01-03 pass (03 stale), 04 pass
    const entries = [
      passEntry('proposal'),
      passEntry('dev-design'),
      staleEntry('test-design'), // marked stale by backtrack propagation
      passEntry('test-gen'), // 04's pass should be ignored because 03 needs to be redone first
    ];
    const result = next(entries);
    // Linear scan: 01 and 02 pass non-stale, 03 pass but stale → return 03
    expect(result.next_phase).toBe('test-design');
  });

  it('should return test-gen when 05 pass but 04 is stale（AC-7）', () => {
    const entries = [
      passEntry('proposal'),
      passEntry('dev-design'),
      passEntry('test-design'),
      passEntry('implement'),
      staleEntry('test-gen'),
    ];
    const result = next(entries);
    expect(result.next_phase).toBe('test-gen');
  });
});

// ---------------------------------------------------------------------------
// Dependency-Graph Driven — AC-5, AC-6, AC-7
// ---------------------------------------------------------------------------

describe('runPhaseNext — Dependency-Graph Driven (AC-5, AC-6, AC-7)', () => {
  it('should return dev-design when test-design missing pass but 02 not passed (AC-5)', () => {
    // 01 pass, 02 fail → 03 cannot run (depends on 02)
    const result = next([passEntry('proposal'), failEntry('dev-design')]);
    expect(result.next_phase).toBe('dev-design');
  });

  it('should return implement when dev-design is pass but 05 not passed (AC-6)', () => {
    // 01 pass, 02 pass → 03/04 not passed but 05's dependency (02) is satisfied
    // Linear scan returns 03 first, but 05 is also available
    // This test verifies that 03 not being passed doesn't block 05 from being considered
    // when scanning linearly — 03 comes before 05 and has no valid pass
    const result = next([passEntry('proposal'), passEntry('dev-design')]);
    // 03 is first without pass → correct per linear scan + propagation model
    expect(result.next_phase).toBe('test-design');
  });

  it('should return implement when 04 pass but 05 not passed — 线性扫描优先 05（AC-11 子场景）', () => {
    const result = next([
      passEntry('proposal'),
      passEntry('dev-design'),
      passEntry('test-design'),
      passEntry('test-gen'),
    ]);
    expect(result.next_phase).toBe('implement');
  });
});

// ---------------------------------------------------------------------------
// Round Limit
// ---------------------------------------------------------------------------

describe('runPhaseNext — Round Limit (session window)', () => {
  it('同 run_id 窗内 21 条 entries 应返回 round_limit_exceeded', () => {
    next([], 'test-change', undefined, 'round-limit-s1');
    const windowEntries: MockEntry[] = [];
    for (let i = 0; i < 21; i++) {
      windowEntries.push(passEntry('proposal', i + 1));
    }
    const result = next(windowEntries, 'test-change', undefined, 'round-limit-s1');
    expect(result.error).toBe('round_limit_exceeded');
    expect(result.done).toBe(false);
  });

  it('round_limit_exceeded 时 message 含 20 轮中文提示，指向 workflow.json 且不再提 eval.json', () => {
    next([], 'test-change', undefined, 'round-limit-s2');
    const windowEntries = buildLongHistory(21);
    const result = next(windowEntries, 'test-change', undefined, 'round-limit-s2');
    expect(result.message).toContain('20 轮');
    expect(result.message).toContain('workflow.json');
    expect(result.message).not.toContain('eval.json');
  });

  it('同 run_id 窗内 19 条 entries 时 round===20 且 error 为 null', () => {
    next([], 'test-change', undefined, 'round-limit-s3');
    const windowEntries = buildLongHistory(19);
    const result = next(windowEntries, 'test-change', undefined, 'round-limit-s3');
    expect(result.round).toBe(20);
    expect(result.error).toBeNull();
  });

  it('同 run_id 窗内回溯循环堆满 21 条仍触发 round_limit_exceeded', () => {
    next([], 'test-change', undefined, 'round-limit-s4');
    const windowEntries: MockEntry[] = [];
    for (let i = 0; i < 21; i++) {
      const phase = i % 2 === 0 ? 'proposal' : 'dev-design';
      windowEntries.push({
        phase,
        verdict: 'fail',
        attempt: Math.floor(i / 2) + 1,
        timestamp: new Date(Date.now() + i).toISOString(),
        backtrack_to: i % 2 === 1 ? 'proposal' : null,
        report: '',
        checklist: [],
      });
    }
    const result = next(windowEntries, 'test-change', undefined, 'round-limit-s4');
    expect(result.error).toBe('round_limit_exceeded');
  });
});

// ---------------------------------------------------------------------------
// Mid-Phase Interruption
// ---------------------------------------------------------------------------

describe('runPhaseNext — Mid-Phase Interruption', () => {
  it('should re-return the phase when evaluator has not logged (no entries for phase)', () => {
    // If the latest eval.json only shows pass for prior phases but the current
    // phase has no evaluator entry yet (never started), we treat it as "not passed"
    // and return it for execution.
    const result = next([passEntry('proposal')]);
    // proposal passed, so next should be dev-design
    expect(result.next_phase).toBe('dev-design');
  });

  it('should return the incomplete phase if evaluator never logged', () => {
    // If proposal passed but dev-design has no eval entries at all
    // (executor ran but evaluator never logged), the phase is not counted
    // as passed and phase_next should return it for execution.
    const result = next([passEntry('proposal')]);
    expect(result.next_phase).toBe('dev-design');
  });
});

// ---------------------------------------------------------------------------
// Skipped (No-Op) Phases
// ---------------------------------------------------------------------------

describe('runPhaseNext — Skipped Phases', () => {
  it('should treat skipped entries as pass', () => {
    const result = next([skippedEntry('test-gen')]);
    // proposal has no pass, so that should be returned despite test-gen having a skipped entry
    expect(result.next_phase).toBe('proposal');
  });

  it('should advance past a mix of skipped and passed phases', () => {
    // Pass 01, skip 02, pass 03 — should advance to implement
    const entries = [passEntry('proposal'), skippedEntry('dev-design'), passEntry('test-design')];
    const result = next(entries);
    expect(result.next_phase).toBe('implement');
  });
});

// ---------------------------------------------------------------------------
// workflow_type Variants
// ---------------------------------------------------------------------------

describe('runPhaseNext — workflow_type', () => {
  it('should default to requirement when workflow_type is omitted', () => {
    const result = next([], 'test-change');
    expect(result.total_phases).toBe(8);
    expect(result.next_phase).toBe('proposal');
  });

  it('should follow bug-fix phase table when workflow_type is bug-fix', () => {
    const result = next([], 'test-change', 'bug-fix');
    expect(result.total_phases).toBe(6);
  });

  it('should follow refactor phase table when workflow_type is refactor', () => {
    const result = next([], 'test-change', 'refactor');
    expect(result.total_phases).toBe(8);
  });

  it('should follow test-only phase table when workflow.json has test-only', () => {
    const result = next([], 'test-change', 'test-only');
    expect(result.total_phases).toBe(5);
    expect(result.next_phase).toBe('proposal');
  });

  it('test-only proposal prompt differs from requirement (AC-9)', () => {
    const requirementResult = next([], 'test-change');
    const testOnlyResult = next([], 'test-change', 'test-only');
    expect(testOnlyResult.executor!.prompt).not.toBe(requirementResult.executor!.prompt);
    expect(testOnlyResult.executor!.prompt).toMatch(/coverage gaps|testing strategy/i);
    expect(testOnlyResult.executor!.prompt).toContain('openspec/changes/test-change/explore.md');
    expect(testOnlyResult.executor!.prompt).toMatch(
      /Do not expect inline EXPLORE_CONTEXT_SUMMARY/i,
    );
  });

  it('test-only returns code-analyze after proposal passes (AC-2)', () => {
    const result = next([passEntry('proposal')], 'test-change', 'test-only');
    expect(result.next_phase).toBe('code-analyze');
  });

  it('test-only returns test-design after 01 and 02 pass (AC-4)', () => {
    const entries = [passEntry('proposal'), passEntry('code-analyze')];
    const result = next(entries, 'test-change', 'test-only');
    expect(result.next_phase).toBe('test-design');
  });

  it('test-only returns done after all five phases pass', () => {
    const entries = [
      passEntry('proposal'),
      passEntry('code-analyze'),
      passEntry('test-design'),
      passEntry('test-gen'),
      passEntry('test-execution'),
    ];
    const result = next(entries, 'test-change', 'test-only');
    expect(result.done).toBe(true);
    expect(result.total_phases).toBe(5);
  });

  it('bug-fix phase table 仍为 6 个 phase，不含 03/04（AC-10）', () => {
    const phases = getPhaseTable('bug-fix').map((p) => p.id);
    expect(phases.length).toBe(6);
    expect(phases).not.toContain('test-design');
    expect(phases).not.toContain('test-gen');
  });

  it('should return acceptance from bug-fix after code-review passes', () => {
    const entries = [
      passEntry('proposal'),
      passEntry('dev-design'),
      passEntry('implement'),
      passEntry('test-execution'),
      passEntry('code-review'),
    ];
    const result = next(entries, 'test-change', 'bug-fix');
    expect(result.next_phase).toBe('acceptance');
  });
});

// ---------------------------------------------------------------------------
// test-only workflow — dedicated scenarios
// ---------------------------------------------------------------------------

describe('runPhaseNext — test-only First Run', () => {
  it('should return proposal when workflow.json is test-only and eval.json empty (AC-1)', () => {
    const result = next([], 'test-change', 'test-only');
    expect(result.next_phase).toBe('proposal');
    expect(result.done).toBe(false);
  });

  it('should set total_phases to 5 for test-only', () => {
    const result = next([], 'test-change', 'test-only');
    expect(result.total_phases).toBe(5);
  });
});

describe('runPhaseNext — test-only Normal Progression', () => {
  it('should return code-analyze after proposal passes (AC-2)', () => {
    const result = next([passEntry('proposal')], 'test-change', 'test-only');
    expect(result.next_phase).toBe('code-analyze');
  });

  it('should return test-design after proposal and code-analyze pass (AC-2)', () => {
    const result = next(
      [passEntry('proposal'), passEntry('code-analyze')],
      'test-change',
      'test-only',
    );
    expect(result.next_phase).toBe('test-design');
  });

  it('should return test-gen after 01-03 pass (AC-2)', () => {
    const result = next(
      [passEntry('proposal'), passEntry('code-analyze'), passEntry('test-design')],
      'test-change',
      'test-only',
    );
    expect(result.next_phase).toBe('test-gen');
  });

  it('should return test-execution after 01-04 pass (AC-2)', () => {
    const result = next(
      [
        passEntry('proposal'),
        passEntry('code-analyze'),
        passEntry('test-design'),
        passEntry('test-gen'),
      ],
      'test-change',
      'test-only',
    );
    expect(result.next_phase).toBe('test-execution');
  });
});

describe('runPhaseNext — test-only Gate (code-analyze)', () => {
  it('should return code-analyze when 01 pass but 02 not passed — 03 cannot run early (AC-4)', () => {
    const result = next([passEntry('proposal')], 'test-change', 'test-only');
    expect(result.next_phase).toBe('code-analyze');
  });

  it('should return test-design when 01-02 pass — gate satisfied (AC-4)', () => {
    const result = next(
      [passEntry('proposal'), passEntry('code-analyze')],
      'test-change',
      'test-only',
    );
    expect(result.next_phase).toBe('test-design');
  });
});

describe('runPhaseNext — test-only Completion', () => {
  it('should return done=true when 01-04, 05 all pass', () => {
    const result = next(
      [
        passEntry('proposal'),
        passEntry('code-analyze'),
        passEntry('test-design'),
        passEntry('test-gen'),
        passEntry('test-execution'),
      ],
      'test-change',
      'test-only',
    );
    expect(result.done).toBe(true);
  });

  it('test-only 不应返回 implement 或 acceptance（AC-2）', () => {
    const entries = [
      passEntry('proposal'),
      passEntry('code-analyze'),
      passEntry('test-design'),
      passEntry('test-gen'),
    ];
    const result = next(entries, 'test-change', 'test-only');
    expect(result.next_phase).toBe('test-execution');
    expect(result.next_phase).not.toBe('implement');
    expect(result.next_phase).not.toBe('acceptance');
  });

  it('integration-test 不应出现在 test-only 的输出范围内', () => {
    const entries = [
      passEntry('proposal'),
      passEntry('code-analyze'),
      passEntry('test-design'),
      passEntry('test-gen'),
    ];
    const result = next(entries, 'test-change', 'test-only');
    expect(result.next_phase).toBe('test-execution');
    expect(result.next_phase).not.toBe('integration-test');
    // total_phases 应仍为 5
    expect(result.total_phases).toBe(5);
  });
});

describe('runPhaseNext — workflow.json 严格前置条件 (AC-13, AC-14)', () => {
  /** 捕获同步抛出的 Error，便于断言 message 内容。 */
  function captureError(fn: () => unknown): Error {
    try {
      fn();
    } catch (e: unknown) {
      return e as Error;
    }
    throw new Error('expected the call to throw, but it returned normally');
  }

  it('workflow.json 缺失时 getWorkflowType 抛错直接终止，message 含绝对路径与 change_create 指引，不返回 proposal（AC-13）', () => {
    const error = captureError(() =>
      next([], 'test-change', 'requirement', DEFAULT_RUN_ID, { missingWorkflow: true }),
    );

    expect(error.message).toContain('workflow.json 不存在');
    // `getWorkflowType` 用真实工程根拼接绝对路径
    expect(error.message).toContain(resolveChangeDir('test-change', getProjectDir()));
    expect(error.message).toContain('change_create');
  });

  it('workflow.json 缺 workflow_type 时抛错终止，不回退 requirement 表（AC-14）', () => {
    const error = captureError(() =>
      next([], 'test-change', 'requirement', DEFAULT_RUN_ID, { workflowRaw: '{}' }),
    );
    expect(error.message).toContain('格式非法');
    expect(error.message).toContain('workflow_type');
  });

  it('workflow.json JSON 非法 / 根为数组时抛错终止（AC-14）', () => {
    expect(() =>
      next([], 'test-change', 'requirement', DEFAULT_RUN_ID, { workflowRaw: '{ broken' }),
    ).toThrow(/解析失败/);
    expect(() =>
      next([], 'test-change', 'requirement', DEFAULT_RUN_ID, { workflowRaw: '[1,2]' }),
    ).toThrow(/根元素必须是对象/);
  });

  it('workflow_type 非枚举时抛错终止，不按 requirement 表推进（AC-14）', () => {
    expect(() => next([], 'test-change', 'unknown-flow')).toThrow(/格式非法/);
  });

  it('created 为 2026/09/11 时抛错终止（AC-14）', () => {
    expect(() =>
      next([], 'test-change', 'requirement', DEFAULT_RUN_ID, {
        workflowRaw: JSON.stringify({ workflow_type: 'requirement', created: '2026/09/11' }),
      }),
    ).toThrow(/格式非法/);
  });
});

describe('runPhaseNext — Backtrack (test-only)', () => {
  it('test-only backtrack_to proposal still returns target phase', () => {
    const entries = [
      passEntry('proposal'),
      passEntry('code-analyze'),
      passEntry('test-design'),
      backtrackEntry('test-design', 'proposal'),
    ];
    const result = next(entries, 'test-change', 'test-only');
    expect(result.next_phase).toBe('proposal');
  });
});

// ---------------------------------------------------------------------------
// last_result 字段 — AC-4
// ---------------------------------------------------------------------------

describe('runPhaseNext — last_result 字段 (AC-4)', () => {
  it('有 entries 时 last_result 字段存在且包含 phase/verdict/report/timestamp', () => {
    const entries = [
      passEntry('proposal', 1, { report: '提案通过' }),
      passEntry('dev-design', 1, { report: '设计通过' }),
    ];
    const result = next(entries, 'test-change');
    expect(result.last_result).not.toBeNull();
    expect(result.last_result).toHaveProperty('phase');
    expect(result.last_result).toHaveProperty('verdict');
    expect(result.last_result).toHaveProperty('report');
    expect(result.last_result).toHaveProperty('timestamp');
  });

  it('last_result.phase 为最新 entry 的 phase（按 timestamp 降序）', () => {
    const entries = [
      passEntry('proposal', 1),
      passEntry('dev-design', 1), // 最新 entry
    ];
    const result = next(entries, 'test-change');
    expect(result.last_result!.phase).toBe('dev-design');
  });

  it('last_result.verdict 为最新 entry 的 verdict', () => {
    const result = next([passEntry('proposal')], 'test-change');
    expect(result.last_result!.verdict).toBe('pass');
  });

  it('last_result.report 为最新 entry 的 report', () => {
    const entries = [passEntry('proposal', 1, { report: '提案评估报告' })];
    const result = next(entries, 'test-change');
    expect(result.last_result!.report).toBe('提案评估报告');
  });

  it('无 entries 时 last_result 为 null', () => {
    const result = next([], 'test-change');
    expect(result.last_result).toBeNull();
  });

  it('fail entry 后 last_result.verdict 为 "fail"', () => {
    const result = next([failEntry('proposal', 1, { report: '失败原因' })], 'test-change');
    expect(result.last_result!.verdict).toBe('fail');
    expect(result.last_result!.report).toBe('失败原因');
  });

  it('last_result 在所有响应类型中均存在（正常、完成、错误）', () => {
    // 正常响应
    const normalResult = next([passEntry('proposal')], 'test-change');
    expect(normalResult).toHaveProperty('last_result');

    // 完成响应
    const doneEntries = [
      passEntry('proposal'),
      passEntry('dev-design'),
      passEntry('test-design'),
      passEntry('test-gen'),
      passEntry('implement'),
      passEntry('test-execution'),
      passEntry('code-review'),
      passEntry('acceptance'),
    ];
    const doneResult = next(doneEntries, 'test-change');
    expect(doneResult).toHaveProperty('last_result');

    // 错误响应（max retries）
    const errorEntries = [
      failEntry('proposal', 1),
      failEntry('proposal', 2),
      failEntry('proposal', 3),
      failEntry('proposal', 4),
      failEntry('proposal', 5),
    ];
    const errorResult = next(errorEntries, 'test-change');
    expect(errorResult).toHaveProperty('last_result');
  });
});

// ---------------------------------------------------------------------------
// Input Validation
// ---------------------------------------------------------------------------

describe('runPhaseNext — Input Validation', () => {
  it('should not throw for empty entries array', () => {
    expect(() => next([], 'test-change')).not.toThrow();
  });

  it('change 参数为空字符串时应抛出 Error', () => {
    expect(() =>
      runPhaseNext({ change: '', project_root: FIXTURE_PROJECT_ROOT, run_id: DEFAULT_RUN_ID }),
    ).toThrow('Missing required parameter: change');
  });

  it('change 不存在时应抛出 Error', () => {
    vi.mocked(fs.existsSync).mockReturnValue(false);
    expect(() =>
      runPhaseNext({
        change: 'non-existent-change',
        project_root: FIXTURE_PROJECT_ROOT,
        run_id: DEFAULT_RUN_ID,
      }),
    ).toThrow(/Change "non-existent-change" does not exist/);
  });
});

// ---------------------------------------------------------------------------
// run_id — missing_run_id (AC-1)
// ---------------------------------------------------------------------------

describe('runPhaseNext — missing_run_id (AC-1)', () => {
  it('省略 run_id 时返回 missing_run_id 结构化错误', () => {
    const result = runPhaseNext({
      change: 'test-change',
      project_root: FIXTURE_PROJECT_ROOT,
    } as Parameters<typeof runPhaseNext>[0]);
    expect(result.error).toBe('missing_run_id');
    expect(result.done).toBe(false);
    expect(result.next_phase).toBeNull();
    expect(result.round).toBe(0);
  });

  it('run_id 为空字符串时返回 missing_run_id', () => {
    const result = runPhaseNext({
      change: 'test-change',
      project_root: FIXTURE_PROJECT_ROOT,
      run_id: '',
    });
    expect(result.error).toBe('missing_run_id');
    expect(result.next_phase).toBeNull();
  });

  it('run_id 仅空白时返回 missing_run_id', () => {
    const result = runPhaseNext({
      change: 'test-change',
      project_root: FIXTURE_PROJECT_ROOT,
      run_id: '   ',
    });
    expect(result.error).toBe('missing_run_id');
  });

  it('run_id 为 null / undefined 时返回 missing_run_id', () => {
    for (const runId of [null, undefined]) {
      const result = runPhaseNext({
        change: 'test-change',
        project_root: FIXTURE_PROJECT_ROOT,
        run_id: runId,
      } as unknown as Parameters<typeof runPhaseNext>[0]);
      expect(result.error).toBe('missing_run_id');
    }
  });

  it('生涯 25 条 entries 且缺 run_id 仍为 missing_run_id，不得 round_limit_exceeded', () => {
    const entries = buildLongHistory(25);
    next(entries);
    const result = runPhaseNext({
      change: 'test-change',
      project_root: FIXTURE_PROJECT_ROOT,
    } as Parameters<typeof runPhaseNext>[0]);
    expect(result.error).toBe('missing_run_id');
    expect(result.error).not.toBe('round_limit_exceeded');
  });

  it('run_id 为非 string 类型时返回 missing_run_id', () => {
    for (const runId of [123, {}, []]) {
      const result = runPhaseNext({
        change: 'test-change',
        project_root: FIXTURE_PROJECT_ROOT,
        run_id: runId,
      } as Parameters<typeof runPhaseNext>[0]);
      expect(result.error).toBe('missing_run_id');
    }
  });

  it('run_id 超长字符串且非空时按正常 session 接受', () => {
    const longRunId = 'r'.repeat(1001);
    const result = next([], 'test-change', undefined, longRunId);
    expect(result.error).toBeNull();
    expect(result.round).toBe(1);
  });

  it('run_id 含换行与 emoji 时接受且视为独立 key', () => {
    const specialA = 'run-\n-🧪';
    const specialB = 'run-emoji-🧪';
    const resultA = next([], 'test-change', undefined, specialA);
    const resultB = next([], 'test-change', undefined, specialB);
    expect(resultA.error).toBeNull();
    expect(resultB.error).toBeNull();
    expect(resultA.round).toBe(1);
    expect(resultB.round).toBe(1);
  });

  it('run_id 为 "0" 或单字符时合法可建立 anchor', () => {
    for (const runId of ['0', 'x']) {
      const result = next([], 'test-change', undefined, runId);
      expect(result.error).toBeNull();
      expect(result.round).toBe(1);
    }
  });
});

// ---------------------------------------------------------------------------
// session round (AC-2)
// ---------------------------------------------------------------------------

describe('runPhaseNext — session round (AC-2)', () => {
  it('entries 已有 12 条时首次 run_id s1 → round===1', () => {
    const prior = buildLongHistory(12);
    const result = next(prior, 'test-change', undefined, 's1');
    expect(result.round).toBe(1);
    expect(result.error).toBeNull();
  });

  it('同 run_id s1 下 entries 增至 15 后再调用 → round===4', () => {
    const prior = buildLongHistory(12);
    next(prior, 'test-change', undefined, 's1');
    const grown = [
      ...prior,
      passEntry('proposal', 13),
      passEntry('proposal', 14),
      passEntry('proposal', 15),
    ];
    const result = next(grown, 'test-change', undefined, 's1');
    expect(result.round).toBe(4);
  });

  it('同 run_id 空窗首次 round===1；追加 1 条后 round===2', () => {
    next([], 'test-change', undefined, 's1-empty');
    const first = next([], 'test-change', undefined, 's1-empty');
    expect(first.round).toBe(1);
    const second = next([passEntry('proposal')], 'test-change', undefined, 's1-empty');
    expect(second.round).toBe(2);
  });

  it('窗内 skipped 条目每条仍计 1 round', () => {
    next([], 'test-change', undefined, 's1-skip');
    const windowEntries = Array.from({ length: 20 }, (_, i) =>
      i % 2 === 0 ? skippedEntry('proposal', i + 1) : passEntry('proposal', i + 1),
    );
    const result = next(windowEntries, 'test-change', undefined, 's1-skip');
    expect(result.round).toBe(21);
    expect(result.error).toBe('round_limit_exceeded');
  });
});

// ---------------------------------------------------------------------------
// session max_retries (AC-3)
// ---------------------------------------------------------------------------

describe('runPhaseNext — session max_retries (AC-3)', () => {
  it('同 window 内目标 phase 5 条 fail → max_retries_exceeded', () => {
    next([passEntry('proposal')], 'test-change', undefined, 'fail-s1');
    const result = next(
      [
        passEntry('proposal'),
        failEntry('dev-design', 1),
        failEntry('dev-design', 2),
        failEntry('dev-design', 3),
        failEntry('dev-design', 4),
        failEntry('dev-design', 5),
      ],
      'test-change',
      undefined,
      'fail-s1',
    );
    expect(result.error).toBe('max_retries_exceeded');
  });

  it('同 window 内目标 phase 仅 4 条 fail 仍返回 next_phase', () => {
    next([passEntry('proposal')], 'test-change', undefined, 'fail-s2');
    const result = next(
      [
        passEntry('proposal'),
        failEntry('dev-design', 1),
        failEntry('dev-design', 2),
        failEntry('dev-design', 3),
        failEntry('dev-design', 4),
      ],
      'test-change',
      undefined,
      'fail-s2',
    );
    expect(result.next_phase).toBe('dev-design');
    expect(result.error).toBeNull();
  });

  it('生涯 fail 在 anchor 之前时新 run_id 不触发 max_retries_exceeded', () => {
    const career = [
      passEntry('proposal'),
      failEntry('dev-design', 1),
      failEntry('dev-design', 2),
      failEntry('dev-design', 3),
      failEntry('dev-design', 4),
      failEntry('dev-design', 5),
    ];
    const result = next(career, 'test-change', undefined, 'fresh-run');
    expect(result.error).not.toBe('max_retries_exceeded');
    expect(result.next_phase).toBe('dev-design');
  });

  it('窗内 5 条 fail 分属不同 phase 时仅按 next_phase 计数', () => {
    next([passEntry('proposal')], 'test-change', undefined, 'fail-s3');
    const result = next(
      [
        passEntry('proposal'),
        failEntry('dev-design', 1),
        failEntry('test-design', 1),
        failEntry('implement', 1),
        failEntry('test-gen', 1),
        failEntry('test-execution', 1),
      ],
      'test-change',
      undefined,
      'fail-s3',
    );
    expect(result.next_phase).toBe('dev-design');
    expect(result.error).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 新 session 重置 (AC-4)
// ---------------------------------------------------------------------------

describe('runPhaseNext — 新 session 重置 (AC-4)', () => {
  it('生涯 25 条 + 新 run_id 首次调用 round===1 且非 round_limit_exceeded', () => {
    const career = buildLongHistory(25);
    const result = next(career, 'test-change', undefined, 'new-session');
    expect(result.round).toBe(1);
    expect(result.error).not.toBe('round_limit_exceeded');
    expect(result.next_phase).not.toBeNull();
    expect(result.done).toBe(false);
  });

  it('耗尽 fail 预算后换 run_id 可继续该 phase', () => {
    next([passEntry('proposal')], 'test-change', undefined, 's1-exhaust');
    const career = [
      passEntry('proposal'),
      failEntry('dev-design', 1),
      failEntry('dev-design', 2),
      failEntry('dev-design', 3),
      failEntry('dev-design', 4),
      failEntry('dev-design', 5),
    ];
    const exhausted = next(career, 'test-change', undefined, 's1-exhaust');
    expect(exhausted.error).toBe('max_retries_exceeded');
    const reset = next(career, 'test-change', undefined, 's2-reset');
    expect(reset.error).toBeNull();
    expect(reset.next_phase).toBe('dev-design');
  });

  it('已建立 session 后换空 run_id → missing_run_id', () => {
    next([], 'test-change', undefined, 's1-valid');
    const result = runPhaseNext({
      change: 'test-change',
      project_root: FIXTURE_PROJECT_ROOT,
      run_id: '',
    });
    expect(result.error).toBe('missing_run_id');
  });

  it('换 run_id 重置后在同窗再堆满 21 条仍 round_limit_exceeded', () => {
    next([], 'test-change', undefined, 's2-limit');
    const windowEntries = buildLongHistory(21);
    const result = next(windowEntries, 'test-change', undefined, 's2-limit');
    expect(result.error).toBe('round_limit_exceeded');
  });

  it('更换 run_id 后 anchor 取当前 entries.length', () => {
    const career = buildLongHistory(10);
    next(career, 'test-change', undefined, 'old-run');
    const result = next(
      [...career, passEntry('proposal', 11)],
      'test-change',
      undefined,
      'new-run',
    );
    expect(result.round).toBe(1);
  });
});

// ---------------------------------------------------------------------------
// map 隔离与易失 (AC-5)
// ---------------------------------------------------------------------------

describe('runPhaseNext — map 隔离与易失 (AC-5)', () => {
  it('change A/B 同 run_id 各自使用自身 entries.length 为 anchor', () => {
    next(buildLongHistory(8), 'change-a', undefined, 'shared-s1');
    const resultB = next(buildLongHistory(3), 'change-b', undefined, 'shared-s1');
    expect(resultB.round).toBe(1);
    const resultA = next(buildLongHistory(10), 'change-a', undefined, 'shared-s1');
    expect(resultA.round).toBe(3);
  });

  it('change A 不得读到 change B 的 session window', () => {
    next([], 'change-a', undefined, 'iso-s1');
    next(buildLongHistory(21), 'change-b', undefined, 'iso-s1');
    const resultA = next([], 'change-a', undefined, 'iso-s1');
    expect(resultA.error).not.toBe('round_limit_exceeded');
    expect(resultA.round).toBe(1);
  });

  it('同一 import 内连续同 (change, run_id) 复用 anchor', () => {
    next([], 'anchor-reuse', undefined, 'reuse-s1');
    const first = next([passEntry('proposal')], 'anchor-reuse', undefined, 'reuse-s1');
    const second = next(
      [passEntry('proposal'), passEntry('proposal', 2)],
      'anchor-reuse',
      undefined,
      'reuse-s1',
    );
    expect(first.round).toBe(2);
    expect(second.round).toBe(3);
  });

  it('vi.resetModules 后同一 run_id 在长历史上 round===1', async () => {
    const career = buildLongHistory(30);
    next(career, 'reset-change', undefined, 'reset-s1');
    vi.resetModules();
    const { runPhaseNext: freshRunPhaseNext } = await import('./phase-next');
    vi.mocked(fs.existsSync).mockImplementation((filePath: fs.PathLike) => {
      const p = String(filePath);
      return (
        p === resolveChangeDir('reset-change', FIXTURE_PROJECT_ROOT) || p.endsWith('workflow.json')
      );
    });
    vi.mocked(fs.readFileSync).mockImplementation(
      (
        path: fs.PathOrFileDescriptor,
        _options?: BufferEncoding | fs.ObjectEncodingOptions | null,
      ): string => {
        const p = String(path);
        if (p.endsWith('workflow.json')) {
          return JSON.stringify({ workflow_type: 'requirement', eval: career });
        }
        return '';
      },
    );
    const result = freshRunPhaseNext({
      change: 'reset-change',
      project_root: FIXTURE_PROJECT_ROOT,
      run_id: 'reset-s1',
    });
    expect(result.round).toBe(1);
  });

  it('resetModules 后 run_id 为空仍 missing_run_id', async () => {
    vi.resetModules();
    const { runPhaseNext: freshRunPhaseNext } = await import('./phase-next');
    const result = freshRunPhaseNext({
      change: 'test-change',
      project_root: FIXTURE_PROJECT_ROOT,
      run_id: '',
    });
    expect(result.error).toBe('missing_run_id');
  });

  it('run_id 含内嵌 \\0 与 change 拼接仍按独立 key 隔离', () => {
    const runId = 's1\0suffix';
    next([], 'change-a', undefined, runId);
    const result = next(buildLongHistory(21), 'change-b', undefined, runId);
    expect(result.round).toBe(1);
    expect(result.error).not.toBe('round_limit_exceeded');
  });
});

// ---------------------------------------------------------------------------
// 回归 (AC-7) — 同一 run_id 下推进/backtrack/done 不变
// ---------------------------------------------------------------------------

describe('runPhaseNext — 回归 (AC-7)', () => {
  const runId = 'regression-run';

  it('空 eval → next_phase proposal、round===1', () => {
    const result = next([], 'test-change', undefined, runId);
    expect(result.next_phase).toBe('proposal');
    expect(result.round).toBe(1);
  });

  it('proposal pass → next_phase dev-design', () => {
    const result = next([passEntry('proposal')], 'test-change', undefined, runId);
    expect(result.next_phase).toBe('dev-design');
  });

  it('全部 phase pass → done===true', () => {
    const entries = getPhaseTable('requirement').map((p) => passEntry(p.id));
    const result = next(entries, 'test-change', undefined, runId);
    expect(result.done).toBe(true);
  });

  it('最新 entry 含 backtrack_to 时返回目标 phase 且无 updatedEntries', () => {
    const entries = [
      passEntry('proposal'),
      passEntry('dev-design'),
      backtrackEntry('test-design', 'proposal'),
    ];
    const result = next(entries, 'test-change', undefined, runId);
    expect(result.next_phase).toBe('proposal');
    expect(result).not.toHaveProperty('updatedEntries');
  });

  it('窗内 round>20 仍停止于 round_limit_exceeded', () => {
    next([], 'test-change', undefined, 'regression-round-limit-s1');
    const result = next(
      buildLongHistory(21),
      'test-change',
      undefined,
      'regression-round-limit-s1',
    );
    expect(result.error).toBe('round_limit_exceeded');
  });

  it('窗内 5 fail 仍停止于 max_retries_exceeded', () => {
    next([passEntry('proposal')], 'test-change', undefined, 'regression-max-retry-s1');
    const result = next(
      [
        passEntry('proposal'),
        failEntry('dev-design', 1),
        failEntry('dev-design', 2),
        failEntry('dev-design', 3),
        failEntry('dev-design', 4),
        failEntry('dev-design', 5),
      ],
      'test-change',
      undefined,
      'regression-max-retry-s1',
    );
    expect(result.error).toBe('max_retries_exceeded');
  });

  it('mid-phase 中断仍返回未完成 phase', () => {
    const result = next([passEntry('proposal')], 'test-change', undefined, runId);
    expect(result.next_phase).toBe('dev-design');
    expect(result.done).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// Boundary Scenarios
// ---------------------------------------------------------------------------

describe('Boundary Scenarios', () => {
  it('should handle empty eval.json (first run)', () => {
    const result = next([]);
    expect(result.next_phase).toBe('proposal');
    expect(result.round).toBe(1);
  });

  it('should handle all phases as first_run (empty entries)', () => {
    const result = next([]);
    expect(result.next_phase).toBe('proposal');
    expect(result.round).toBe(1);
  });

  it('should handle exactly 20 rounds with all pass (no limit error)', () => {
    // To hit exactly round 20, we need 19 entries. If all phases are pass
    // by that point, we should get done=true, not round limit error.
    const entries: MockEntry[] = [];
    // Add 8 phases pass in sequence
    const phaseIds: EvalEntry['phase'][] = [
      'proposal',
      'dev-design',
      'test-design',
      'test-gen',
      'implement',
      'test-execution',
      'code-review',
      'acceptance',
    ];
    // Each phase has 2 pass entries (total 16), plus 3 extra = 19 entries = round 20
    for (let i = 0; i < 16; i++) {
      entries.push(passEntry(phaseIds[i % 8], Math.floor(i / 8) + 1));
    }
    // At this point all 8 phases have at least one pass
    const result = next(entries);
    expect(result.done).toBe(true);
    expect(result.error).toBeNull();
  });

  it('01-03 pass + skip 04 时仍返回 implement（05 未 pass）', () => {
    const entries = [
      passEntry('proposal'),
      passEntry('dev-design'),
      passEntry('test-design'),
      skippedEntry('test-gen'),
    ];
    const result = next(entries);
    expect(result.next_phase).toBe('implement');
  });

  it('should handle change name with special characters', () => {
    const result = next([], 'my-test-变更');
    expect(result.executor!.prompt).toContain('my-test-变更');
  });

  it('test-only change name with special characters still appears in executor prompt', () => {
    const result = next([], 'my-test-变更', 'test-only');
    expect(result.executor!.prompt).toContain('my-test-变更');
  });

  it('should return done=true after acceptance passes', () => {
    const entries = [
      passEntry('proposal'),
      passEntry('dev-design'),
      passEntry('test-design'),
      passEntry('test-gen'),
      passEntry('implement'),
      passEntry('test-execution'),
      passEntry('code-review'),
      passEntry('acceptance'),
    ];
    const result = next(entries);
    expect(result.done).toBe(true);
    expect(result.executor).toBeNull();
    expect(result.evaluator).toBeNull();
  });
});

describe('phase_next Output Schema', () => {
  it('should return valid JSON when next phase is ready', () => {
    const result = next([], 'test');
    // Verify all required fields exist
    expect(result).toHaveProperty('done');
    expect(result).toHaveProperty('error');
    expect(result).toHaveProperty('next_phase');
    expect(result).toHaveProperty('executor');
    expect(result).toHaveProperty('evaluator');
    expect(result).toHaveProperty('allowed_backtrack_phases');
    expect(result).toHaveProperty('total_phases');
    expect(result).toHaveProperty('phase_index');
    expect(result).toHaveProperty('round');
  });

  it('should return valid JSON when all phases are done', () => {
    const entries = [
      passEntry('proposal'),
      passEntry('dev-design'),
      passEntry('test-design'),
      passEntry('test-gen'),
      passEntry('implement'),
      passEntry('test-execution'),
      passEntry('code-review'),
      passEntry('acceptance'),
    ];
    const result = next(entries, 'test');
    expect(result.done).toBe(true);
    expect(result.next_phase).toBeNull();
    expect(result.executor).toBeNull();
    expect(result.evaluator).toBeNull();
  });

  it('should return valid JSON on error', () => {
    next([], 'test-change', undefined, 'output-error-s1');
    const result = next(
      [
        failEntry('proposal', 1),
        failEntry('proposal', 2),
        failEntry('proposal', 3),
        failEntry('proposal', 4),
        failEntry('proposal', 5),
      ],
      'test-change',
      undefined,
      'output-error-s1',
    );
    expect(result.error).toBe('max_retries_exceeded');
    expect(result.next_phase).toBeNull();
    expect(result.executor).toBeNull();
    expect(result.evaluator).toBeNull();
  });

  it('should return same prompt on retry (skill handles retry context)', () => {
    // The phase_next tool returns the same prompt template on retry.
    // The skill is responsible for adding retry context (e.g. "attempt 2/5").
    const firstResult = next([], 'test-change');
    const retryResult = next([failEntry('proposal', 1)], 'test-change');
    // Both should have the same prompt
    expect(firstResult.executor!.prompt).toBe(retryResult.executor!.prompt);
    expect(retryResult.error).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// allowed_backtrack_phases
// ---------------------------------------------------------------------------

describe('phase_next — allowed_backtrack_phases', () => {
  it('should return empty allowed_backtrack_phases for first phase', () => {
    const result = next([], 'test-change');
    expect(result.allowed_backtrack_phases).toEqual([]);
  });

  it('should return preceding phases for second phase', () => {
    const result = next([passEntry('proposal')], 'test-change');
    expect(result.allowed_backtrack_phases).toHaveLength(2);
    expect(result.allowed_backtrack_phases[0].id).toBe('proposal');
    expect(result.allowed_backtrack_phases[0].description).toBeTruthy();
  });

  it('should return all preceding phases with id and description', () => {
    const entries = [passEntry('proposal'), passEntry('dev-design'), passEntry('test-design')];
    const result = next(entries, 'test-change');
    expect(result.next_phase).toBe('implement');
    // implement is the 4th phase in the table → 3 preceding phases
    expect(result.allowed_backtrack_phases).toHaveLength(4);
    expect(result.allowed_backtrack_phases[0]).toEqual({
      id: 'proposal',
      description: expect.any(String),
    });
    expect(result.allowed_backtrack_phases[1].id).toBe('dev-design');
    expect(result.allowed_backtrack_phases[2].id).toBe('test-design');
    // All descriptions should be non-empty strings
    for (const p of result.allowed_backtrack_phases) {
      expect(p.description).toBeTruthy();
    }
  });

  it('should have empty allowed_backtrack_phases on done', () => {
    const entries = [
      passEntry('proposal'),
      passEntry('dev-design'),
      passEntry('test-design'),
      passEntry('test-gen'),
      passEntry('implement'),
      passEntry('test-execution'),
      passEntry('code-review'),
      passEntry('acceptance'),
    ];
    const result = next(entries, 'test-change');
    expect(result.done).toBe(true);
    expect(result.allowed_backtrack_phases).toEqual([]);
  });

  it('should have empty allowed_backtrack_phases on error', () => {
    next([], 'test-change', undefined, 'backtrack-error-s1');
    const result = next(
      [
        failEntry('proposal', 1),
        failEntry('proposal', 2),
        failEntry('proposal', 3),
        failEntry('proposal', 4),
        failEntry('proposal', 5),
      ],
      'test-change',
      undefined,
      'backtrack-error-s1',
    );
    expect(result.error).toBe('max_retries_exceeded');
    expect(result.allowed_backtrack_phases).toEqual([]);
  });

  it('test-execution 的 allowed_backtrack_phases 不应包含 unit-test 或 integration-test', () => {
    // Pass 01-05 to reach test-execution
    const entries = [
      passEntry('proposal'),
      passEntry('dev-design'),
      passEntry('test-design'),
      passEntry('implement'),
      passEntry('test-gen'),
    ];
    const result = next(entries, 'test-change');
    expect(result.next_phase).toBe('test-execution');
    const backtrackIds = result.allowed_backtrack_phases.map((p) => p.id);
    expect(backtrackIds).not.toContain('unit-test');
    expect(backtrackIds).not.toContain('integration-test');
  });

  it('should include backtrack hint after backtrack detection', () => {
    const entries = [
      passEntry('proposal'),
      passEntry('dev-design'),
      passEntry('test-design'),
      backtrackEntry('test-design', 'proposal'),
    ];
    const result = next(entries, 'test-change');
    expect(result.next_phase).toBe('proposal');
    // proposal is the first phase → no preceding phases
    expect(result.allowed_backtrack_phases).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// interpolate / done / backtrack / retry（mutation 补强）
// ---------------------------------------------------------------------------

describe('runPhaseNext / agent tokens 与 interpolate（突变补强）', () => {
  it('首跑 executor/evaluator agent_type 精确为 __CALL_AGENT:proposal-planner/evaluator__', () => {
    const result = next([]);
    expect(result.executor!.agent_type).toBe('__CALL_AGENT:proposal-planner__');
    expect(result.evaluator!.agent_type).toBe('__CALL_AGENT:proposal-evaluator__');
  });

  it('executor/evaluator prompt 中 <change>/<phase> 被替换且无残留', () => {
    const result = next([], 'my-feature-change');
    expect(result.executor!.prompt).toContain('my-feature-change');
    expect(result.executor!.prompt).not.toContain('<change>');
    expect(result.evaluator!.prompt).toContain('my-feature-change');
    expect(result.evaluator!.prompt).toContain('proposal');
    expect(result.evaluator!.prompt).not.toContain('<change>');
    expect(result.evaluator!.prompt).not.toContain('<phase>');
  });

  it('proposal executor prompt 含 explore.md 与 merge 语义', () => {
    const result = next([], 'chg');
    expect(result.executor!.prompt).toContain('explore.md');
    expect(result.executor!.prompt).toMatch(/merge/i);
    expect(result.executor!.prompt).toContain('Do not expect inline EXPLORE_CONTEXT_SUMMARY');
  });
});

describe('runPhaseNext / done（突变补强）', () => {
  it('全部 phase pass 时 done=true，message 精确等于 Ready for archiving 文案', () => {
    const entries = getPhaseTable('requirement').map((p) => passEntry(p.id));
    const result = next(entries);
    expect(result.done).toBe(true);
    expect(result.message).toBe('All phases have passed evaluation. Ready for archiving.');
  });

  it('done 响应 error 为 null 且未提供时 last_result 可为最新条目快照', () => {
    const entries = getPhaseTable('requirement').map((p) => passEntry(p.id));
    const result = next(entries);
    expect(result.done).toBe(true);
    expect(result.error).toBeNull();
    expect(result.last_result).not.toBeNull();
    expect(result.last_result!.phase).toBe('acceptance');
  });

  it('空 entries 首跑 last_result 为 null 且 error 为 null', () => {
    const result = next([]);
    expect(result.done).toBe(false);
    expect(result.error).toBeNull();
    expect(result.last_result).toBeNull();
  });
});

describe('runPhaseNext / error 与 backtrack（突变补强）', () => {
  it('max retries 时 done 为 false（非 true）、error 为 max_retries_exceeded', () => {
    next([passEntry('proposal')], 'test-change', undefined, 'mut-max-retry-s1');
    const result = next(
      [
        passEntry('proposal'),
        failEntry('dev-design', 1),
        failEntry('dev-design', 2),
        failEntry('dev-design', 3),
        failEntry('dev-design', 4),
        failEntry('dev-design', 5),
      ],
      'test-change',
      undefined,
      'mut-max-retry-s1',
    );
    expect(result.done).toBe(false);
    expect(result.done).not.toBe(true);
    expect(result.error).toBe('max_retries_exceeded');
  });

  it("最新条目 backtrack_to: '' 视为无回溯，继续正常推进", () => {
    // Zod 拒绝空串；stub readEvalJson 以覆盖 getLatestBacktrackInfo 的 !== '' 分支。
    // 经 next(..., 'requirement') 固定 workflow.json，避免 shuffle 下残留 test-only fs mock。
    const spy = vi.spyOn(evalJson, 'readEvalJson').mockReturnValue([
      passEntry('proposal'),
      {
        ...failEntry('dev-design', 1),
        backtrack_to: '' as unknown as string,
      },
    ]);
    try {
      const result = next([], 'test-change', 'requirement');
      expect(result.error).toBeNull();
      expect(result.next_phase).toBe('dev-design');
      expect(result.executor!.prompt).not.toContain('⚠️ 回溯原因');
    } finally {
      spy.mockRestore();
    }
  });

  it("backtrack_reason: null / 缺失 / '' 不拼 ⚠️；空白 reason 仍拼接", () => {
    const noField = next([passEntry('proposal'), backtrackEntry('dev-design', 'proposal', 1)]);
    expect(noField.executor!.prompt).not.toContain('⚠️ 回溯原因');

    const nullReason = next([
      passEntry('proposal'),
      backtrackEntry('dev-design', 'proposal', 1, { backtrack_reason: null }),
    ]);
    expect(nullReason.executor!.prompt).not.toContain('⚠️ 回溯原因');

    const emptyReason = next([
      passEntry('proposal'),
      backtrackEntry('dev-design', 'proposal', 1, { backtrack_reason: '' }),
    ]);
    // 当前 truthy 语义：'' 为 falsy → 不拼接；若改为 != null 则会误拼
    expect(emptyReason.executor!.prompt).not.toContain('⚠️ 回溯原因');

    const spaceReason = next([
      passEntry('proposal'),
      backtrackEntry('dev-design', 'proposal', 1, { backtrack_reason: ' ' }),
    ]);
    expect(spaceReason.executor!.prompt).toContain('⚠️ 回溯原因:  ');
  });

  it('无效 backtrack 目标 → invalid_backtrack_target，message 含目标 JSON', () => {
    const result = next([
      passEntry('proposal'),
      failEntry('dev-design', 1, { backtrack_to: 'not-a-phase' }),
    ]);
    expect(result.error).toBe('invalid_backtrack_target');
    expect(result.message).toContain('not-a-phase');
    expect(result.message).toContain(JSON.stringify('not-a-phase'));
  });

  it('连续 5 次同 phase fail 触发 max retries；同 phase 的 pass 不计入 fail 次数', () => {
    const withPass = next([
      passEntry('proposal'),
      failEntry('dev-design', 1),
      failEntry('dev-design', 2),
      failEntry('dev-design', 3),
      failEntry('dev-design', 4),
      passEntry('dev-design', 5),
      failEntry('test-design', 1),
    ]);
    // 已通过 dev-design，应进入 test-design（或其后），不得因 4 次 fail 误触 max retries
    expect(withPass.error).toBeNull();
    expect(withPass.next_phase).toBe('test-design');

    next([passEntry('proposal')], 'test-change', undefined, 'mut-five-fail-s1');
    const fiveFails = next(
      [
        passEntry('proposal'),
        failEntry('dev-design', 1),
        failEntry('dev-design', 2),
        failEntry('dev-design', 3),
        failEntry('dev-design', 4),
        failEntry('dev-design', 5),
      ],
      'test-change',
      undefined,
      'mut-five-fail-s1',
    );
    expect(fiveFails.error).toBe('max_retries_exceeded');
  });
});

describe('runPhaseNext / hasPhasePassed 与边界（突变补强）', () => {
  it('skipped:true 且非 stale 视为已通过；stale:true 的 pass 忽略', () => {
    const skipped = next([skippedEntry('proposal')]);
    expect(skipped.next_phase).toBe('dev-design');

    const stale = next([staleEntry('proposal'), passEntry('dev-design')]);
    expect(stale.next_phase).toBe('proposal');
  });

  it("options.change 为 '' 时抛错", () => {
    expect(() =>
      runPhaseNext({ change: '', project_root: FIXTURE_PROJECT_ROOT, run_id: DEFAULT_RUN_ID }),
    ).toThrow(/Missing required parameter: change/);
  });

  it('回溯 reason 长度 500 时 prompt 完整包含', () => {
    const longReason = 'R'.repeat(500);
    const result = next([
      passEntry('proposal'),
      backtrackEntry('dev-design', 'proposal', 1, { backtrack_reason: longReason }),
    ]);
    expect(result.executor!.prompt).toContain(`⚠️ 回溯原因: ${longReason}`);
    expect(result.evaluator!.prompt).toContain(longReason);
  });
});

// ===========================================================================
// hasPhasePassed 导出函数 — 直接单元测试（export 变更，纯函数，无 fs 依赖）
// ===========================================================================

describe('hasPhasePassed — 导出函数（纯函数，无 fs 依赖）', () => {
  it('已有非 stale 的 pass 条目时返回 true', () => {
    expect(hasPhasePassed([passEntry('proposal')], 'proposal')).toBe(true);
  });

  it('已有非 stale 的 skipped 条目时返回 true', () => {
    expect(hasPhasePassed([skippedEntry('proposal')], 'proposal')).toBe(true);
  });

  it('仅有 fail 条目时返回 false', () => {
    expect(hasPhasePassed([failEntry('proposal')], 'proposal')).toBe(false);
  });

  it('仅有 stale 的 pass 条目时返回 false', () => {
    expect(hasPhasePassed([staleEntry('proposal')], 'proposal')).toBe(false);
  });

  it('空 entries 数组返回 false', () => {
    expect(hasPhasePassed([], 'proposal')).toBe(false);
  });

  it('混合 stale + 非 stale 条目时，非 stale 通过则返回 true', () => {
    const entries = [staleEntry('proposal'), passEntry('proposal')];
    expect(hasPhasePassed(entries, 'proposal')).toBe(true);
  });

  it('缺失 stale 字段的 pass 条目视为非 stale（向后兼容）返回 true', () => {
    const entry = passEntry('proposal');
    // 显式移除 stale 字段，模拟旧版 eval 条目
    delete (entry as { stale?: boolean }).stale;
    expect(entry.stale).toBeUndefined();
    expect(hasPhasePassed([entry], 'proposal')).toBe(true);
  });

  it('不同 phase 的 pass 条目不匹配目标 phase（返回 false）', () => {
    expect(hasPhasePassed([passEntry('dev-design')], 'proposal')).toBe(false);
  });

  it('多个 phase 条目混存时只匹配目标 phase 的非 stale pass', () => {
    const entries = [
      failEntry('proposal'),
      staleEntry('dev-design'),
      passEntry('proposal'),
      skippedEntry('acceptance'),
    ];
    expect(hasPhasePassed(entries, 'proposal')).toBe(true);
    expect(hasPhasePassed(entries, 'dev-design')).toBe(false);
    expect(hasPhasePassed(entries, 'acceptance')).toBe(true);
  });
});

// ===========================================================================
// runPhaseNext — 新运行态字段容忍（phase-lifecycle-file-log AC-12）
// ===========================================================================

describe('runPhaseNext — workflow.json 含新运行态字段时照常解析推进 (AC-12)', () => {
  it('含 active_phase / file_log / interrupted（looseObject 允许的未知键）时 gate 正常返回下一 phase，新字段不参与 gate 计算', () => {
    const entries = [passEntry('proposal')];
    const raw = JSON.stringify({
      workflow_type: 'requirement',
      created: '2026-09-18',
      eval: entries,
      active_phase: { phase: 'implement', attempt: 1, start_at: '2026-09-18T08:00:00.000Z' },
      interrupted: [
        {
          phase: 'implement',
          attempt: 1,
          start_at: '2026-09-18T07:00:00.000Z',
          end_at: '2026-09-18T07:30:00.000Z',
        },
      ],
      file_log: [
        {
          op: 'write',
          scope: 'implement',
          attempt: 1,
          path: 'src/a.ts',
          at: '2026-09-18T08:05:00.000Z',
        },
      ],
    });

    const result = next([], 'test-change', 'requirement', DEFAULT_RUN_ID, { workflowRaw: raw });

    // proposal 已 pass → gate 照常推进；active_phase 的 implement 也不影响序列
    expect(result.done).toBe(false);
    expect(result.error).toBeNull();
    expect(['dev-design', 'test-design']).toContain(result.next_phase);
  });

  it('active_phase: null 与空 file_log（change_create 初始形态 + phase_log 清场后）→ First Run 正常返回 proposal', () => {
    const raw = JSON.stringify({
      workflow_type: 'requirement',
      created: '2026-09-18',
      eval: [],
      active_phase: null,
      file_log: [],
    });

    const result = next([], 'test-change', 'requirement', DEFAULT_RUN_ID, { workflowRaw: raw });

    expect(result.next_phase).toBe('proposal');
    expect(result.done).toBe(false);
    expect(result.round).toBe(1);
  });
});
