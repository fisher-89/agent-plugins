/**
 * 单元测试: commands/record-files.ts — PostToolUse hook 入口（门①②③）
 *
 * 覆盖范围（openspec/changes/phase-lifecycle-file-log/test-design.md）:
 * - AC-9: `phase_next` / `phase_start` MCP 事件均建立/刷新 `session_id → change`
 *   绑定且不归账
 * - AC-4 门①: 未绑定 session 的写事件 → 丢弃 + stderr「未绑定」，workflow.json 不变
 * - AC-4 门②: 已绑定但无 active_phase → 丢弃 + stderr 诊断（区别于门①文案）
 * - AC-4 门②+门③: 绑定 + active_phase + agent_type 归一匹配 → phase scope
 *   （scope=phase id、attempt=active_phase.attempt）；主 agent / executor: null /
 *   无 agent_type / cursor-home 前缀变体 → workflow scope（或前缀归一 phase scope）
 * - 门②重开: phase_log 清场后同 session 再写 → 门②丢弃；重新 phase_start 后恢复归账
 * - fail-open: 非法 stdin / tool_input 残缺 / recordFileOps 抛错（legacy change）→
 *   stderr + exit 0，不阻塞观察的工具调用
 *
 * Mock 策略（仅进程边界）: node:fs 仅替换 readFileSync（fd 0 注入事件，其余转发真实
 * 实现）；os.tmpdir 重定向承载 session 注册表；getProjectDir 指向临时项目根；
 * process.exit / stderr spy 拦截。文件系统不 mock（workflow.json 真盘读写）。
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
  actualFsRef: {
    current: null as null | ((...args: unknown[]) => string),
  },
  osMockState: {
    tmpdir: null as string | null,
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
      // stdin 分流：fd 0 的读取交给测试注入，其余路径转发真实实现
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
import { bindSession } from '../lib/session-registry';
import { runRecordFiles } from './record-files';

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

function createTempRoot(prefix = 'record-files-test-'): string {
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

const ACTIVE_IMPLEMENT = {
  phase: 'implement',
  attempt: 2,
  start_at: '2026-09-18T08:00:00.000Z',
};

let root = '';
let changeDir = '';
let exitMock: ReturnType<typeof vi.spyOn>;
let stderrLines: string[];

beforeEach(() => {
  root = createTempRoot();
  projectRootRef.current = root;
  osMockState.tmpdir = createTempRoot('record-files-registry-');

  exitMock = vi.spyOn(process, 'exit').mockImplementation(() => undefined as never);
  const stderrSpy = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);
  stderrLines = [];
  stderrSpy.mockImplementation(((chunk: unknown) => {
    stderrLines.push(String(chunk));
    return true;
  }) as typeof process.stderr.write);

  changeDir = createChangeFixture(root, 'my-change', validDoc());
});

afterEach(() => {
  vi.restoreAllMocks();
  realFs.rmSync(root, { recursive: true, force: true });
  if (osMockState.tmpdir) realFs.rmSync(osMockState.tmpdir, { recursive: true, force: true });
  osMockState.tmpdir = null;
});

/** 经真实 readFileSync 读回磁盘 file_log。 */
function readLog(dir = changeDir): Array<Record<string, unknown>> {
  const read = actualFsRef.current!;
  const doc = JSON.parse(read(nodePath.join(dir, 'workflow.json'), 'utf-8')) as {
    file_log?: Array<Record<string, unknown>>;
  };
  return doc.file_log ?? [];
}

function readRawWorkflow(dir = changeDir): string {
  const read = actualFsRef.current!;
  return read(nodePath.join(dir, 'workflow.json'), 'utf-8');
}

/** 注入一条 stdin 事件并执行 record-files。 */
function record(event: Record<string, unknown>): void {
  const raw = JSON.stringify(event);
  mockReadFileSyncRef.current = (() => raw) as (...args: unknown[]) => string;
  runRecordFiles();
}

function writeEvent(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    tool_name: 'Write',
    tool_input: { file_path: 'src/foo.ts' },
    session_id: 'S1',
    ...overrides,
  };
}

// ===========================================================================
// 绑定刷新 — phase_next / phase_start 事件 (AC-9)
// ===========================================================================

describe('绑定刷新 — MCP 生命周期事件 (AC-9)', () => {
  it('mcp__…__phase_next 事件（tool_input.change）→ bindSession 等价建立绑定且不归账', () => {
    record({
      tool_name: 'mcp__plugin_dev-team_dev-team__phase_next',
      tool_input: { change: 'my-change' },
      session_id: 'S1',
    });

    // 绑定生效的旁证：随后同 session 的写事件不再走「未绑定」分支
    stderrLines.length = 0;
    record(writeEvent({ tool_input: { file_path: 'src/bound.ts' } }));
    expect(stderrLines.join('')).not.toContain('未绑定');
    expect(readLog()).toEqual([]);
    expect(exitMock).not.toHaveBeenCalled();
  });

  it('mcp__…__phase_start 事件 → 同样建立/刷新绑定且不归账（AC-9 phase_start 分支）', () => {
    record({
      tool_name: 'mcp__plugin_dev-team_dev-team__phase_start',
      tool_input: { change: 'my-change', phase: 'implement' },
      session_id: 'S1',
    });

    stderrLines.length = 0;
    record(writeEvent({ tool_input: { file_path: 'src/bound.ts' } }));
    expect(stderrLines.join('')).not.toContain('未绑定');
    expect(readLog()).toEqual([]);
    expect(exitMock).not.toHaveBeenCalled();
  });

  it('phase_start 事件缺 tool_input.change → 不建立绑定、不写清单、不抛错', () => {
    for (const toolInput of [{}, { change: '' }, { change: 42 }, undefined]) {
      stderrLines.length = 0;
      record({
        tool_name: 'mcp__plugin_dev-team_dev-team__phase_start',
        tool_input: toolInput,
        session_id: 'S1',
      });
      expect(readLog()).toEqual([]);
      expect(exitMock).not.toHaveBeenCalled();
    }
  });
});

// ===========================================================================
// 门① — session 绑定 (AC-4 分支①)
// ===========================================================================

describe('门① — 未绑定 session 丢弃 (AC-4 分支①)', () => {
  it('未绑定 session 的写事件 → 丢弃 + stderr「未绑定」诊断，workflow.json 不变', () => {
    const before = readRawWorkflow();

    record(writeEvent({ session_id: 'S-unbound' }));

    expect(stderrLines.join('')).toContain('未绑定');
    expect(readRawWorkflow()).toBe(before);
    expect(exitMock).not.toHaveBeenCalled();
  });
});

// ===========================================================================
// 门② — active_phase 运行态 (AC-4 分支②)
// ===========================================================================

describe('门② — 无 active_phase 丢弃 (AC-4 分支②)', () => {
  it('已绑定但 change 无 active_phase → 丢弃 + stderr 诊断（区别于门①文案），workflow.json 不变', () => {
    bindSession(root, 'S1', 'my-change');
    const before = readRawWorkflow();

    record(writeEvent());

    expect(stderrLines.join('')).toContain('无 active_phase');
    expect(stderrLines.join('')).not.toContain('未绑定');
    expect(readRawWorkflow()).toBe(before);
    expect(exitMock).not.toHaveBeenCalled();
  });

  it('门②重开：phase_log 清场后（active_phase=null）同 session 再写 → 门②丢弃；重新开门后恢复归账', () => {
    bindSession(root, 'S1', 'my-change');
    createChangeFixture(root, 'my-change', validDoc({ active_phase: ACTIVE_IMPLEMENT }));

    record(
      writeEvent({
        tool_input: { file_path: 'src/first.ts' },
        agent_type: 'dev-team:implementation-generator',
      }),
    );
    expect(readLog()).toHaveLength(1);

    // 清场（phase_log 语义）
    createChangeFixture(root, 'my-change', {
      ...validDoc({ active_phase: null }),
      file_log: readLog(),
    });

    record(
      writeEvent({
        tool_input: { file_path: 'src/after-clear.ts' },
        agent_type: 'dev-team:implementation-generator',
      }),
    );
    expect(stderrLines.join('')).toContain('无 active_phase');
    expect(readLog()).toHaveLength(1);

    // 重新开门（phase_start 语义）
    createChangeFixture(root, 'my-change', {
      ...validDoc({ active_phase: ACTIVE_IMPLEMENT }),
      file_log: readLog(),
    });

    record(
      writeEvent({
        tool_input: { file_path: 'src/reopen.ts' },
        agent_type: 'dev-team:implementation-generator',
      }),
    );
    const log = readLog();
    expect(log).toHaveLength(2);
    expect(log[1]).toMatchObject({ scope: 'implement', attempt: 2, path: 'src/reopen.ts' });
  });
});

// ===========================================================================
// 门②+门③ — agent 归一匹配与 scope 解析 (AC-4 分支③④)
// ===========================================================================

describe('门②+门③ — scope 解析 (AC-4 分支③④)', () => {
  beforeEach(() => {
    bindSession(root, 'S1', 'my-change');
    createChangeFixture(root, 'my-change', validDoc({ active_phase: ACTIVE_IMPLEMENT }));
  });

  it('事件 agent_type=dev-team:implementation-generator 与运行 phase executor token 归一相等 → phase scope 记录（attempt=active_phase.attempt）', () => {
    record(writeEvent({ agent_type: 'dev-team:implementation-generator' }));

    const log = readLog();
    expect(log).toHaveLength(1);
    expect(log[0]).toMatchObject({
      op: 'write',
      scope: 'implement',
      attempt: 2,
      path: 'src/foo.ts',
    });
    expect(String(log[0].at)).toMatch(/^\d{4}-\d{2}-\d{2}T/);
  });

  it('无 agent_type（主 agent）→ scope={kind:"workflow"} 记录且条目无 attempt 字段（AC-4 分支④）', () => {
    record(writeEvent());

    const log = readLog();
    expect(log).toHaveLength(1);
    expect(log[0]).toMatchObject({ op: 'write', scope: 'workflow', path: 'src/foo.ts' });
    expect(Object.prototype.hasOwnProperty.call(log[0], 'attempt')).toBe(false);
  });

  it('cursor-home 产物前缀 dev-team_implementation-generator 归一后与 token 相等 → phase scope', () => {
    record(writeEvent({ agent_type: 'dev-team_implementation-generator' }));

    expect(readLog()[0]).toMatchObject({ scope: 'implement', attempt: 2 });
  });

  it('运行 phase 为 executor: null（code-review）→ 门③不匹配 → workflow scope（不丢事件）', () => {
    createChangeFixture(
      root,
      'my-change',
      validDoc({ active_phase: { ...ACTIVE_IMPLEMENT, phase: 'code-review' } }),
    );

    record(writeEvent({ agent_type: 'dev-team:implementation-generator' }));

    expect(readLog()[0]).toMatchObject({ scope: 'workflow', path: 'src/foo.ts' });
  });

  it('事件无 agent_type 字段（undefined，序列化后字段缺失）→ 不匹配 → workflow scope（不报错）', () => {
    record({ tool_name: 'Write', tool_input: { file_path: 'src/foo.ts' }, session_id: 'S1' });

    expect(readLog()[0]).toMatchObject({ scope: 'workflow' });
    expect(exitMock).not.toHaveBeenCalled();
  });

  it('agent_type 归一后不相等（其他 subagent）→ workflow scope', () => {
    record(writeEvent({ agent_type: 'dev-team:test-gen-generator' }));

    expect(readLog()[0]).toMatchObject({ scope: 'workflow' });
  });
});

// ===========================================================================
// fail-open — 吞错与 exit 0
// ===========================================================================

describe('fail-open — 吞错与 exit 0', () => {
  it('非法 stdin / tool_input 结构残缺 → 静默 exit 0，无写副作用', () => {
    bindSession(root, 'S1', 'my-change');
    const before = readRawWorkflow();
    for (const raw of [
      '{not json',
      '',
      '   ',
      '[1,2]',
      '"str"',
      JSON.stringify({ tool_name: 42 }),
      JSON.stringify({ tool_name: 'Write', tool_input: 'str' }),
    ]) {
      stderrLines.length = 0;
      mockReadFileSyncRef.current = (() => raw) as (...args: unknown[]) => string;
      expect(() => runRecordFiles()).not.toThrow();
      expect(exitMock).not.toHaveBeenCalled();
    }
    expect(readRawWorkflow()).toBe(before);
  });

  it('recordFileOps 抛错（legacy change 缺 file_log）→ stderr 诊断 + exit 0，不阻塞观察的工具调用', () => {
    bindSession(root, 'S1', 'legacy-change');
    createChangeFixture(root, 'legacy-change', {
      workflow_type: 'requirement',
      created: '2026-09-18',
      active_phase: ACTIVE_IMPLEMENT,
      // 无 file_log：机制前旧 change
    });
    const legacyDir = nodePath.join(root, 'openspec', 'changes', 'legacy-change');
    const before = readRawWorkflow(legacyDir);

    record(writeEvent({ agent_type: 'dev-team:implementation-generator' }));

    expect(stderrLines.join('')).toContain('record-files:');
    expect(stderrLines.join('')).toContain('请重建');
    expect(readRawWorkflow(legacyDir)).toBe(before);
    expect(exitMock).not.toHaveBeenCalled();
  });
});
