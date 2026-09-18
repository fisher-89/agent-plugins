/**
 * 集成测试: phase_start → active_phase 运行态 → phase_log 盖章清场 / sweep 中断归档
 *
 * 真实链路（模块全为真实实现，仅 mock 进程边界）:
 * - `bin/src/commands/change-create.ts` — fixture 创建方（workflow.json）
 * - `bin/src/commands/phase-start.ts` — 触发方（校验 + attempt 推导 + 写运行态）
 * - `bin/src/modules/workflow/phase/phase-state.ts` — 状态层（active_phase / interrupted）
 * - `bin/src/lib/eval-json.ts` — 条目组装（buildEntry 携带 start_at、appendEntry 落盘）
 * - `bin/src/commands/phase-log.ts` — 盖章清场方（落盘前盖章、成功后清场）
 * - `bin/src/commands/sweep-phase.ts` + `bin/src/lib/session-registry.ts` — 中断收口方
 *
 * Mock 策略（仅进程边界）:
 * - stdin（readFileSync fd 0）: vi.mock 仅替换 readFileSync（sweep 场景注入 session_id），
 *   其余路径转发真实实现（真盘读写不受影响）；
 * - os.tmpdir: 重定向隔离注册表目录（sweep 绑定用例）；
 * - getProjectDir: vi.mock 指向临时项目根；
 * - phase-state 层（仅清场故障注入）: vi.spyOn clearActivePhase 一次性抛错；
 * - 系统时钟: vi.setSystemTime 固定 start_at / end_at 断言。
 *
 * 关联 AC: AC-2, AC-3, AC-10（并覆盖 AC-1 的 start_at 落盘面）。
 */

import * as fs from 'fs';
import type * as NodeFs from 'node:fs';
import type * as NodeOs from 'node:os';
import * as os from 'os';
import * as path from 'path';

import { afterAll, afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { runChangeCreate } from '../../src/commands/change-create';
import { runPhaseLog } from '../../src/commands/phase-log';
import { runPhaseStart } from '../../src/commands/phase-start';
import { runSweepPhase } from '../../src/commands/sweep-phase';
import type * as ProjectRoot from '../../src/lib/project-root';
import { bindSession } from '../../src/lib/session-registry';
import * as workflowModule from '../../src/modules/workflow';

// ---------------------------------------------------------------------------
// 可控 mock（仅进程边界）
// ---------------------------------------------------------------------------

/** phase 入参类型（phaseIdSchema 的 9 值枚举）。 */
type PhaseId = Parameters<typeof runPhaseStart>[0]['phase'];

const h = vi.hoisted(() => ({
  queue: [] as string[],
  tmpRoot: '',
  actualReadFileSync: null as unknown as typeof NodeFs.readFileSync,
  mockReadFileSync: vi.fn(),
  mockGetProjectDir: vi.fn<() => string>(),
}));

vi.mock('node:fs', async (importOriginal) => {
  const actual = await importOriginal<typeof NodeFs>();
  h.actualReadFileSync = actual.readFileSync;
  return { ...actual, readFileSync: h.mockReadFileSync };
});

vi.mock('fs', async (importOriginal) => {
  const actual = await importOriginal<typeof NodeFs>();
  h.actualReadFileSync = actual.readFileSync;
  return { ...actual, readFileSync: h.mockReadFileSync };
});

vi.mock('node:os', async (importOriginal) => {
  const actual = await importOriginal<typeof NodeOs>();
  return { ...actual, tmpdir: () => (h.tmpRoot ? h.tmpRoot : actual.tmpdir()) };
});

vi.mock('../../src/lib/project-root', async (importOriginal) => {
  const actual = await importOriginal<typeof ProjectRoot>();
  return { ...actual, getProjectDir: h.mockGetProjectDir };
});

// stdin 注入实现：fd 0 → 弹出注入队列（sweep 场景）；其余委托真实 readFileSync。
function installStdinFork(): void {
  h.mockReadFileSync.mockReset();
  h.mockReadFileSync.mockImplementation(((...args: unknown[]) => {
    if (args[0] === 0) {
      const next = h.queue.shift();
      if (next === undefined) {
        throw new Error('stdin 注入队列已空');
      }
      return next;
    }
    const realReadFileSync = h.actualReadFileSync as unknown as (...a: unknown[]) => unknown;
    return realReadFileSync(args[0], args[1]);
  }) as unknown as (...args: unknown[]) => unknown);
}

installStdinFork();

const exitMock = vi.spyOn(process, 'exit').mockImplementation(() => undefined as never);
const stderrSpy = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);

// ---------------------------------------------------------------------------
// Fixture helpers（真盘）
// ---------------------------------------------------------------------------

let projectRoot: string;
let tmpBase: string;

beforeEach(() => {
  projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'phase-lifecycle-proj-'));
  tmpBase = fs.mkdtempSync(path.join(os.tmpdir(), 'phase-lifecycle-tmp-'));
  h.tmpRoot = tmpBase;
  h.queue.length = 0;
  installStdinFork();
  h.mockGetProjectDir.mockReset();
  h.mockGetProjectDir.mockReturnValue(projectRoot);
  stderrSpy.mockReset();
  stderrSpy.mockImplementation(() => true);
  exitMock.mockClear();
  vi.setSystemTime(new Date('2026-09-18T08:00:00.000Z'));
});

afterEach(() => {
  vi.useRealTimers();
  fs.rmSync(projectRoot, { recursive: true, force: true });
  fs.rmSync(tmpBase, { recursive: true, force: true });
  h.tmpRoot = '';
});

afterAll(() => {
  vi.useRealTimers();
});

function workflowPath(change: string): string {
  return path.join(projectRoot, 'openspec', 'changes', change, 'workflow.json');
}

function readDoc(change: string): Record<string, unknown> {
  return JSON.parse(fs.readFileSync(workflowPath(change), 'utf-8')) as Record<string, unknown>;
}

interface EvalEntryShape {
  phase: string;
  attempt?: number;
  verdict: string;
  report: string;
  checklist: Array<Record<string, unknown>>;
  timestamp: string;
  start_at?: string;
  skipped?: boolean;
}

function readEval(change: string): EvalEntryShape[] {
  return (readDoc(change).eval as EvalEntryShape[]) ?? [];
}

/** change_create 建立后注入一条该 phase 的 fail eval 条目（retry 场景前置）。 */
function createChangeWithFail(change: string, phase: string): string {
  const created = runChangeCreate(change, projectRoot, 'requirement');
  const doc = JSON.parse(fs.readFileSync(workflowPath(change), 'utf-8')) as Record<string, unknown>;
  doc.eval = [
    {
      phase,
      verdict: 'fail',
      attempt: 1,
      timestamp: '2026-09-18T07:00:00.000Z',
      report: 'first fail',
      checklist: [{ item: 'i', pass: false, evidence: 'e' }],
      backtrack_to: null,
    },
  ];
  fs.writeFileSync(workflowPath(change), `${JSON.stringify(doc, null, 2)}\n`, 'utf-8');
  return created.path;
}

function startPhase(
  change: string,
  phase: PhaseId,
): { started: boolean; attempt: number; start_at: string } {
  return runPhaseStart({ change, phase, project_root: projectRoot });
}

function logPhase(
  change: string,
  phase: PhaseId,
  pass = true,
): { written: boolean; phase: string; attempt: number } {
  return runPhaseLog({
    change,
    phase,
    report: 'ok',
    checklist: [{ item: 'i', pass, evidence: 'e' }],
    project_root: projectRoot,
  });
}

/** sweep 场景：注入 stdin 事件并执行。 */
function sweep(sessionId: string): void {
  h.queue.push(JSON.stringify({ session_id: sessionId }));
  runSweepPhase();
  expect(h.queue).toHaveLength(0);
}

function stderrText(): string {
  return stderrSpy.mock.calls.map((c) => String(c[0])).join('');
}

// ============================================================================
// 场景: 完整 turn 生命周期（开启 → 盖章 → 清场）
// ============================================================================

describe('场景: 完整 turn 生命周期（AC-2 / AC-3 跨模块面）', () => {
  it('正向: phase_start → phase_log 全链路：条目盖 start_at、active_phase 清空、attempt=2（retry 前置 fail 条目）', () => {
    createChangeWithFail('lifecycle-change', 'implement');

    // 固定时钟下开门：start_at 精确可知
    const started = startPhase('lifecycle-change', 'implement');
    expect(started).toEqual({
      started: true,
      phase: 'implement',
      attempt: 2,
      start_at: '2026-09-18T08:00:00.000Z',
    });
    expect(readDoc('lifecycle-change').active_phase as Record<string, unknown>).toEqual({
      phase: 'implement',
      attempt: 2,
      start_at: '2026-09-18T08:00:00.000Z',
    });

    // 推进时钟后 phase_log：条目 timestamp 与 start_at 均为 ISO，耗时可得
    vi.setSystemTime(new Date('2026-09-18T08:30:00.000Z'));
    const logged = logPhase('lifecycle-change', 'implement');
    expect(logged).toEqual({ written: true, phase: 'implement', attempt: 2 });

    const evalEntries = readEval('lifecycle-change');
    const last = evalEntries[evalEntries.length - 1];
    expect(last.start_at).toBe('2026-09-18T08:00:00.000Z');
    expect(last.timestamp).toBe('2026-09-18T08:30:00.000Z');
    // per-attempt 耗时的数据前提：timestamp − start_at 为正
    expect(new Date(last.timestamp).getTime() - new Date(last.start_at!).getTime()).toBeGreaterThan(
      0,
    );
    // 清场：门②关闭
    expect(readDoc('lifecycle-change').active_phase ?? null).toBeNull();
  });

  it('正向: 无 phase_start 直接 phase_log → 条目无 start_at、active_phase 字段不存在不报错', () => {
    runChangeCreate('no-start-change', projectRoot, 'requirement');

    const logged = logPhase('no-start-change', 'proposal');

    expect(logged).toEqual({ written: true, phase: 'proposal', attempt: 1 });
    const entries = readEval('no-start-change');
    expect(entries).toHaveLength(1);
    expect(Object.prototype.hasOwnProperty.call(entries[0], 'start_at')).toBe(false);
    expect(readDoc('no-start-change').active_phase).toBeUndefined();
  });

  it('异常: phase_start 传非法 phase → 报错且 workflow.json 逐字节不变，后续 phase_log 正常（无残留半态）', () => {
    runChangeCreate('invalid-phase-change', projectRoot, 'requirement');
    const before = fs.readFileSync(workflowPath('invalid-phase-change'), 'utf-8');

    expect(() => startPhase('invalid-phase-change', 'code-analyze')).toThrow(
      /不属于 workflow_type/,
    );

    expect(fs.readFileSync(workflowPath('invalid-phase-change'), 'utf-8')).toBe(before);

    // 无残留半态：随后 phase_log 正常落盘
    const logged = logPhase('invalid-phase-change', 'proposal');
    expect(logged.written).toBe(true);
    expect(readEval('invalid-phase-change')).toHaveLength(1);
  });

  it('边界: retry 序列：fail 落盘 → 重新 phase_start → attempt 递增且 start_at 刷新（独立计时）', () => {
    createChangeWithFail('retry-change', 'implement');
    startPhase('retry-change', 'implement'); // attempt=2, start_at=T0

    // 第一次尝试 fail 落盘（verdict 由 checklist 推导为 fail）
    vi.setSystemTime(new Date('2026-09-18T08:10:00.000Z'));
    const failed = logPhase('retry-change', 'implement', false);
    expect(failed.attempt).toBe(2);

    // 重新开门：attempt=3，start_at 刷新为新的固定时刻
    vi.setSystemTime(new Date('2026-09-18T09:00:00.000Z'));
    const restarted = startPhase('retry-change', 'implement');
    expect(restarted.attempt).toBe(3);
    expect(restarted.start_at).toBe('2026-09-18T09:00:00.000Z');

    // retry 落盘条目盖新 start_at（独立计时，不串上一轮）
    vi.setSystemTime(new Date('2026-09-18T09:20:00.000Z'));
    logPhase('retry-change', 'implement');
    const entries = readEval('retry-change');
    expect(entries).toHaveLength(3);
    expect(entries[1].start_at).toBe('2026-09-18T08:00:00.000Z');
    expect(entries[2].start_at).toBe('2026-09-18T09:00:00.000Z');
  });
});

// ============================================================================
// 场景: sweep 中断归档与竞争防护（AC-10）
// ============================================================================

describe('场景: sweep 中断归档与竞争防护', () => {
  /** 建立 change + 遗留 active_phase，并绑定 session。 */
  function setupInterruptedChange(change: string, sessionId: string): { startedAt: string } {
    runChangeCreate(change, projectRoot, 'requirement');
    const started = startPhase(change, 'implement');
    bindSession(projectRoot, sessionId, change);
    return { startedAt: started.start_at };
  }

  it('正向: 遗留 active_phase → interrupted[] 追加一条 {phase, attempt, start_at, end_at}、active_phase 清空、eval 不变', () => {
    const { startedAt } = setupInterruptedChange('interrupted-change', 'S1');
    const evalBefore = readEval('interrupted-change');

    vi.setSystemTime(new Date('2026-09-18T10:00:00.000Z'));
    sweep('S1');

    const doc = readDoc('interrupted-change');
    expect(doc.active_phase ?? null).toBeNull();
    const interrupted = doc.interrupted as Array<Record<string, unknown>>;
    expect(interrupted).toEqual([
      { phase: 'implement', attempt: 1, start_at: startedAt, end_at: '2026-09-18T10:00:00.000Z' },
    ]);
    // eval 数组不变（sweep 不写 eval、不烧 retry 配额）
    expect(readEval('interrupted-change')).toEqual(evalBefore);
    expect(stderrText()).toContain('sweep-phase');
    expect(exitMock).not.toHaveBeenCalled();
  });

  it('正向: sweep 后 phase_log → 条目无 start_at（中断后不误盖章）', () => {
    setupInterruptedChange('post-sweep-change', 'S1');
    sweep('S1');

    const logged = logPhase('post-sweep-change', 'implement');
    expect(logged.written).toBe(true);

    const entries = readEval('post-sweep-change');
    expect(entries).toHaveLength(1);
    expect(Object.prototype.hasOwnProperty.call(entries[0], 'start_at')).toBe(false);
  });

  it('边界: 无遗留 / 未绑定 / 重复 sweep → no-op 不增长 interrupted', () => {
    // 未绑定 session（无注册表条目）→ 静默 no-op
    runChangeCreate('unbound-change', projectRoot, 'requirement');
    startPhase('unbound-change', 'implement');
    sweep('S-none');
    expect((readDoc('unbound-change').interrupted as unknown[] | undefined) ?? []).toEqual([]);

    // 已绑定 + 有遗留 → 第一次归档
    bindSession(projectRoot, 'S1', 'bound-change');
    runChangeCreate('bound-change', projectRoot, 'requirement');
    startPhase('bound-change', 'implement');
    sweep('S1');
    expect(readDoc('bound-change').interrupted).toHaveLength(1);

    // 重复 sweep → 无遗留可归档，interrupted 不增长
    sweep('S1');
    expect(readDoc('bound-change').interrupted).toHaveLength(1);

    // 已绑定但无遗留（active_phase 缺失）→ no-op
    bindSession(projectRoot, 'S2', 'idle-change');
    runChangeCreate('idle-change', projectRoot, 'requirement');
    sweep('S2');
    expect(readDoc('idle-change').interrupted).toBeUndefined();
  });

  it('异常: workflow.json 缺失（interruptActivePhase 抛错）→ stderr 诊断 + exit 0 不阻塞（fail-open）', () => {
    bindSession(projectRoot, 'S-ghost', 'ghost-change');

    sweep('S-ghost');

    expect(stderrText()).toContain('sweep-phase');
    expect(stderrText()).toContain('workflow.json');
    expect(exitMock).not.toHaveBeenCalled();
  });
});

// ============================================================================
// 场景: 清场失败吞错与无双录（AC-3 吞错收尾）
// ============================================================================

describe('场景: 清场失败吞错与无双录', () => {
  it('异常: clearActivePhase 一次性抛错 → stderr 诊断、条目落盘、输出形状不变（吞错收尾无双录）', async () => {
    createChangeWithFail('clear-fail-change', 'implement');
    startPhase('clear-fail-change', 'implement');

    // 跨进程边界外的内部故障注入（仅此场景）：clearActivePhase 一次性抛错
    const spy = vi.spyOn(workflowModule, 'clearActivePhase').mockImplementationOnce(() => {
      throw new Error('EACCES: 模拟清场失败');
    });

    let logged: { written: boolean; phase: string; attempt: number } | undefined;
    expect(() => {
      logged = logPhase('clear-fail-change', 'implement');
    }).not.toThrow();
    spy.mockRestore();

    // 条目已落盘、输出形状不变、stderr 有诊断
    expect(logged).toEqual({ written: true, phase: 'implement', attempt: 2 });
    expect(readEval('clear-fail-change')).toHaveLength(2);
    expect(stderrText()).toContain('phase-log:');
    expect(stderrText()).toContain('清理 active_phase 失败');
    // 遗留 active_phase 未被清掉（下次 phase_start / sweep 回收）
    expect((readDoc('clear-fail-change').active_phase as Record<string, unknown>).attempt).toBe(2);
  });

  it('边界: 清场失败后重新 phase_start → last-wins 覆盖遗留态，无双录条目', async () => {
    createChangeWithFail('clear-fail-recover', 'implement');
    startPhase('clear-fail-recover', 'implement');

    const spy = vi.spyOn(workflowModule, 'clearActivePhase').mockImplementationOnce(() => {
      throw new Error('EACCES: 模拟清场失败');
    });
    logPhase('clear-fail-recover', 'implement');
    spy.mockRestore();

    // 遗留 active_phase（attempt=2）仍在；重新开门 last-wins 覆盖
    vi.setSystemTime(new Date('2026-09-18T11:00:00.000Z'));
    const restarted = startPhase('clear-fail-recover', 'implement');
    expect(restarted.attempt).toBe(3);
    expect(restarted.start_at).toBe('2026-09-18T11:00:00.000Z');
    expect((readDoc('clear-fail-recover').active_phase as Record<string, unknown>).attempt).toBe(3);

    // 后续正常落盘：eval 仅追加一条新条目（无双录）
    logPhase('clear-fail-recover', 'implement');
    const entries = readEval('clear-fail-recover');
    expect(entries).toHaveLength(3);
    expect(entries.filter((e) => e.attempt === 2)).toHaveLength(1);
    expect(readDoc('clear-fail-recover').active_phase ?? null).toBeNull();
  });
});
