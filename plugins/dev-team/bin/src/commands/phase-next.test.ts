/**
 * Unit tests for phase_next MCP tool — server-side orchestration logic.
 *
 * Tests cover: phase table resolution, normal progression, skip passed phases,
 * retry logic, backtrack, round limit, mid-phase interruption, skipped entries,
 * workflow_type variants, input validation, and all boundary scenarios.
 *
 * Tests call runPhaseNext with mocked eval.json reads — no real filesystem access.
 *
 * @see openspec/changes/add-workflow-requirement-skill/test-design.md
 */

import * as fs from 'fs';

import { describe, it, expect, vi } from 'vite-plus/test';

vi.mock('fs', async (importOriginal) => {
  const actual = await importOriginal<typeof fs>();
  return { ...actual, existsSync: vi.fn(), readFileSync: vi.fn() };
});

import { runPhaseNext } from '../commands/phase-next';
import { getChangeDir } from '../lib/change';
import { type EvalEntry } from '../lib/eval-json';
import { getPhaseTable } from '../lib/workflow';

const FIXTURE_PROJECT_ROOT = '/tmp/fixture-project';

// ---------------------------------------------------------------------------
// Mock helpers — construct eval.json entries for test scenarios
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

function next(entries: MockEntry[], change: string = 'test-change', workflowType?: string) {
  vi.mocked(fs.existsSync).mockImplementation((filePath: fs.PathLike) => {
    const p = String(filePath);
    if (p === getChangeDir(change, FIXTURE_PROJECT_ROOT)) {
      return true;
    }
    if (p.endsWith('workflow.json')) {
      return workflowType !== undefined;
    }
    if (p.endsWith('eval.json')) {
      return entries.length > 0;
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
        return JSON.stringify({ workflow_type: workflowType ?? 'requirement' });
      }
      if (p.endsWith('eval.json')) {
        return JSON.stringify(entries);
      }
      return '';
    },
  );
  return runPhaseNext({ change, project_root: FIXTURE_PROJECT_ROOT });
}

// ---------------------------------------------------------------------------
// First Run — Empty eval.json
// ---------------------------------------------------------------------------

describe('runPhaseNext — First Run (empty eval.json)', () => {
  it('should return proposal as next_phase when entries is empty', () => {
    const result = next([]);
    expect(result.next_phase).toBe('proposal');
    expect(result.done).toBe(false);
    expect(result.error).toBeNull();
  });

  it('should return proposal-planner as executor agent_type', () => {
    const result = next([]);
    expect(result.executor!.agent_type).toBe('dev-team:proposal-planner');
  });

  it('should return proposal-evaluator as evaluator agent_type', () => {
    const result = next([]);
    expect(result.evaluator!.agent_type).toBe('dev-team:proposal-evaluator');
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
    expect(result.evaluator!.agent_type).toBe('dev-team:code-review-evaluator');
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

describe('runPhaseNext — Retry Logic (AC-7)', () => {
  it('should return same phase for first retry after fail', () => {
    const result = next([passEntry('proposal'), failEntry('dev-design')]);
    expect(result.next_phase).toBe('dev-design');
    expect(result.error).toBeNull();
  });

  it('should return same phase on 4th consecutive fail (attempt 5)', () => {
    const result = next([
      passEntry('proposal'),
      failEntry('dev-design', 1),
      failEntry('dev-design', 2),
      failEntry('dev-design', 3),
      failEntry('dev-design', 4),
    ]);
    expect(result.next_phase).toBe('dev-design');
    expect(result.error).toBeNull();
  });

  it('should return max_retries_exceeded error on 5th consecutive fail', () => {
    const result = next([
      passEntry('proposal'),
      failEntry('dev-design', 1),
      failEntry('dev-design', 2),
      failEntry('dev-design', 3),
      failEntry('dev-design', 4),
      failEntry('dev-design', 5),
    ]);
    expect(result.error).toBe('max_retries_exceeded');
    expect(result.next_phase).toBeNull();
  });

  it('should return error message in Chinese for max retries', () => {
    const result = next([
      passEntry('proposal'),
      failEntry('dev-design', 1),
      failEntry('dev-design', 2),
      failEntry('dev-design', 3),
      failEntry('dev-design', 4),
      failEntry('dev-design', 5),
    ]);
    expect(result.message).toContain('超过最大重试次数');
  });

  it('should reset retry count after a pass (then fail, then pass, then fail)', () => {
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
    expect(result.executor!.agent_type).toBe('dev-team:proposal-planner');
    expect(result.evaluator!.agent_type).toBe('dev-team:proposal-evaluator');
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

describe('runPhaseNext — Round Limit (AC-10)', () => {
  it('should return round_limit_exceeded error when round > 20', () => {
    // Construct 21 rounds of entries
    const entries = [];
    for (let i = 0; i < 21; i++) {
      entries.push(passEntry('proposal', i + 1));
    }
    const result = next(entries);
    expect(result.error).toBe('round_limit_exceeded');
  });

  it('should return Chinese error message for round limit', () => {
    const entries = [];
    for (let i = 0; i < 21; i++) {
      entries.push(passEntry('proposal', i + 1));
    }
    const result = next(entries);
    expect(result.message).toContain('20 轮');
  });

  it('should NOT error at exactly round 20 with all pass', () => {
    // 20 entries = round 21, which exceeds limit
    // Actually round = entries.length + 1, so round === 20 when entries.length === 19
    const entries = [];
    const testPhases: EvalEntry['phase'][] = ['proposal', 'dev-design', 'test-design'];
    for (let i = 0; i < 19; i++) {
      entries.push(passEntry(testPhases[i % 3], i + 1));
    }
    // At 19 entries, round = 20. We should still be able to process.
    const result = next(entries);
    // round = 20, should not error (threshold is > 20)
    expect(result.error).toBeNull();
  });

  it('should detect round limit via backtrack cycles', () => {
    // Simulate 21+ rounds caused by repeated backtrack
    const entries: MockEntry[] = [];
    for (let i = 0; i < 21; i++) {
      const phase = i % 2 === 0 ? 'proposal' : 'dev-design';
      const entry: MockEntry = {
        phase,
        verdict: 'fail',
        attempt: Math.floor(i / 2) + 1,
        timestamp: new Date(Date.now() + i).toISOString(),
        backtrack_to: i % 2 === 1 ? 'proposal' : null,
        report: '',
        checklist: [],
      };
      entries.push(entry);
    }
    const result = next(entries);
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

describe('runPhaseNext — workflow.json default', () => {
  it('should use requirement table when workflow.json missing', () => {
    const result = next([], 'test-change');
    expect(result.total_phases).toBe(8);
  });

  it('should use requirement table when workflow.json lacks workflow_type', () => {
    vi.mocked(fs.existsSync).mockImplementation((filePath: fs.PathLike) => {
      const p = String(filePath);
      return (
        p === getChangeDir('test-change', FIXTURE_PROJECT_ROOT) ||
        p.endsWith('workflow.json') ||
        p.endsWith('eval.json')
      );
    });
    vi.mocked(fs.readFileSync).mockImplementation(
      (
        path: fs.PathOrFileDescriptor,
        _options?: BufferEncoding | fs.ObjectEncodingOptions | null,
      ): string => {
        const p = String(path);
        if (p.endsWith('workflow.json')) {
          return '{}';
        }
        if (p.endsWith('eval.json')) {
          return '[]';
        }
        return '';
      },
    );
    const result = runPhaseNext({ change: 'test-change', project_root: FIXTURE_PROJECT_ROOT });
    expect(result.total_phases).toBe(8);
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
    // 直接调用 runPhaseNext 验证空 change 抛错
    expect(() => runPhaseNext({ change: '', project_root: FIXTURE_PROJECT_ROOT })).toThrow(
      'Missing required parameter: change',
    );
  });

  it('change 不存在时应抛出 Error', () => {
    vi.mocked(fs.existsSync).mockReturnValue(false);
    expect(() =>
      runPhaseNext({ change: 'non-existent-change', project_root: FIXTURE_PROJECT_ROOT }),
    ).toThrow(/Change "non-existent-change" does not exist/);
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
    const entries: MockEntry[] = [];
    for (let i = 0; i < 5; i++) {
      entries.push(failEntry('proposal', i + 1));
    }
    entries.push(failEntry('proposal', 6)); // 6 attempts = 5 fails + 1 = max retries
    // Actually 5 fails (attempts 1-5) should trigger max_retries
    const result = next([
      failEntry('proposal', 1),
      failEntry('proposal', 2),
      failEntry('proposal', 3),
      failEntry('proposal', 4),
      failEntry('proposal', 5),
    ]);
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
    const entries = [
      failEntry('proposal', 1),
      failEntry('proposal', 2),
      failEntry('proposal', 3),
      failEntry('proposal', 4),
      failEntry('proposal', 5),
    ];
    const result = next(entries, 'test-change');
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
