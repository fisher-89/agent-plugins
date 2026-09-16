/**
 * 集成测试: backtrack-reason-compat
 *
 * 验证旧格式条目（无 `backtrack_reason` 字段）的向后兼容性。条目来源为
 * `workflow.json.eval`（权威数组），遗留 `eval.json` 仅作为回退路径的补充用例。
 *
 * @see openspec/changes/backtrack-reason-propagation/test-design.md — AC-6
 */

import * as fs from 'fs';

import { describe, it, expect, vi, beforeEach } from 'vite-plus/test';

vi.mock('fs', async (importOriginal) => {
  const actual = await importOriginal<typeof fs>();
  return { ...actual, existsSync: vi.fn(), readFileSync: vi.fn() };
});

// 只有写入侧与 stale 传播被 stub；读取走真实 readEvalJson，以覆盖双源优先级。
vi.mock('../../src/lib/eval-json', async () => {
  const actual = await vi.importActual('../../src/lib/eval-json');
  return {
    ...actual,
    markPhaseStale: vi.fn(),
    appendEntry: vi.fn(),
    writeEvalJson: vi.fn(),
  };
});

vi.mock('../../src/lib/change', () => ({
  getChangeDir: vi.fn(() => '/tmp/test-change'),
}));

import { runPhaseNext } from '../../src/commands/phase-next';

const FIXTURE_PROJECT_ROOT = '/tmp/fixture-project';
const DEFAULT_RUN_ID = 'test-run';

// Pre-built old-format entries WITHOUT backtrack_reason field
const oldFormatEntries: Array<Record<string, unknown>> = [
  {
    phase: 'proposal',
    verdict: 'pass',
    attempt: 1,
    timestamp: new Date('2026-01-01T00:00:00.000Z').toISOString(),
    backtrack_to: null,
    report: 'Proposal accepted',
    checklist: [{ item: 'scope', pass: true, evidence: 'defined' }],
    skipped: false,
  },
  {
    phase: 'code-analyze',
    verdict: 'pass',
    attempt: 1,
    timestamp: new Date('2026-01-02T00:00:00.000Z').toISOString(),
    backtrack_to: null,
    report: 'Analysis complete',
    checklist: [{ item: 'analysis', pass: true, evidence: 'done' }],
    skipped: false,
  },
  {
    phase: 'test-design',
    verdict: 'fail',
    attempt: 1,
    timestamp: new Date('2026-01-03T00:00:00.000Z').toISOString(),
    backtrack_to: 'proposal',
    // Intentionally NO backtrack_reason — old format
    report: 'Test design incomplete',
    checklist: [{ item: 'coverage', pass: false, evidence: 'missing' }],
    skipped: false,
  },
];

// Mixed format: some old entries without backtrack_reason, some new with it
const mixedFormatEntries: Array<Record<string, unknown>> = [
  ...oldFormatEntries,
  {
    phase: 'test-gen',
    verdict: 'pass',
    attempt: 1,
    timestamp: new Date('2026-01-04T00:00:00.000Z').toISOString(),
    backtrack_to: null,
    report: 'Tests generated',
    checklist: [{ item: 'generation', pass: true, evidence: 'done' }],
    skipped: false,
  },
  {
    phase: 'test-execution',
    verdict: 'fail',
    attempt: 1,
    timestamp: new Date('2026-01-05T00:00:00.000Z').toISOString(),
    backtrack_to: 'code-analyze',
    backtrack_reason: '测试执行发现新的分析需求',
    report: 'Execution found issues',
    checklist: [{ item: 'execution', pass: false, evidence: 'failure' }],
    skipped: false,
  },
];

interface StoreLayout {
  /** `workflow.json.eval` 的内容（`[]` 也是权威值）；`legacyOnly` 时不写该键。 */
  entries?: Array<Record<string, unknown>>;
  /** 不写 `eval` 键，让读取走遗留 `eval.json` 回退。 */
  legacyOnly?: boolean;
  /** 遗留 `eval.json` 的内容；设置后该文件存在。 */
  legacy?: Array<Record<string, unknown>>;
  workflowType?: string;
  missingWorkflow?: boolean;
}

/** 布置 `workflow.json` / 遗留 `eval.json`，读取走真实 readEvalJson。 */
function arrangeStore(layout: StoreLayout): void {
  const workflowType = layout.workflowType ?? 'test-only';
  const changeDir = '/tmp/test-change';

  vi.mocked(fs.existsSync).mockImplementation((filePath: fs.PathLike) => {
    const p = String(filePath);
    if (p === changeDir) return true;
    if (p.endsWith('workflow.json')) return layout.missingWorkflow !== true;
    if (p.endsWith('eval.json')) return layout.legacy !== undefined;
    return false;
  });
  vi.mocked(fs.readFileSync).mockImplementation(
    (
      path: fs.PathOrFileDescriptor,
      _options?: BufferEncoding | fs.ObjectEncodingOptions | null,
    ): string => {
      const p = String(path);
      if (p.endsWith('workflow.json')) {
        const doc: Record<string, unknown> = { workflow_type: workflowType, created: '2026-09-11' };
        if (layout.legacyOnly !== true) {
          doc.eval = layout.entries ?? [];
        }
        return JSON.stringify(doc);
      }
      if (p.endsWith('eval.json')) {
        return JSON.stringify(layout.legacy ?? []);
      }
      return '';
    },
  );
}

function nextPhaseNext() {
  return runPhaseNext({
    change: 'test-change',
    project_root: FIXTURE_PROJECT_ROOT,
    run_id: DEFAULT_RUN_ID,
  });
}

beforeEach(() => {
  vi.clearAllMocks();
  arrangeStore({ entries: [] });
});

describe('backtrack-reason-compat — 向后兼容 (AC-6)', () => {
  it('workflow.json.eval 中不含 backtrack_reason 字段的旧格式条目，phase_next 正常返回不抛错', () => {
    arrangeStore({ entries: [...oldFormatEntries] });

    expect(() => nextPhaseNext()).not.toThrow();

    const result = nextPhaseNext();
    expect(result.error).toBeNull();
    // Should backtrack to proposal (from old-format test-design)
    expect(result.next_phase).toBe('proposal');
  });

  it('混合新旧格式条目，回溯时 prompt 拼接最新条目的原因', () => {
    arrangeStore({ entries: [...mixedFormatEntries] });

    const result = nextPhaseNext();

    // Latest entry is test-execution with backtrack_to: code-analyze
    expect(result.error).toBeNull();
    expect(result.next_phase).toBe('code-analyze');
    expect(result.executor!.prompt).toContain('⚠️ 回溯原因: 测试执行发现新的分析需求');
  });

  it('旧条目中无 backtrack_reason，backtrack_to 有效时仍正常回溯', () => {
    arrangeStore({ entries: [...oldFormatEntries] });

    const result = nextPhaseNext();

    // Should still backtrack to proposal (backtrack_to is present)
    expect(result.next_phase).toBe('proposal');
    expect(result.error).toBeNull();
    // Reason should be null (no backtrack_reason in old entries)
    expect(result.executor!.prompt).not.toContain('⚠️ 回溯原因');
    // Planner and evaluator should be present (normal phase response)
    expect(result.executor).not.toBeNull();
    expect(result.evaluator).not.toBeNull();
  });

  it('遗留 eval.json 回退路径下的旧条目同样可解析（AC-3）', () => {
    arrangeStore({ legacyOnly: true, legacy: [...oldFormatEntries] });

    const result = nextPhaseNext();

    expect(result.error).toBeNull();
    expect(result.next_phase).toBe('proposal');
    expect(result.executor!.prompt).not.toContain('⚠️ 回溯原因');
  });

  it('权威 eval 为 [] 时挡住遗留 eval.json，旧条目不再被读取（AC-5）', () => {
    arrangeStore({ entries: [], legacy: [...oldFormatEntries] });

    const result = nextPhaseNext();

    // 空权威数组 → first run，不合并遗留条目
    expect(result.next_phase).toBe('proposal');
    expect(result.last_result).toBeNull();
  });

  it('夹具不再依赖向磁盘 eval.json 写盘来喂条目：workflow.json 缺失即抛错（AC-13）', () => {
    arrangeStore({ missingWorkflow: true, legacyOnly: true, legacy: [...oldFormatEntries] });

    expect(() => nextPhaseNext()).toThrow(/workflow\.json 不存在/);
  });
});
