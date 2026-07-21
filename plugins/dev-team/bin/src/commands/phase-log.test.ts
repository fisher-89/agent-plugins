/**
 * Tests for phase-log.ts — simplified input validation and entry writing.
 *
 * backtrack_to/backtrack_reason are no longer accepted by phase_log.
 * Backtrack state is managed exclusively by the standalone backtrack tool.
 */

import type * as fs from 'fs';

import { describe, it, expect, vi, beforeEach } from 'vite-plus/test';

vi.mock('fs', async (importOriginal) => {
  const actual = await importOriginal<typeof fs>();
  return { ...actual, existsSync: vi.fn(), readFileSync: vi.fn() };
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
  getChangeDir: vi.fn(() => '/tmp/test-change'),
}));

import { appendEntry, writeEvalJson } from '../lib/eval-json';
import { runPhaseLog } from './phase-log';
const VALID_ITEMS = [{ item: 'test', pass: true, evidence: 'ok' }];
const FAILED_ITEMS = [{ item: 'test', pass: false, evidence: 'ok' }];

beforeEach(() => {
  vi.clearAllMocks();
});

describe('runPhaseLog -- 无回溯参数的正常路径', () => {
  it('pass entry appends normally', () => {
    runPhaseLog({
      change: 'test-change',
      phase: 'proposal',
      report: 'ok',
      checklist: VALID_ITEMS,
    });

    expect(appendEntry).toHaveBeenCalled();
  });

  it('fail entry appends normally', () => {
    runPhaseLog({
      change: 'test-change',
      phase: 'proposal',
      report: 'has issues',
      checklist: FAILED_ITEMS,
    });

    expect(appendEntry).toHaveBeenCalled();
  });

  it('skipped entry includes skipped flag', () => {
    runPhaseLog({
      change: 'test-change',
      phase: 'test-execution',
      report: 'No tests found',
      checklist: [],
      skipped: true,
    });

    expect(appendEntry).toHaveBeenCalled();
    const entry = vi.mocked(appendEntry).mock.calls[0][1];
    expect(entry.skipped).toBe(true);
    expect(entry.verdict).toBe('pass');
  });

  it('auto-calculates pass when all checklist items pass', () => {
    runPhaseLog({
      change: 'test-change',
      phase: 'proposal',
      report: 'all good',
      checklist: [
        { item: 'check 1', pass: true, evidence: 'ok' },
        { item: 'check 2', pass: true, evidence: 'ok' },
      ],
    });

    expect(appendEntry).toHaveBeenCalled();
    const entry = vi.mocked(appendEntry).mock.calls[0][1];
    expect(entry.verdict).toBe('pass');
  });

  it('auto-calculates fail when any item fails', () => {
    runPhaseLog({
      change: 'test-change',
      phase: 'proposal',
      report: 'has issues',
      checklist: [
        { item: 'check 1', pass: true, evidence: 'ok' },
        { item: 'check 2', pass: false, evidence: 'broken' },
      ],
    });

    expect(appendEntry).toHaveBeenCalled();
    const entry = vi.mocked(appendEntry).mock.calls[0][1];
    expect(entry.verdict).toBe('fail');
  });

  it('report exceeding 500 chars throws', () => {
    expect(() =>
      runPhaseLog({
        change: 'test-change',
        phase: 'proposal',
        report: 'x'.repeat(501),
        checklist: VALID_ITEMS,
      }),
    ).toThrow();
  });

  it('always uses appendEntry (no conditional write logic)', () => {
    runPhaseLog({
      change: 'test-change',
      phase: 'proposal',
      report: 'ok',
      checklist: VALID_ITEMS,
    });

    expect(appendEntry).toHaveBeenCalled();
    expect(writeEvalJson).not.toHaveBeenCalled();
  });

  it('传入 backtrack_to 时 schema 层静默忽略（Zod stripUnknown 行为）', () => {
    // phaseLogInputSchema 不包含 backtrack_to/backtrack_reason 字段，
    // Zod 默认 stripUnknown=true，额外字段被静默忽略
    runPhaseLog({
      change: 'test-change',
      phase: 'proposal',
      report: 'ok',
      checklist: VALID_ITEMS,
      // @ts-expect-error — backtrack_to 不在 PhaseLogOptions 类型中，但运行时传入
      backtrack_to: 'proposal',
    });

    expect(appendEntry).toHaveBeenCalledTimes(1);
    // 写入的 entry 不应包含 backtrack_to 字段
    const entry = vi.mocked(appendEntry).mock.calls[0][1];
    expect(entry.backtrack_to).toBeUndefined();
  });

  it('identical pass calls invoke appendEntry twice (idempotency)', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-06-25T01:29:05.000Z'));
    try {
      const opts = {
        change: 'test-change',
        phase: 'proposal' as const,
        report: 'ok',
        checklist: VALID_ITEMS,
      };

      runPhaseLog(opts);
      runPhaseLog(opts);

      expect(appendEntry).toHaveBeenCalledTimes(2);
      expect(vi.mocked(appendEntry).mock.calls[0]).toEqual(vi.mocked(appendEntry).mock.calls[1]);
    } finally {
      vi.useRealTimers();
    }
  });
});
