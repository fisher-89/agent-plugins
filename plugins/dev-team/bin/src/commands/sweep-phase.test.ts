/**
 * 单元测试: commands/sweep-phase.ts — UserPromptSubmit hook 子命令
 *
 * 覆盖范围（openspec/changes/phase-lifecycle-file-log/test-design.md）:
 * - AC-10: 绑定 change 有遗留 active_phase → 移入 interrupted[]（end_at=now）并清空，
 *   eval 数组不变；无绑定 no-op；任何失败仅 stderr、exit 0
 * - 绑定 change 无 active_phase（null/缺失）→ no-op，interrupted 不增长
 * - 连续两条消息（两次调用）→ 第二次无遗留可归档，interrupted 仅一条
 * - session 未绑定 → 静默 no-op（stderr 无诊断），exit 0
 * - stdin 非 JSON / 空串 / session_id 缺失 → 静默 exit 0 不抛
 * - workflow.json 缺失或非法（interruptActivePhase 抛错）→ stderr 诊断 + exit 0 不阻塞
 *
 * Mock 策略（仅进程边界）:
 * - stdin: vi.mock node:fs 仅替换 readFileSync，fd 0 按队列弹出（其余转发真实实现，
 *   供 session 注册表读写）；os.tmpdir 重定向到每用例隔离目录承载注册表；
 * - getProjectDir: vi.mock 指向临时项目根（不用 process.chdir）；
 * - process.exit / stderr: vi.spyOn 拦截（exit 断言未被调用，stderr 捕获诊断）。
 */

import * as realFs from 'node:fs';
import type * as NodeOs from 'node:os';
import * as nodePath from 'node:path';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type * as ProjectRoot from '../lib/project-root';

// ---------------------------------------------------------------------------
// 可控制 mock（vi.hoisted 保证工厂执行前已初始化）
// ---------------------------------------------------------------------------

const { mockReadFileSyncRef, actualFsRef, osMockState, projectRootRef } = vi.hoisted(() => ({
  mockReadFileSyncRef: {
    current: null as null | ((...args: unknown[]) => string),
  },
  /** 真实 readFileSync（测试断言读盘走它，绕开 stdin 分流 mock 面）。 */
  actualFsRef: {
    current: null as null | ((...args: unknown[]) => string),
  },
  osMockState: {
    /** 每用例隔离目录；null 时回退真实 tmpdir。 */
    tmpdir: null as string | null,
    /** 真实 tmpdir（工厂首次执行时捕获）。 */
    realTmp: null as string | null,
  },
  projectRootRef: { current: null as null | string },
}));

vi.mock('node:fs', async (importOriginal) => {
  const actual = await importOriginal<typeof realFs>();
  mockReadFileSyncRef.current = actual.readFileSync as (...args: unknown[]) => string;
  actualFsRef.current = actual.readFileSync as (...args: unknown[]) => string;
  return {
    ...actual,
    readFileSync: ((...args: unknown[]) => {
      // stdin 分流：fd 0 的读取交给测试注入，其余路径转发真实实现（注册表/fixture 装载）
      if (args[0] === 0) {
        return mockReadFileSyncRef.current!(...args);
      }
      return actualFsRef.current!(...args);
    }) as typeof actual.readFileSync,
  };
});

vi.mock('node:os', async (importOriginal) => {
  const actual = await importOriginal<typeof NodeOs>();
  osMockState.realTmp = actual.tmpdir();
  return {
    ...actual,
    tmpdir: () => osMockState.tmpdir ?? osMockState.realTmp!,
  };
});

vi.mock('../lib/project-root', async (importOriginal) => {
  const actual = await importOriginal<typeof ProjectRoot>();
  return {
    ...actual,
    getProjectDir: () => projectRootRef.current!,
  };
});

// mock 就位后导入被测模块与真实注册表
import { bindSession, lookupChange } from '../lib/session-registry';
import { runSweepPhase } from './sweep-phase';

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

function createTempRoot(prefix = 'sweep-phase-test-'): string {
  return realFs.mkdtempSync(nodePath.join(osMockState.realTmp!, prefix));
}

function createChangeFixture(root: string, name: string, doc: unknown): string {
  const changeDir = nodePath.join(root, 'openspec', 'changes', name);
  realFs.mkdirSync(changeDir, { recursive: true });
  realFs.writeFileSync(nodePath.join(changeDir, 'workflow.json'), JSON.stringify(doc), 'utf-8');
  return changeDir;
}

function validDoc(extra: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    workflow_type: 'requirement',
    created: '2026-09-18',
    eval: [],
    file_log: [],
    ...extra,
  };
}

function evalEntry(phase: string): Record<string, unknown> {
  return {
    phase,
    verdict: 'pass',
    attempt: 1,
    timestamp: '2026-09-18T00:00:00.000Z',
    report: 'r',
    checklist: [{ item: 'i', pass: true, evidence: 'e' }],
    backtrack_to: null,
  };
}

/** 经真实 readFileSync 读回磁盘 workflow.json（绕开 stdin 分流 mock 面）。 */
function readRaw(changeDir: string): string {
  const read = actualFsRef.current;
  if (!read) throw new Error('真实 readFileSync 未就绪（node:fs mock 工厂未执行）');
  return read(nodePath.join(changeDir, 'workflow.json'), 'utf-8');
}

function readDoc(changeDir: string): Record<string, unknown> {
  return JSON.parse(readRaw(changeDir)) as Record<string, unknown>;
}

const ACTIVE = { phase: 'implement', attempt: 2, start_at: '2026-09-18T08:00:00.000Z' };

let root = '';
let exitMock: ReturnType<typeof vi.spyOn>;
let stderrLines: string[];

beforeEach(() => {
  root = createTempRoot();
  projectRootRef.current = root;
  osMockState.tmpdir = createTempRoot('sweep-registry-');

  exitMock = vi.spyOn(process, 'exit').mockImplementation(() => undefined as never);
  const stderrSpy = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);
  stderrLines = [];
  stderrSpy.mockImplementation(((chunk: unknown) => {
    stderrLines.push(String(chunk));
    return true;
  }) as typeof process.stderr.write);

  bindSession(root, 'S1', 'my-change');
});

afterEach(() => {
  vi.restoreAllMocks();
  realFs.rmSync(root, { recursive: true, force: true });
  if (osMockState.tmpdir) realFs.rmSync(osMockState.tmpdir, { recursive: true, force: true });
  osMockState.tmpdir = null;
});

/** 注入一条 stdin 事件并执行 sweep。 */
function sweep(event: Record<string, unknown>): void {
  const raw = JSON.stringify(event);
  mockReadFileSyncRef.current = (() => raw) as (...args: unknown[]) => string;
  runSweepPhase();
}

// ===========================================================================
// runSweepPhase — 正向 (AC-10)
// ===========================================================================

describe('runSweepPhase — 正向 (AC-10)', () => {
  it('绑定 change 有遗留 active_phase → 移入 interrupted[]（end_at 为当次时刻）并清空；eval 数组不变', () => {
    const changeDir = createChangeFixture(root, 'my-change', validDoc({ active_phase: ACTIVE }));

    sweep({ session_id: 'S1' });

    const doc = readDoc(changeDir);
    expect(doc.active_phase).toBeNull();
    const interrupted = doc.interrupted as Array<Record<string, unknown>>;
    expect(interrupted).toHaveLength(1);
    expect(interrupted[0]).toMatchObject({ ...ACTIVE });
    expect(String(interrupted[0].end_at)).toMatch(/^\d{4}-\d{2}-\d{2}T/);
    expect(doc.eval).toEqual([]);
    // 归档动作有 stderr 诊断
    expect(stderrLines.join('')).toContain('sweep-phase');
    expect(exitMock).not.toHaveBeenCalled();
  });

  it('绑定 change 无 active_phase（null / 字段缺失）→ no-op，interrupted 不增长', () => {
    for (const activePhase of [null, undefined]) {
      const name = `idle-${String(activePhase)}`;
      bindSession(root, `S-${name}`, name);
      const changeDir = createChangeFixture(
        root,
        name,
        validDoc(activePhase === null ? { active_phase: null } : {}),
      );

      sweep({ session_id: `S-${name}` });

      const doc = readDoc(changeDir);
      expect(doc.interrupted).toBeUndefined();
      expect(exitMock).not.toHaveBeenCalled();
    }
  });

  it('连续两条消息（两次调用）→ 第二次无遗留可归档，interrupted 仅一条', () => {
    const changeDir = createChangeFixture(root, 'my-change', validDoc({ active_phase: ACTIVE }));

    sweep({ session_id: 'S1' });
    sweep({ session_id: 'S1' });

    const doc = readDoc(changeDir);
    expect(doc.interrupted).toHaveLength(1);
    expect(doc.active_phase).toBeNull();
    expect(exitMock).not.toHaveBeenCalled();
  });
});

// ===========================================================================
// runSweepPhase — 边界
// ===========================================================================

describe('runSweepPhase — 边界', () => {
  it('session 未绑定 → 静默 no-op（stderr 无诊断），exit 0', () => {
    const changeDir = createChangeFixture(root, 'my-change', validDoc({ active_phase: ACTIVE }));
    const before = readRaw(changeDir);

    sweep({ session_id: 'S-unbound' });

    expect(stderrLines.join('')).toBe('');
    expect(readRaw(changeDir)).toBe(before);
    expect(exitMock).not.toHaveBeenCalled();
  });

  it('绑定指向不存在的 change 目录 → 归档抛错被吞，stderr 诊断 + exit 0', () => {
    bindSession(root, 'S-ghost', 'ghost-change');

    sweep({ session_id: 'S-ghost' });

    expect(stderrLines.join('')).toContain('sweep-phase');
    expect(stderrLines.join('')).toContain('workflow.json');
    expect(exitMock).not.toHaveBeenCalled();
  });
});

// ===========================================================================
// runSweepPhase — 异常（fail-open）
// ===========================================================================

describe('runSweepPhase — 异常（fail-open）', () => {
  it('stdin 非 JSON / 空串 / 空白 / session_id 缺失或非字符串 → 静默 exit 0 不抛', () => {
    createChangeFixture(root, 'my-change', validDoc({ active_phase: ACTIVE }));
    for (const raw of [
      '{not json',
      '',
      '   ',
      '[1,2]',
      '"str"',
      JSON.stringify({}),
      JSON.stringify({ session_id: 42 }),
    ]) {
      stderrLines.length = 0;
      mockReadFileSyncRef.current = (() => raw) as (...args: unknown[]) => string;
      expect(() => runSweepPhase()).not.toThrow();
      expect(stderrLines.join('')).toBe('');
      expect(exitMock).not.toHaveBeenCalled();
    }
    // workflow.json 未被触碰
    const changeDir = nodePath.join(root, 'openspec', 'changes', 'my-change');
    expect(readDoc(changeDir).active_phase).toEqual(ACTIVE);
  });

  it('workflow.json 非法（interruptActivePhase 抛错）→ stderr 诊断 + exit 0，不阻塞（fail-open）', () => {
    const changeDir = createChangeFixture(root, 'my-change', validDoc({ active_phase: ACTIVE }));
    realFs.writeFileSync(nodePath.join(changeDir, 'workflow.json'), '{broken', 'utf-8');

    sweep({ session_id: 'S1' });

    expect(stderrLines.join('')).toContain('sweep-phase');
    expect(stderrLines.join('')).toContain('解析失败');
    expect(exitMock).not.toHaveBeenCalled();
    expect(readRaw(changeDir)).toBe('{broken');
  });

  it('workflow.json 缺失 → stderr 诊断 + exit 0', () => {
    bindSession(root, 'S-missing', 'missing-change');

    sweep({ session_id: 'S-missing' });

    expect(stderrLines.join('')).toContain('sweep-phase');
    expect(stderrLines.join('')).toContain('workflow.json 不存在');
    expect(exitMock).not.toHaveBeenCalled();
  });
});

// ===========================================================================
// runSweepPhase — eval 不被 sweep 改写（AC-10 不写 eval）
// ===========================================================================

describe('runSweepPhase — eval 不变', () => {
  it('归档遗留 active_phase 时 eval 数组逐字保留（不烧 retry 配额）', () => {
    const entries = [evalEntry('implement'), evalEntry('proposal')];
    const changeDir = createChangeFixture(
      root,
      'my-change',
      validDoc({ active_phase: ACTIVE, eval: entries }),
    );

    sweep({ session_id: 'S1' });

    expect(readDoc(changeDir).eval).toEqual(entries);
  });
});

// lookupChange 直读注册表（os.tmpdir 重定向生效的旁证）
describe('session 注册表（tmpdir 重定向）', () => {
  it('bindSession 后 lookupChange 返回绑定 change；未绑定返回 null', () => {
    expect(lookupChange(root, 'S1')).toBe('my-change');
    expect(lookupChange(root, 'S-none')).toBeNull();
  });
});
