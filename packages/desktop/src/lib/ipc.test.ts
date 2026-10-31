import type { Channel } from '@tauri-apps/api/core';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';
import { type ZodObject, type ZodType } from 'zod';

import type { AgentRunMessage, FileWatchEvent, RunNotice } from '../types/dto';
import { type ArchiDriftMode, commands } from '../types/generated/bindings';
import { argsSchemas, channelArgs, resultSchemas } from '../types/generated/bindings.zod';
import { ipc } from './ipc';
import { driftCount, driftReports, reportDrift, resetDrift } from './ipc-drift';

// ---------------------------------------------------------------------------
// 进程边界 Mock：`@tauri-apps/api/core` 的 `invoke` 与 `Channel`。
//
// `bindings.ts` / `bindings.zod.ts` / `ipc-drift.ts` 一律**真实组合**——三表是契约
// 事实来源，替身化即绕开被测对象。
//
// 信封面用例一律按**真实消费形态**接线：`onmessage` 在 `ipc.<cmd>(…)` 之前就已
// 设置（`agent-transport` / `use-change-flow-run` / `use-explore-doc` 三个消费点
// 均先接线再调用），故「包裹需覆盖已接线的 handler」是 AC-3 的判据而非测试细节。
//
// `Channel` 备两份替身：
// - `ChannelMock`：原型无 accessor、`onmessage` 为实例字段（既有各测试沿用的最
//   小面），覆盖「原型无 accessor 时 get 返回调用方 handler」的兼容分支；
// - `AccessorChannelMock`：与真实 `Channel` 同构（原型 accessor + 私有字段 +
//   `deliver()` 投递），装配层把稳定 forwarder 写进私有字段的路径只能在此形态下
//   观测——信封漂移上报依赖它。
// ---------------------------------------------------------------------------

const { ChannelMock, AccessorChannelMock, invokeMock } = vi.hoisted(() => {
  class ChannelMock {
    onmessage: ((message: unknown) => void) | null = null;
    id = 101;
    toJSON(): string {
      return JSON.stringify({ id: this.id });
    }
  }

  class AccessorChannelMock {
    #onmessage: (message: unknown) => void;
    id = 202;

    constructor(handler: (message: unknown) => void = () => {}) {
      this.#onmessage = handler;
    }

    get onmessage(): (message: unknown) => void {
      return this.#onmessage;
    }

    set onmessage(handler: (message: unknown) => void) {
      this.#onmessage = handler;
    }

    /** Rust → 前端投递：读私有字段（与真实 `Channel` 的 deliver 同构） */
    deliver(message: unknown): void {
      this.#onmessage(message);
    }

    toJSON(): string {
      return JSON.stringify({ id: this.id });
    }
  }

  return { AccessorChannelMock, ChannelMock, invokeMock: vi.fn() };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock, Channel: ChannelMock }));

// ---------------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------------

const ROOT = 'C:\\demo\\alpha';

/** 合法 `TurnSummary`（`AgentRunMessage` 的 `record` 臂承载类型）。 */
function turnSummary(): Record<string, unknown> {
  return {
    turnId: 1,
    sessionId: 's-1',
    status: 'running',
    startedAt: 1727000000000,
    finishedAt: null,
    numTurns: null,
    costUsd: null,
    durationMs: null,
    error: null,
  };
}

/** 合法 `ChangeList`。 */
function changeList(name = 'add-feature'): Record<string, unknown> {
  return {
    active: [
      {
        id: `id-${name}`,
        name,
        title: name,
        source: 'active',
        status: 'active',
        activePhase: null,
        created: '2026-10-26',
      },
    ],
    archiveGroups: [],
  };
}

/** 合法 `AgentRunMessage` 信封（`record` 臂）。 */
function validEnvelope(): Record<string, unknown> {
  return { ipc: 'record', record: turnSummary() };
}

/** 脏信封：`turnId` 声明为 number 实为字符串。 */
function dirtyEnvelope(): Record<string, unknown> {
  return { ipc: 'record', record: { ...turnSummary(), turnId: 'not-a-number' } };
}

/** 千级行的 `RecordsPage`（`RecordEnvelope.key/value` 均为 `unknown` 白洞面）。 */
function bigRecordsPage(size: number): Record<string, unknown> {
  return {
    records: Array.from({ length: size }, (_, index) => ({
      key: index,
      value: { nested: [index, `row-${index}`], flag: index % 2 === 0 },
    })),
    total: size,
  };
}

/** 合法 `ArtifactEnvelope`（`payload` 为 `serde_json::Value` 出线白洞）。 */
function artifactEnvelope(payload: unknown): Record<string, unknown> {
  return { kind: 'proposal', version: 1, title: '提案', payload, fallbackText: null };
}

/** 合法 `AgentProviderRecord`。 */
function providerRecord(): Record<string, unknown> {
  return {
    id: 1,
    name: 'prov',
    protocol: 'openai',
    baseUrl: 'https://api.example.com',
    apiKey: 'sk-1',
    models: { high: 'high-model', medium: 'medium-model', low: 'low-model' },
    contextLength: null,
    maxTokens: null,
    webSearch: true,
  };
}

/** 各命令的最小合法结果（「参数面放行 + 结果面干净」用例的默认返回值）。 */
function validResultFor(commandName: string): unknown {
  switch (commandName) {
    case 'list_changes':
      return changeList();
    case 'agent_start':
      return turnSummary();
    case 'save_agent_provider':
      return providerRecord();
    case 'change_flow_start':
      return { runId: 'run-1', status: 'running' };
    case 'watch_subscribe':
      return 7;
    case 'db_records':
      return bigRecordsPage(2);
    default:
      return null;
  }
}

/** 按命令名分发合法结果（结果面零误报的默认装置）。 */
function mockValidResults(): void {
  invokeMock.mockImplementation((commandName: string) =>
    Promise.resolve(validResultFor(commandName)),
  );
}

// ---------------------------------------------------------------------------
// 观测辅助
// ---------------------------------------------------------------------------

/** 静音并观测 `console.warn`（DEV 汇总告警的观测面，不改行为）。 */
function spyWarn() {
  return vi.spyOn(console, 'warn').mockImplementation(() => {});
}

/** 本地拒绝的错误形态（`IpcArgsError` 未导出，按声明面观测）。 */
interface ArgsErrorView {
  name: string;
  message: string;
  cmd: string;
  path: string;
  expected: string;
  got: unknown;
}

/** 取调用的拒绝值（成功则判失败：违反「本地拒绝」判据）。 */
async function rejectionOf(pending: Promise<unknown>): Promise<unknown> {
  return pending.then(
    () => {
      throw new Error('期望本地参数拒绝，但调用成功返回');
    },
    (error: unknown) => error,
  );
}

/** 断言拒绝值确为本地参数拒绝，并返回其可观测形态。 */
function expectArgsError(error: unknown): ArgsErrorView {
  expect(error).toBeInstanceOf(Error);
  const view = error as ArgsErrorView;
  expect(view.name).toBe('IpcArgsError');
  return view;
}

/** 构造一份「原型带 accessor」的 Channel 替身（投递经私有字段）。 */
function accessorChannel<T>(): { channel: Channel<T>; deliver: (message: unknown) => void } {
  const raw = new AccessorChannelMock();
  return {
    channel: raw as unknown as Channel<T>,
    deliver: (message: unknown) => {
      raw.deliver(message);
    },
  };
}

/** 构造一份「原型无 accessor」的 Channel 替身（`onmessage` 为实例字段）。 */
function plainChannel<T>(): { channel: Channel<T>; deliver: (message: unknown) => void } {
  const raw = new ChannelMock();
  return {
    channel: raw as unknown as Channel<T>,
    deliver: (message: unknown) => {
      raw.onmessage?.(message);
    },
  };
}

/** 取出某个命令调用时传给 `invoke` 的 payload（第二实参）。 */
function payloadOf(commandName: string): Record<string, unknown> {
  const call = invokeMock.mock.calls.find(([name]) => name === commandName);
  if (call === undefined) throw new Error(`未观测到命令 ${commandName} 的调用`);
  return call[1] as Record<string, unknown>;
}

let warnSpy: ReturnType<typeof spyWarn>;

beforeEach(() => {
  invokeMock.mockReset();
  resetDrift();
  warnSpy = spyWarn();
});

afterEach(() => {
  warnSpy.mockRestore();
  vi.unstubAllEnvs();
});

// ---------------------------------------------------------------------------
// 键遍历面：命令集合全部来自 `commands`，装配层零手写清单
// ---------------------------------------------------------------------------

describe('ipc 键遍历面', () => {
  it('Object.keys(ipc) 与 Object.keys(commands) 完全一致（长度与逐项相等）', () => {
    expect(Object.keys(ipc)).toEqual(Object.keys(commands));
    expect(Object.keys(ipc)).toHaveLength(Object.keys(commands).length);
  });

  it.each(Object.keys(commands))('命令键 %s 取值为函数（键遍历自动覆盖新命令）', (key) => {
    expect(typeof Reflect.get(ipc, key)).toBe('function');
  });

  it.each(['then', 'toString', 'hasOwnProperty'])('非自有键 %s 原样透传：不包裹、不抛', (key) => {
    const value = Reflect.get(ipc, key);

    if (key === 'then') {
      expect(value).toBeUndefined();
    } else {
      expect(value).toBe(Reflect.get(commands, key));
      expect(typeof value).toBe('function');
    }
    expect(Reflect.get(ipc, 'notACommand')).toBeUndefined();
  });

  it('符号键原样透传：Symbol.toStringTag / Symbol.iterator 均不包裹', () => {
    expect(() => Reflect.get(ipc, Symbol.toStringTag)).not.toThrow();
    expect(Reflect.get(ipc, Symbol.toStringTag)).toBe(Reflect.get(commands, Symbol.toStringTag));
    expect(() => Reflect.get(ipc, Symbol.iterator)).not.toThrow();
  });

  it('按名缓存：同一命令两次取值同一引用，不同命令引用不同（React 依赖不抖动）', () => {
    expect(ipc.addWorkspace).toBe(ipc.addWorkspace);
    expect(ipc.agentStart).toBe(ipc.agentStart);
    expect(ipc.addWorkspace).not.toBe(ipc.removeWorkspace);
  });

  it('访问不存在的命令键 → undefined，不抛异常', () => {
    expect(() => Reflect.get(ipc, 'definitelyNotACommand')).not.toThrow();
    expect(Reflect.get(ipc, 'definitelyNotACommand')).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// 参数面：strict，失败本地抛 IpcArgsError（零 IPC 往返）
// ---------------------------------------------------------------------------

describe('ipc 参数面：合法输入放行', () => {
  it('listChanges(root) 放行：invoke 恰 1 次、payload 只含形参键', async () => {
    const list = changeList();
    invokeMock.mockResolvedValue(list);

    const result = await ipc.listChanges(ROOT);

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(payloadOf('list_changes')).toEqual({ root: ROOT });
    expect(Object.keys(payloadOf('list_changes'))).toEqual(['root']);
    expect(result).toBe(list);
  });

  it('agentSessions 的两个可空位给 null 放行、payload 键名 lowerCamelCase 逐字命中', async () => {
    const page = { sessions: [], total: 0 };
    invokeMock.mockResolvedValue(page);

    await ipc.agentSessions(ROOT, null, null, 0, 100);

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(payloadOf('agent_sessions')).toEqual({
      root: ROOT,
      source: null,
      sourceRef: null,
      offset: 0,
      limit: 100,
    });
  });

  it('saveAgentProvider 的 9 形参齐全、数值可空位给 null 放行', async () => {
    mockValidResults();

    await ipc.saveAgentProvider(
      null,
      'prov',
      'openai',
      'https://api.example.com',
      'sk-1',
      { high: 'high-model', medium: 'medium-model', low: 'low-model' },
      null,
      null,
      true,
    );

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(Object.keys(payloadOf('save_agent_provider'))).toEqual([
      'id',
      'name',
      'protocol',
      'baseUrl',
      'apiKey',
      'models',
      'contextLength',
      'maxTokens',
      'webSearch',
    ]);
  });

  it('合法调用零漂移：driftCount() 为 0 且无 console.warn', async () => {
    invokeMock.mockResolvedValue(changeList());

    await ipc.listChanges(ROOT);

    expect(driftCount()).toBe(0);
    expect(warnSpy).not.toHaveBeenCalled();
  });

  it.each([
    ['listWorkspaces', 'list_workspaces'],
    ['appEnvironment', 'app_environment'],
  ])('零形参命令 %s：z.strictObject({}) 放行、invoke 恰 1 次', async (accessor, commandName) => {
    invokeMock.mockResolvedValue([]);

    const loose = Reflect.get(ipc, accessor) as () => Promise<unknown>;
    await loose();

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock.mock.calls[0][0]).toBe(commandName);
  });

  it('多传超出形参个数的实参：本地不拒、payload 按键序取位', async () => {
    invokeMock.mockResolvedValue(changeList());
    const loose = ipc.listChanges as unknown as (...args: unknown[]) => Promise<unknown>;

    await loose(ROOT, 'surplus-arg');

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(payloadOf('list_changes')).toEqual({ root: ROOT });
    expect(driftCount()).toBe(0);
  });
});

describe('ipc 参数面：契约外输入本地拒绝', () => {
  it('数字给 string 形参 → IpcArgsError（含命令名与 #/… 路径）、invoke 零调用', async () => {
    const error = await rejectionOf(ipc.listChanges(42 as unknown as string));

    const view = expectArgsError(error);
    expect(view.cmd).toBe('listChanges');
    expect(view.path).toBe('#/root');
    // 消息形态 = `命令 <accessor> 参数校验失败：` + prettify 文案
    expect(view.message).toMatch(/^命令 listChanges 参数校验失败：/);
    expect(view.message).toContain('listChanges');
    expect(view.message).toContain('root');
    // 类型错走 issue 的 `expected` 字段（而非 issue 文案）
    expect(view.expected).toBe('string');
    expect(view.got).toBe(42);
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('null 给必填 string 形参 → IpcArgsError，got 为 null', async () => {
    const error = await rejectionOf(ipc.listChanges(null as unknown as string));

    const view = expectArgsError(error);
    expect(view.got).toBeNull();
    expect(view.expected).toBe('string');
    expect(view.path).toBe('#/root');
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('少传实参 → IpcArgsError，path 指向首个缺失形参', async () => {
    const loose = ipc.agentSessions as unknown as (...args: unknown[]) => Promise<unknown>;

    const error = await rejectionOf(loose(ROOT));

    const view = expectArgsError(error);
    expect(view.cmd).toBe('agentSessions');
    expect(view.path).toBe('#/source');
    expect(view.got).toBeUndefined();
    expect(view.expected).toBe('string');
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('嵌套对象多余子键 → IpcArgsError（strictObject 深层同样被拒）', async () => {
    const models = {
      high: 'high-model',
      medium: 'medium-model',
      low: 'low-model',
      surplus: 'unexpected',
    };
    const error = await rejectionOf(
      ipc.saveAgentProvider(
        null,
        'prov',
        'openai',
        'https://api.example.com',
        'sk-1',
        models as unknown as Parameters<typeof ipc.saveAgentProvider>[5],
        null,
        null,
        false,
      ),
    );

    const view = expectArgsError(error);
    expect(view.cmd).toBe('saveAgentProvider');
    expect(view.path.startsWith('#/models')).toBe(true);
    // `unrecognized_keys` 不带 `expected` 字段 → 回落 issue 文案（非空且点名多余键）
    expect(view.expected).toContain('surplus');
    // `got` 取路径处原值：此处即传进去的 models 对象本体（同一引用）
    expect(view.got).toBe(models);
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('显式 undefined 给 string | null 形参 → IpcArgsError（只 null 可过，未放宽为 optional）', async () => {
    const error = await rejectionOf(ipc.createExploreRecord(ROOT, undefined as unknown as string));

    const view = expectArgsError(error);
    expect(view.cmd).toBe('createExploreRecord');
    expect(view.path).toBe('#/name');
    expect(view.expected).toBe('string');
    expect(view.got).toBeUndefined();
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('参数面失败不产生漂移记录（错误面不混入 IpcDriftReport）', async () => {
    await rejectionOf(ipc.listChanges(42 as unknown as string));

    expect(driftCount()).toBe(0);
    expect(driftReports()).toEqual([]);
    expect(warnSpy).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// 结果面：soft-report，原值透传（零 throw / 零改写）
// ---------------------------------------------------------------------------

describe('ipc 结果面：干净结果零记录', () => {
  it('干净结果 → 零漂移，返回值与后端返回对象同一引用', async () => {
    const list = changeList();
    invokeMock.mockResolvedValue(list);

    const result = await ipc.listChanges(ROOT);

    expect(result).toBe(list);
    expect(driftCount()).toBe(0);
    expect(warnSpy).not.toHaveBeenCalled();
  });

  it.each([
    ['对象', { nested: { value: 1 } }],
    ['数组', [1, 'two', null]],
    ['字符串', 'plain-text'],
    ['null', null],
  ])('payload 白洞给 %s → 合法 unknown 零漂移、原值返回', async (_label, payload) => {
    const envelope = artifactEnvelope(payload);
    invokeMock.mockResolvedValue(envelope);

    const result = await ipc.readArtifact(ROOT, 'change-1', 'proposal', 'active');

    expect(result).toBe(envelope);
    expect(driftCount()).toBe(0);
  });

  it('大结果（千级行）→ 零漂移、原引用透传（无深拷贝、无重排）', async () => {
    const page = bigRecordsPage(1000);
    invokeMock.mockResolvedValue(page);

    const result = await ipc.dbRecords('workspace', ROOT, 'sessions', 0, 1000);

    expect(result).toBe(page);
    expect(driftCount()).toBe(0);
  });
});

describe('ipc 结果面：脏结果只上报', () => {
  it('created 声明为 string 实为 number → 漂移 1 条、原引用返回、Promise 正常 resolve', async () => {
    const dirty = {
      active: [
        {
          id: 'id-1',
          name: 'add-feature',
          title: 'add-feature',
          source: 'active',
          status: 'active',
          activePhase: null,
          created: 42,
        },
      ],
      archiveGroups: [],
    };
    invokeMock.mockResolvedValue(dirty);

    const result = await ipc.listChanges(ROOT);

    expect(result).toBe(dirty);
    expect(driftCount()).toBe(1);
    const [entry] = driftReports();
    expect(entry.cmd).toBe('listChanges');
    expect(entry.path).toBe('#/active/0/created');
    expect(entry.expected).not.toBe('');
    expect(entry.got).toBe(42);
  });

  it('多层嵌套脏字段 → 漂移 path 逐段命中（#/a/0/b 形态）', async () => {
    invokeMock.mockResolvedValue({
      active: [
        {
          id: 'id-1',
          name: 'add-feature',
          title: 'add-feature',
          source: 'active',
          status: 'active',
          activePhase: { phase: 9, attempt: 0, startAt: null },
          created: null,
        },
      ],
      archiveGroups: [],
    });

    await ipc.listChanges(ROOT);

    expect(driftReports()[0].path).toBe('#/active/0/activePhase/phase');
  });

  it('结果整体类型不符（对象位置给标量）→ 漂移 path 为根 #、got 为原值', async () => {
    invokeMock.mockResolvedValue(42);

    const result = await ipc.listChanges(ROOT);

    expect(result).toBe(42);
    expect(driftCount()).toBe(1);
    expect(driftReports()[0].path).toBe('#');
    expect(driftReports()[0].expected).toBe('object');
    expect(driftReports()[0].got).toBe(42);
  });

  it('同 (cmd, path) 连续两次脏结果 → 1 条记录且 count 为 2', async () => {
    const dirty = { ...changeList(), archiveGroups: 42 };
    invokeMock.mockResolvedValue(dirty);

    await ipc.listChanges(ROOT);
    await ipc.listChanges(ROOT);

    expect(driftCount()).toBe(1);
    expect(driftReports()[0].count).toBe(2);
  });

  it('同一命令两处不同字段脏 → 两条独立漂移条目', async () => {
    invokeMock.mockResolvedValueOnce({ ...changeList(), archiveGroups: 42 });
    await ipc.listChanges(ROOT);

    invokeMock.mockResolvedValueOnce({
      active: 42,
      archiveGroups: [],
    });
    await ipc.listChanges(ROOT);

    expect(driftCount()).toBe(2);
    expect(driftReports().map((entry) => entry.path)).toEqual(['#/archiveGroups', '#/active']);
  });

  it('结果 1 条漂移落在首个 issue（单条上报，不膨胀为多条）', async () => {
    invokeMock.mockResolvedValue({ active: 42, archiveGroups: 43 });

    await ipc.listChanges(ROOT);

    expect(driftCount()).toBe(1);
    expect(driftReports()[0].path).toBe('#/active');
  });
});

// ---------------------------------------------------------------------------
// Channel 信封：逐条 soft-report、原消息原样交付、包裹幂等
// ---------------------------------------------------------------------------

describe('ipc Channel 信封', () => {
  it('合法信封 → handler 收到原消息（同一引用）、零漂移', async () => {
    const { channel, deliver } = accessorChannel<AgentRunMessage>();
    const handler = vi.fn();
    channel.onmessage = handler;
    const message = validEnvelope();
    invokeMock.mockResolvedValue(turnSummary());

    await ipc.agentStart(channel, ROOT, 'hello', 'default', null, null, null, null, null);
    deliver(message);

    expect(handler).toHaveBeenCalledTimes(1);
    expect(handler).toHaveBeenCalledWith(message);
    expect(handler.mock.calls[0][0]).toBe(message);
    expect(driftCount()).toBe(0);
  });

  it('脏信封 → 漂移恰 1 条，handler 仍收到原消息（不丢流、不改内容）', async () => {
    const { channel, deliver } = accessorChannel<AgentRunMessage>();
    const handler = vi.fn();
    channel.onmessage = handler;
    const message = dirtyEnvelope();
    invokeMock.mockResolvedValue(turnSummary());

    await ipc.agentStart(channel, ROOT, 'hello', 'default', null, null, null, null, null);
    deliver(message);

    expect(driftCount()).toBe(1);
    expect(driftReports()[0].cmd).toBe('agentStart');
    // `AgentRunMessage` 为 union：zod 以单条 `invalid_union` issue 表达（path 为根）
    expect(driftReports()[0].path).toBe('#');
    // 非 `invalid_type` issue 无 `expected` 字段 → 回落 issue 文案（`invalid_union` 为 "Invalid input"）
    expect(driftReports()[0].expected).toBe('Invalid input');
    expect(handler).toHaveBeenCalledTimes(1);
    expect(handler.mock.calls[0][0]).toBe(message);
  });

  it('序列「合法 → 脏 → 合法」→ handler 三次、调用顺序不变、漂移恰 1 条', async () => {
    const { channel, deliver } = accessorChannel<AgentRunMessage>();
    const seen: unknown[] = [];
    channel.onmessage = (message: unknown) => {
      seen.push(message);
    };
    const first = validEnvelope();
    const dirty = dirtyEnvelope();
    const last = validEnvelope();
    invokeMock.mockResolvedValue(turnSummary());

    await ipc.agentStart(channel, ROOT, 'hello', 'default', null, null, null, null, null);
    deliver(first);
    deliver(dirty);
    deliver(last);

    expect(seen).toHaveLength(3);
    expect(seen[0]).toBe(first);
    expect(seen[1]).toBe(dirty);
    expect(seen[2]).toBe(last);
    expect(driftCount()).toBe(1);
  });

  it('同一 channel 对同一命令连续传入两次 → 只包裹一次：每条消息 handler 1 次、脏消息只 1 条漂移', async () => {
    const { channel, deliver } = accessorChannel<AgentRunMessage>();
    const handler = vi.fn();
    channel.onmessage = handler;
    invokeMock.mockResolvedValue(turnSummary());

    await ipc.agentStart(channel, ROOT, 'hello', 'default', null, null, null, null, null);
    await ipc.agentStart(channel, ROOT, 'hello', 'default', null, null, null, null, null);
    deliver(dirtyEnvelope());

    expect(handler).toHaveBeenCalledTimes(1);
    expect(driftCount()).toBe(1);
    expect(driftReports()[0].count).toBe(1);
  });

  it('同一 channel 传入两条不同命令 → WeakSet 短路，不重复包裹、不重复上报', async () => {
    const { channel, deliver } = accessorChannel<AgentRunMessage>();
    const handler = vi.fn();
    channel.onmessage = handler;
    invokeMock.mockResolvedValue(turnSummary());

    await ipc.agentStart(channel, ROOT, 'hello', 'default', null, null, null, null, null);
    invokeMock.mockResolvedValue(null);
    await ipc.agentSessionWatch(channel, ROOT, 's-1', 0);
    deliver(dirtyEnvelope());

    expect(handler).toHaveBeenCalledTimes(1);
    expect(driftCount()).toBe(1);
    expect(driftReports()[0].count).toBe(1);
    // 首个出口的包裹保持生效：第二条命令不重装 forwarder（否则上报会换到 agentSessionWatch）
    expect(driftReports()[0].cmd).toBe('agentStart');
  });

  it('onEvent 键保留在 invoke 第二实参内（同一引用）且不参与值校验', async () => {
    const { channel } = accessorChannel<AgentRunMessage>();
    invokeMock.mockResolvedValue(turnSummary());

    await ipc.agentStart(
      channel,
      ROOT,
      'hello',
      'acceptEdits',
      's-1',
      'change',
      'add-feature',
      3,
      null,
    );

    const payload = payloadOf('agent_start');
    expect(payload.onEvent).toBe(channel);
    expect(Object.keys(payload)).toContain('onEvent');
    expect(driftCount()).toBe(0);
  });

  it('Channel 形参位传 null（非 Channel 实例）→ 本地不抛、跳过包裹', async () => {
    invokeMock.mockResolvedValue(null);

    await expect(
      ipc.agentSessionWatch(null as unknown as Channel<AgentRunMessage>, ROOT, 's-1', 0),
    ).resolves.toBeNull();
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(driftCount()).toBe(0);
  });

  it('handler 未设置时投递不抛；随后设置 handler 再投递 → 正常交付且校验生效', async () => {
    const { channel, deliver } = accessorChannel<AgentRunMessage>();
    invokeMock.mockResolvedValue(turnSummary());

    await ipc.agentStart(channel, ROOT, 'hello', 'default', null, null, null, null, null);

    expect(() => {
      deliver(validEnvelope());
    }).not.toThrow();
    expect(driftCount()).toBe(0);

    const handler = vi.fn();
    channel.onmessage = handler;
    const message = dirtyEnvelope();
    deliver(message);

    expect(handler).toHaveBeenCalledTimes(1);
    expect(handler.mock.calls[0][0]).toBe(message);
    expect(driftCount()).toBe(1);
  });

  it('包裹不改变其它可观察行为：id 与 toJSON() 结果与包裹前逐字一致', async () => {
    const { channel } = accessorChannel<AgentRunMessage>();
    const idBefore = channel.id;
    const jsonBefore = channel.toJSON();
    invokeMock.mockResolvedValue(turnSummary());

    await ipc.agentStart(channel, ROOT, 'hello', 'default', null, null, null, null, null);

    expect(channel.id).toBe(idBefore);
    expect(channel.toJSON()).toBe(jsonBefore);
  });

  it('原型带 accessor 的替身：私有字段仍经原型 setter 写入（get / set 两条 descriptor 路径）', async () => {
    const { channel, deliver } = accessorChannel<AgentRunMessage>();
    const descriptor = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(channel), 'onmessage');
    expect(typeof descriptor?.set).toBe('function');
    expect(typeof descriptor?.get).toBe('function');
    invokeMock.mockResolvedValue(turnSummary());

    await ipc.agentStart(channel, ROOT, 'hello', 'default', null, null, null, null, null);

    const handler = vi.fn();
    channel.onmessage = handler;
    expect(channel.onmessage).toBe(handler);
    deliver(validEnvelope());
    expect(handler).toHaveBeenCalledTimes(1);
  });

  it('原型无 accessor 的替身：不抛，get 返回调用方 handler（兼容面不被包裹收紧）', async () => {
    const { channel, deliver } = plainChannel<AgentRunMessage>();
    invokeMock.mockResolvedValue(turnSummary());

    await ipc.agentStart(channel, ROOT, 'hello', 'default', null, null, null, null, null);
    // 未接线（实例字段为 null）时 get 仍返回可调用的 forwarder，而非 null/undefined
    expect(typeof channel.onmessage).toBe('function');
    const handler = vi.fn();
    channel.onmessage = handler;
    expect(channel.onmessage).toBe(handler);
    deliver(validEnvelope());

    expect(handler).toHaveBeenCalledTimes(1);
    expect(driftCount()).toBe(0);
  });

  it('handler 自身抛异常 → 原样向投递方传播（装配层不吞、不转成漂移）', async () => {
    const { channel, deliver } = accessorChannel<AgentRunMessage>();
    channel.onmessage = () => {
      throw new Error('handler 炸了');
    };
    invokeMock.mockResolvedValue(turnSummary());

    await ipc.agentStart(channel, ROOT, 'hello', 'default', null, null, null, null, null);

    expect(() => {
      deliver(validEnvelope());
    }).toThrow('handler 炸了');
    expect(driftCount()).toBe(0);
  });
});

// ---------------------------------------------------------------------------
// 错误面三分：后端 reject 串 / IpcArgsError / 漂移报告
// ---------------------------------------------------------------------------

describe('ipc 错误面三分', () => {
  it('后端 reject 字符串 → 调用方收到同一字符串、零漂移、非 IpcArgsError', async () => {
    invokeMock.mockRejectedValue('开库失败：/repo/ws.db');

    const error = await rejectionOf(ipc.listChanges(ROOT));

    expect(typeof error).toBe('string');
    expect(error).toBe('开库失败：/repo/ws.db');
    expect(String(error)).toBe('开库失败：/repo/ws.db');
    expect(driftCount()).toBe(0);
  });

  it.each([
    ['Error 实例', new Error('store 读失败')],
    ['普通对象', { code: 'E_BROKEN', detail: 'db' }],
  ])('后端 reject 为 %s → 原样向上传递（不改造、不包装成字符串）', async (_label, rejection) => {
    invokeMock.mockRejectedValue(rejection);

    const error = await rejectionOf(ipc.listChanges(ROOT));

    expect(error).toBe(rejection);
    expect(driftCount()).toBe(0);
  });

  it('IpcArgsError 经 String(err) 呈现含命令名与 #/… 路径（既有 error 态路径零改动）', async () => {
    const error = await rejectionOf(ipc.listChanges(42 as unknown as string));

    const rendered = String(error);
    expect(rendered).toContain('IpcArgsError');
    expect(rendered).toContain('listChanges');
    // jsonPointer 路径由错误对象的 `path` 字段承载，prettify 文案里以字段名呈现
    expect((error as ArgsErrorView).path).toBe('#/root');
    expect(rendered).toContain('root');
  });

  it('结果面漂移时 Promise 正常 resolve（零 reject、不抛 IpcArgsError）', async () => {
    const dirty = { ...changeList(), archiveGroups: 42 };
    invokeMock.mockResolvedValue(dirty);

    await expect(ipc.listChanges(ROOT)).resolves.toBe(dirty);
    expect(driftCount()).toBe(1);
  });
});

// ---------------------------------------------------------------------------
// DEV 汇总告警：诊断输出可见性门（只影响输出，不影响记录）
// ---------------------------------------------------------------------------

describe('ipc DEV 汇总告警', () => {
  it('首次出现新漂移键 → console.warn 恰 1 次，实参含命令名、path、键数与键清单', async () => {
    invokeMock.mockResolvedValue({ ...changeList(), archiveGroups: 42 });

    await ipc.listChanges(ROOT);

    expect(warnSpy).toHaveBeenCalledTimes(1);
    const [message, reports] = warnSpy.mock.calls[0];
    // 告警形态：[ipc-drift] <命令> <path> 漂移（expected <期望>）；当前去重键 <n> 个
    expect(String(message)).toContain('[ipc-drift]');
    expect(String(message)).toContain('listChanges');
    expect(String(message)).toContain('#/archiveGroups');
    expect(String(message)).toContain('expected array');
    expect(String(message)).toContain('当前去重键 1 个');
    expect(String(message)).toContain('1');
    expect(reports).toEqual(driftReports());
    expect(driftCount()).toBe(1);
  });

  it('同一键再次漂移（计数增、键未新增）→ 不重复 warn（每去重键一次）', async () => {
    invokeMock.mockResolvedValue({ ...changeList(), archiveGroups: 42 });

    await ipc.listChanges(ROOT);
    await ipc.listChanges(ROOT);

    expect(warnSpy).toHaveBeenCalledTimes(1);
    expect(driftReports()[0].count).toBe(2);
  });

  it('DEV 门关闭时 → 漂移照旧记录但零 warn（门只决定诊断输出可见性）', async () => {
    vi.stubEnv('DEV', false);
    invokeMock.mockResolvedValue({ ...changeList(), archiveGroups: 42 });

    await ipc.listChanges(ROOT);

    expect(warnSpy).not.toHaveBeenCalled();
    expect(driftCount()).toBe(1);
  });
});

// ---------------------------------------------------------------------------
// 装配层 × collector 组合：上报与去重累计一致
// ---------------------------------------------------------------------------

describe('ipc 与 ipc-drift 组合', () => {
  it('脏结果调用 → driftReports() 1 条且 count 等于该键上报次数', async () => {
    invokeMock.mockResolvedValue({ ...changeList(), archiveGroups: 42 });

    await ipc.listChanges(ROOT);
    await ipc.listChanges(ROOT);

    const entries = driftReports();
    expect(entries).toHaveLength(1);
    expect(entries[0].count).toBe(2);
    expect(driftCount()).toBe(1);
  });

  it('collector 达键数上限后继续产生新键 → 装配层不抛、不重试、driftCount() 恒为 200', async () => {
    for (let index = 0; index < 200; index += 1) {
      const active = Array.from({ length: index + 1 }, (_, position) => ({
        id: `id-${position}`,
        name: `n-${position}`,
        title: `t-${position}`,
        source: 'active',
        status: 'active',
        activePhase: null,
        created: position === index ? 42 : null,
      }));
      invokeMock.mockResolvedValueOnce({ active, archiveGroups: [] });
      await ipc.listChanges(ROOT);
    }
    expect(driftCount()).toBe(200);

    invokeMock.mockResolvedValueOnce({ ...changeList(), archiveGroups: 42 });
    await expect(ipc.listChanges(ROOT)).resolves.toBeDefined();
    expect(driftCount()).toBe(200);
  });

  it('resetDrift() 后同一脏结果再次调用 → 重新累计 1 条（不续接旧计数）', async () => {
    invokeMock.mockResolvedValue({ ...changeList(), archiveGroups: 42 });
    await ipc.listChanges(ROOT);
    expect(driftReports()[0].count).toBe(1);

    resetDrift();
    await ipc.listChanges(ROOT);

    expect(driftCount()).toBe(1);
    expect(driftReports()[0].count).toBe(1);
  });

  it('collector 预置漂移后装配层上报 → 两条共存（跨模块不重复计数）', async () => {
    reportDrift({ cmd: 'preset', path: '#', expected: 'string', got: 1 });
    invokeMock.mockResolvedValue({ ...changeList(), archiveGroups: 42 });

    await ipc.listChanges(ROOT);

    expect(driftCount()).toBe(2);
    expect(driftReports().map((entry) => entry.count)).toEqual([1, 1]);
  });
});

// ---------------------------------------------------------------------------
// 装配层 × bindings.zod 组合：三表驱动（无镜像、无值嗅探）
// ---------------------------------------------------------------------------

describe('ipc 与 bindings.zod 组合', () => {
  const channelCommands = [
    'agentStart',
    'agentSessionWatch',
    'changeFlowStart',
    'changeFlowWatch',
    'watchSubscribe',
  ] as const;

  it.each(channelCommands)('%s 的 channelArgs 表项恰为 onEvent（信封 schema 命中）', (cmd) => {
    const table = Reflect.get(channelArgs, cmd) as Record<string, unknown>;

    expect(Object.keys(table)).toEqual(['onEvent']);
    expect(driftCount()).toBe(0);
  });

  it('5 个 Channel 命令全部经 ipc 调用 → 参数面放行、onEvent 原样在 payload', async () => {
    const agent = accessorChannel<AgentRunMessage>();
    const notice = accessorChannel<RunNotice>();
    const watch = accessorChannel<FileWatchEvent>();
    mockValidResults();

    await ipc.agentStart(agent.channel, ROOT, 'p', 'default', null, null, null, null, null);
    await ipc.agentSessionWatch(agent.channel, ROOT, 's-1', 0);
    await ipc.changeFlowStart(notice.channel, ROOT, 'change-1', true);
    await ipc.changeFlowWatch(notice.channel, ROOT, 'change-1');
    await ipc.watchSubscribe(watch.channel, '/repo/doc.md');

    expect(payloadOf('agent_start').onEvent).toBe(agent.channel);
    expect(payloadOf('agent_session_watch').onEvent).toBe(agent.channel);
    expect(payloadOf('change_flow_start').onEvent).toBe(notice.channel);
    expect(payloadOf('change_flow_watch').onEvent).toBe(notice.channel);
    expect(payloadOf('watch_subscribe').onEvent).toBe(watch.channel);
    expect(driftCount()).toBe(0);
  });

  it.each([
    ['listChanges', () => ipc.listChanges(ROOT)],
    ['dbRecords', () => ipc.dbRecords('workspace', ROOT, 'sessions', 0, 20)],
  ])('无 Channel 形参的命令 %s → 跳过包裹、不抛、信封面零上报', async (_label, call) => {
    mockValidResults();

    await expect(call()).resolves.toBeDefined();

    expect(Object.keys(Reflect.get(channelArgs, _label) as object)).toEqual([]);
    expect(driftCount()).toBe(0);
  });

  it('argsSchemas 表项为 getter：连续两次调用判定一致（getter 重建 × 判定稳定）', async () => {
    const descriptor = Object.getOwnPropertyDescriptor(argsSchemas, 'listChanges');
    expect(typeof descriptor?.get).toBe('function');
    invokeMock.mockResolvedValue(changeList());

    await ipc.listChanges(ROOT);
    await ipc.listChanges(ROOT);

    expect(invokeMock).toHaveBeenCalledTimes(2);
    expect(driftCount()).toBe(0);
    const resultDescriptor = Object.getOwnPropertyDescriptor(resultSchemas, 'listChanges');
    expect(typeof resultDescriptor?.get).toBe('function');
  });

  it('结果面表项为 getter：每次访问重建但判定结果一致（无状态残留）', async () => {
    const dirty = { ...changeList(), archiveGroups: 42 };
    invokeMock.mockResolvedValue(dirty);

    await ipc.listChanges(ROOT);
    await ipc.listChanges(ROOT);

    expect(driftCount()).toBe(1);
    expect(driftReports()[0].count).toBe(2);
  });
});

// ---------------------------------------------------------------------------
// 形参名面：逐命令 `argsSchemas` 的形参名集合 / 键序与 `bindings.ts` 的 payload 键集
// 逐字一致。design R-4：形参名在类型层不可得（`Parameters<…>` 只给位置元组），名称
// 权威只有「生成期 `Casing::CamelCase`」与「运行期 `invoke` 第二实参键集」两处——
// 后者是唯一能逐命令把两个**独立生成面**（TS 包装 / zod 表）对位比较的观测点。
// ---------------------------------------------------------------------------

describe('ipc 形参名面：逐命令 payload 键集 == argsSchemas.shape 键集', () => {
  it.each(Object.keys(commands))('%s 的形参名集合与键序逐字一致', async (key) => {
    invokeMock.mockResolvedValue(null);
    const schema = Reflect.get(argsSchemas, key) as ZodObject;
    const shapeKeys = Object.keys(schema.shape);
    const raw: unknown = Reflect.get(commands, key);
    if (typeof raw !== 'function') throw new Error(`命令 ${key} 不是函数`);

    // 直接调 `bindings.ts` 的包装函数：payload 键由 TS 生成物自行给出、不经 `ipc`
    // 参数面装配，于是两处独立生成的形参名（名字 + 序）在此逐命令对位比较
    const call = raw as (...args: unknown[]) => Promise<unknown>;
    await call(...shapeKeys.map(() => undefined));

    const [commandName, payload] = invokeMock.mock.calls[0];
    expect(typeof commandName).toBe('string');
    expect(Object.keys((payload ?? {}) as Record<string, unknown>)).toEqual(shapeKeys);
  });
});

// ---------------------------------------------------------------------------
// `archi_drift` 命令面
// ---------------------------------------------------------------------------

/** 合法结构诊断（5 字段，`column` 非 null）。 */
function driftDiagnostic(): Record<string, unknown> {
  return {
    kind: 'cycle',
    message: '元素 App.Auth 依赖祖先成环',
    file: '.dev-team/architecture/models/01-app.toml',
    line: 12,
    column: 3,
  };
}

/** 合法命中点（4 字段）。 */
function driftOccurrence(): Record<string, unknown> {
  return { file: 'src/billing/x.ts', line: 7, specifier: '../auth/token', typeOnly: false };
}

/** 合法漂移条目（6 字段，含 1 条 occurrence）。 */
function driftEntry(kind = 'undeclaredDependency'): Record<string, unknown> {
  return {
    kind,
    from: 'App.Auth',
    to: 'App.Billing',
    gate: 'strict',
    message: '未声明依赖 App.Auth -> App.Billing',
    occurrences: [driftOccurrence()],
  };
}

/**
 * 全字段 `ArchiDriftReport`：14 个根级字段全部非空，嵌套面 model 5 / scan 7 / 条目 6 /
 * 命中点 4 / 结构诊断 5 各至少一项（单次 parse 即覆盖 drift / scan 图全部字段 getter）。
 */
function archiDriftReport(): Record<string, unknown> {
  return {
    version: 1,
    timestamp: '2026-10-20T08:31:12Z',
    mode: 'incremental',
    root: '/repo',
    change: 'archi-check-and-gate',
    base: 'HEAD',
    modelHash: 'sha256:9f2c1e',
    model: {
      elements: 50,
      relations: 111,
      byGate: { strict: 2, warn: 3, off: 45 },
      structuralErrors: [driftDiagnostic()],
      warnings: [driftEntry('noPath')],
    },
    scan: {
      mode: 'incremental',
      changedFiles: 12,
      scannedFiles: 10,
      excluded: [{ file: 'src/auth/auth.test.ts', rule: '**/*.test.ts' }],
      unmatchedFiles: ['src/shared/token.ts'],
      matched: [{ element: 'App.Auth', files: ['src/auth/token.ts'] }],
      unresolved: [{ file: 'src/dyn.ts', specifier: './dyn', message: '动态导入' }],
    },
    errors: [driftEntry()],
    warnings: [driftEntry('ambiguousOwner')],
    suppressed: [
      {
        from: 'App.Auth',
        to: 'App.Billing',
        via: 'inline',
        reason: '迁移期',
        file: 'src/auth/token.ts',
        line: 3,
      },
    ],
    observations: [
      {
        kind: 'unfulfilledDeclaration',
        from: 'App.Auth',
        to: 'App.Billing',
        message: '本次扫描面内未见该 import',
      },
    ],
    status: 'errors',
  };
}

/** `resultSchemas.archiDrift` 表项（getter → `z.lazy`，每次读取新实例）。 */
function archiDriftSchema(): ZodType {
  return Reflect.get(resultSchemas, 'archiDrift') as ZodType;
}

/** 报告 payload 的 parse 是否通过。 */
function parsesReport(payload: unknown): boolean {
  return archiDriftSchema().safeParse(payload).success;
}

/** 深路径处的父容器（对象 / 数组同式：下标以字符串表达）。 */
function parentAt(root: unknown, segments: ReadonlyArray<string>): Record<string, unknown> {
  return segments
    .slice(0, -1)
    .reduce<Record<string, unknown>>(
      (node, segment) => Reflect.get(node, segment) as Record<string, unknown>,
      root as Record<string, unknown>,
    );
}

/** fixture 的形变副本：深路径赋错值 / 新增多余键（每次新建，不污染共享 fixture）。 */
function reportWith(segments: ReadonlyArray<string>, value: unknown): Record<string, unknown> {
  const report = archiDriftReport();
  parentAt(report, segments)[segments[segments.length - 1]] = value;
  return report;
}

/** fixture 的形变副本：深路径删除必填键。 */
function reportWithout(segments: ReadonlyArray<string>): Record<string, unknown> {
  const report = archiDriftReport();
  delete parentAt(report, segments)[segments[segments.length - 1]];
  return report;
}

/** 首个 issue 的 jsonPointer 路径（`#/a/0/b` 形态）。 */
function firstIssuePath(payload: unknown): string {
  const outcome = archiDriftSchema().safeParse(payload);
  if (outcome.success) throw new Error('期望 parse 失败，但 payload 通过');
  const [first] = outcome.error.issues;
  return `#${first.path.map((segment) => `/${String(segment)}`).join('')}`;
}

/** 根级 14 个必填键（`ArchiDriftReport` 字段面全集）。 */
const ARCHI_ROOT_KEYS = [
  'version',
  'timestamp',
  'mode',
  'root',
  'change',
  'base',
  'modelHash',
  'model',
  'scan',
  'errors',
  'warnings',
  'suppressed',
  'observations',
  'status',
] as const;

/** 根级 14 键的错类型取样（`version` 用 `null`：它的 number 面给 42 会合法）。 */
const ARCHI_ROOT_WRONG_TYPES: ReadonlyArray<[string, unknown]> = [
  ['version', null],
  ['timestamp', 42],
  ['mode', 'dirty'],
  ['root', 42],
  ['change', 42],
  ['base', 42],
  ['modelHash', 42],
  ['model', []],
  ['scan', 'scan'],
  ['errors', 'errors'],
  ['warnings', {}],
  ['suppressed', 'suppressed'],
  ['observations', 'observations'],
  ['status', 42],
];

/** 嵌套必填键 5 处代表（深一层到三层，含数组下标段）。 */
const ARCHI_NESTED_KEYS: ReadonlyArray<[string, string[]]> = [
  ['model.byGate.strict', ['model', 'byGate', 'strict']],
  ['scan.matched[0].files', ['scan', 'matched', '0', 'files']],
  ['errors[0].occurrences[0].typeOnly', ['errors', '0', 'occurrences', '0', 'typeOnly']],
  ['suppressed[0].reason', ['suppressed', '0', 'reason']],
  ['observations[0].to', ['observations', '0', 'to']],
];

/** 嵌套键 5 处的错类型取样。 */
const ARCHI_NESTED_WRONG_TYPES: ReadonlyArray<[string, string[], unknown]> = [
  ['model.elements', ['model', 'elements'], '50'],
  ['model.byGate', ['model', 'byGate'], 'byGate'],
  ['scan.excluded', ['scan', 'excluded'], 'excluded'],
  ['errors[0].occurrences[0].line', ['errors', '0', 'occurrences', '0', 'line'], '12'],
  ['suppressed[0].line', ['suppressed', '0', 'line'], 3.5],
];

/** 九处数组字段（`[]` 与单元素两态、规模面）。 */
const ARCHI_ARRAY_PATHS: ReadonlyArray<[string, string[]]> = [
  ['errors', ['errors']],
  ['warnings', ['warnings']],
  ['model.warnings', ['model', 'warnings']],
  ['model.structuralErrors', ['model', 'structuralErrors']],
  ['suppressed', ['suppressed']],
  ['observations', ['observations']],
  ['scan.excluded', ['scan', 'excluded']],
  ['scan.matched', ['scan', 'matched']],
  ['scan.unresolved', ['scan', 'unresolved']],
];

/** 十处可空位（`null` 与有值两态）。 */
const ARCHI_NULLABLE_PATHS: ReadonlyArray<[string, string[]]> = [
  ['change', ['change']],
  ['base', ['base']],
  ['errors[0].from', ['errors', '0', 'from']],
  ['errors[0].to', ['errors', '0', 'to']],
  ['errors[0].gate', ['errors', '0', 'gate']],
  ['errors[0].occurrences[0].specifier', ['errors', '0', 'occurrences', '0', 'specifier']],
  ['suppressed[0].file', ['suppressed', '0', 'file']],
  ['suppressed[0].line', ['suppressed', '0', 'line']],
  ['model.structuralErrors[0].column', ['model', 'structuralErrors', '0', 'column']],
  ['scan.unresolved[0].specifier', ['scan', 'unresolved', '0', 'specifier']],
];

/** u32 面六键（`0` 与 `4294967295` 两界）。 */
const ARCHI_U32_PATHS: ReadonlyArray<[string, string[]]> = [
  ['version', ['version']],
  ['scan.changedFiles', ['scan', 'changedFiles']],
  ['scan.scannedFiles', ['scan', 'scannedFiles']],
  ['model.elements', ['model', 'elements']],
  ['model.relations', ['model', 'relations']],
  ['errors[0].occurrences[0].line', ['errors', '0', 'occurrences', '0', 'line']],
];

/** 五处非可空字符串键（空串 / 超长 / 特殊字符均合法；`null` 必拒）。 */
const ARCHI_STRING_PATHS: ReadonlyArray<[string, string[]]> = [
  ['root', ['root']],
  ['timestamp', ['timestamp']],
  ['modelHash', ['modelHash']],
  ['errors[0].message', ['errors', '0', 'message']],
  ['scan.excluded[0].rule', ['scan', 'excluded', '0', 'rule']],
];

/** 词外枚举值（各挂一处枚举面：status / mode / driftKind / observationKind / via）。 */
const ARCHI_OUT_OF_VOCAB: ReadonlyArray<[string, string[], unknown]> = [
  ['status 词外值', ['status'], 'unknown'],
  ['status 大小写变体', ['status'], 'ArchiCheck'],
  ['status 空串', ['status'], ''],
  ['status null', ['status'], null],
  ['mode 词外值', ['mode'], 'dirty'],
  ['driftKind 词外值', ['errors', '0', 'kind'], 'undeclared_dependency'],
  ['observationKind 词外值', ['observations', '0', 'kind'], 'Unknown'],
  ['suppressVia 词外值', ['suppressed', '0', 'via'], 'INLINE'],
];

describe('ipc archi_drift：resultSchemas.archiDrift 全字段 parse', () => {
  it('14 根级字段全非空的报告 → safeParse 成功（drift / scan 图字段 getter 一次遍历全覆盖）', () => {
    expect(parsesReport(archiDriftReport())).toBe(true);
  });

  it('字段面计数在案：根 14 / model 5 / scan 7 / 条目 6 / 命中点 4 / 结构诊断 5', () => {
    const report = archiDriftReport();
    const model = report.model as Record<string, unknown>;
    const scan = report.scan as Record<string, unknown>;
    const [error] = report.errors as Array<Record<string, unknown>>;
    const [warning] = report.warnings as Array<Record<string, unknown>>;
    const [occurrence] = error.occurrences as Array<Record<string, unknown>>;
    const [diagnostic] = model.structuralErrors as Array<Record<string, unknown>>;

    expect(Object.keys(report)).toHaveLength(14);
    expect(Object.keys(model)).toHaveLength(5);
    expect(Object.keys(scan)).toHaveLength(7);
    expect(Object.keys(error)).toHaveLength(6);
    expect(Object.keys(warning)).toHaveLength(6);
    expect(Object.keys(occurrence)).toHaveLength(4);
    expect(Object.keys(diagnostic)).toHaveLength(5);
  });

  it('ipc.archiDrift 端到端（零漂移）：命令名 archi_drift、payload 键集恰 [root, mode]、原引用返回', async () => {
    const report = archiDriftReport();
    invokeMock.mockResolvedValue(report);

    const result = await ipc.archiDrift('/repo', 'incremental');

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock.mock.calls[0][0]).toBe('archi_drift');
    expect(Object.keys(payloadOf('archi_drift'))).toEqual(['root', 'mode']);
    expect(payloadOf('archi_drift')).toEqual({ root: '/repo', mode: 'incremental' });
    expect(result).toBe(report);
    expect(driftCount()).toBe(0);
  });
});

describe('ipc archi_drift：枚举词面与可空位', () => {
  it.each(['clean', 'errors', 'warnings', 'noChanges', 'skipped'])(
    'status 合法值 %s → parse 通过',
    (status) => {
      expect(parsesReport(reportWith(['status'], status))).toBe(true);
    },
  );

  it.each(['full', 'incremental'])('mode 合法值 %s → 根与 scan.mode 两处均 parse 通过', (mode) => {
    expect(parsesReport(reportWith(['mode'], mode))).toBe(true);
    expect(parsesReport(reportWith(['scan', 'mode'], mode))).toBe(true);
  });

  it.each(['undeclaredDependency', 'pathNotFound', 'ambiguousOwner', 'noPath'])(
    'errors[0].kind 合法值 %s → parse 通过',
    (kind) => {
      expect(parsesReport(reportWith(['errors', '0', 'kind'], kind))).toBe(true);
    },
  );

  it.each(['unfulfilledDeclaration', 'unusedDeclaration'])(
    'observations[0].kind 合法值 %s → parse 通过（词汇位保留，不因本期不产出而缺位）',
    (kind) => {
      expect(parsesReport(reportWith(['observations', '0', 'kind'], kind))).toBe(true);
    },
  );

  it.each(['config', 'inline'])('suppressed[0].via 合法值 %s → parse 通过', (via) => {
    expect(parsesReport(reportWith(['suppressed', '0', 'via'], via))).toBe(true);
  });

  it.each(ARCHI_OUT_OF_VOCAB)(
    '%s → parse 失败（封闭词汇零宽松归一）',
    (_label, segments, value) => {
      expect(parsesReport(reportWith(segments, value))).toBe(false);
    },
  );

  it.each(ARCHI_NULLABLE_PATHS)('可空位 %s 的有值 / null 两态均 parse 通过', (_label, segments) => {
    expect(parsesReport(archiDriftReport())).toBe(true);
    expect(parsesReport(reportWith(segments, null))).toBe(true);
  });
});

describe('ipc archi_drift：必填字段缺失与错类型必拒', () => {
  it.each(ARCHI_ROOT_KEYS)('根级必填键 %s 缺失 → parse 失败且首个 issue 路径指回该键', (key) => {
    const report = reportWithout([key]);

    expect(parsesReport(report)).toBe(false);
    expect(firstIssuePath(report)).toBe(`#/${key}`);
  });

  it.each(ARCHI_NESTED_KEYS)(
    '嵌套必填键 %s 缺失 → parse 失败且路径指回该键',
    (_label, segments) => {
      const report = reportWithout(segments);

      expect(parsesReport(report)).toBe(false);
      expect(firstIssuePath(report)).toBe(`#${segments.map((segment) => `/${segment}`).join('')}`);
    },
  );

  it.each(ARCHI_ROOT_WRONG_TYPES)('根级键 %s 给错类型 → parse 失败且路径指回该键', (key, value) => {
    const report = reportWith([key], value);

    expect(parsesReport(report)).toBe(false);
    expect(firstIssuePath(report)).toBe(`#/${key}`);
  });

  it.each(ARCHI_NESTED_WRONG_TYPES)(
    '嵌套键 %s 给错类型 → parse 失败且路径指回该键',
    (_label, segments, value) => {
      const report = reportWith(segments, value);

      expect(parsesReport(report)).toBe(false);
      expect(firstIssuePath(report)).toBe(`#${segments.map((segment) => `/${segment}`).join('')}`);
    },
  );

  it.each([
    ['根级', ['surplus']],
    ['model 内', ['model', 'surplus']],
    ['errors[0] 内', ['errors', '0', 'surplus']],
  ] as ReadonlyArray<[string, string[]]>)(
    '%s多一键 → parse 失败（strictObject，防生成物退化成 loose）',
    (_label, segments) => {
      expect(parsesReport(reportWith(segments, 1))).toBe(false);
    },
  );

  it('经 ipc.archiDrift 时同形多余键 → 只记 1 条漂移、返回值原引用、Promise 正常 resolve', async () => {
    const dirty = reportWith(['surplus'], 1);
    invokeMock.mockResolvedValue(dirty);

    const result = await ipc.archiDrift('/repo', 'full');

    expect(driftCount()).toBe(1);
    expect(result).toBe(dirty);
  });
});

describe('ipc archi_drift：数值 / 字符串 / 数组边界', () => {
  it.each(ARCHI_U32_PATHS)(
    'u32 面 %s 的两界（0 / 4294967295）均 parse 通过',
    (_label, segments) => {
      expect(parsesReport(reportWith(segments, 0))).toBe(true);
      expect(parsesReport(reportWith(segments, 4294967295))).toBe(true);
    },
  );

  it.each([
    ['-1', -1],
    ['1.5', 1.5],
    ['4294967296', 4294967296],
    ['NaN', Number.NaN],
  ] as ReadonlyArray<[string, number]>)(
    'version 取越界 / 非整数 %s → parse 失败且路径指回该键',
    (_label, value) => {
      const report = reportWith(['version'], value);

      expect(parsesReport(report)).toBe(false);
      expect(firstIssuePath(report)).toBe('#/version');
    },
  );

  it.each(ARCHI_STRING_PATHS)(
    '字符串键 %s 的空串 / 超长 / 特殊字符均 parse 通过（schema 无 minLength）',
    (_label, segments) => {
      for (const value of ['', 'x'.repeat(1200), '行\n列\0🎯']) {
        expect(parsesReport(reportWith(segments, value))).toBe(true);
      }
    },
  );

  it.each(ARCHI_STRING_PATHS)('非可空字符串键 %s 给 null → parse 失败', (_label, segments) => {
    expect(parsesReport(reportWith(segments, null))).toBe(false);
  });

  it.each(ARCHI_ARRAY_PATHS)('数组字段 %s 的 [] 与单元素两态均 parse 通过', (_label, segments) => {
    expect(parsesReport(reportWith(segments, []))).toBe(true);
    // 单元素形态即基线 fixture 本体的形态（九处各持一项）
    expect(parsesReport(archiDriftReport())).toBe(true);
  });

  it('errors 取 1000 条 → 单次 parse 通过（规模面零截断）', () => {
    const many = Array.from({ length: 1000 }, () => driftEntry());

    expect(parsesReport(reportWith(['errors'], many))).toBe(true);
  });
});

describe('ipc archi_drift：结果面脏值只上报、参数面本地拒', () => {
  // 脏值取根级 `modelHash`（`model` 段无 `modelHash` 字段——多出来的键走 strictObject 的
  // 「多余字段被拒」面，见上节），保证判据是「类型错 → 单条漂移 + 路径精确」。
  it.each([
    ['modelHash', ['modelHash'], 42, 'string'],
    ['scan.scannedFiles', ['scan', 'scannedFiles'], '12', 'number'],
  ] as ReadonlyArray<[string, string[], unknown, string]>)(
    '脏值 %s → driftCount 1、path 精确、原引用返回（soft-report 不 throw）',
    async (_label, segments, value, expected) => {
      const dirty = reportWith(segments, value);
      invokeMock.mockResolvedValue(dirty);

      const result = await ipc.archiDrift('/repo', 'incremental');

      expect(result).toBe(dirty);
      expect(driftCount()).toBe(1);
      const [entry] = driftReports();
      expect(entry.cmd).toBe('archiDrift');
      expect(entry.path).toBe(`#${segments.map((segment) => `/${segment}`).join('')}`);
      expect(entry.expected).toBe(expected);
      expect(entry.got).toBe(value);
    },
  );

  it('status 给词外值 → driftCount 1、path #/status、值原样透传（非类型错走 issue 文案）', async () => {
    const dirty = reportWith(['status'], 'dirty');
    invokeMock.mockResolvedValue(dirty);

    const result = await ipc.archiDrift('/repo', 'incremental');

    expect(result).toBe(dirty);
    expect(driftCount()).toBe(1);
    const [entry] = driftReports();
    expect(entry.path).toBe('#/status');
    expect(entry.expected).not.toBe('');
    expect(entry.got).toBe('dirty');
  });

  it('root 传数字 → IpcArgsError（cmd archiDrift / path #/root / expected string）、invoke 零调用', async () => {
    const error = await rejectionOf(ipc.archiDrift(42 as unknown as string, 'incremental'));

    const view = expectArgsError(error);
    expect(view.cmd).toBe('archiDrift');
    expect(view.path).toBe('#/root');
    expect(view.expected).toBe('string');
    expect(view.got).toBe(42);
    expect(invokeMock).not.toHaveBeenCalled();
    expect(driftCount()).toBe(0);
  });

  it.each([
    ['词外值', 'dirty'],
    ['null', null],
  ] as ReadonlyArray<[string, unknown]>)(
    'mode 传 %s → 本地拒（路径 #/mode）、零 IPC 往返',
    async (_label, mode) => {
      const error = await rejectionOf(ipc.archiDrift('/repo', mode as ArchiDriftMode));

      const view = expectArgsError(error);
      expect(view.cmd).toBe('archiDrift');
      expect(view.path).toBe('#/mode');
      expect(view.got).toBe(mode);
      expect(invokeMock).not.toHaveBeenCalled();
    },
  );
});

describe('ipc archi_drift：三表表项形态与无状态残留', () => {
  it('channelArgs.archiDrift 为空对象表项 → 不包裹任何对象、零信封漂移', async () => {
    const table = Reflect.get(channelArgs, 'archiDrift') as Record<string, unknown>;

    expect(table).toEqual({});
    expect(Object.keys(table)).toHaveLength(0);
    invokeMock.mockResolvedValue(archiDriftReport());

    await ipc.archiDrift('/repo', 'full');

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(driftCount()).toBe(0);
  });

  it('三表 archiDrift 表项皆为 getter（descriptor.get 为函数）', () => {
    for (const table of [argsSchemas, resultSchemas, channelArgs]) {
      const descriptor = Object.getOwnPropertyDescriptor(table, 'archiDrift');
      expect(typeof descriptor?.get).toBe('function');
    }
  });

  it('resultSchemas.archiDrift 两次读取得到不同实例、同一 payload 判定一致（无状态残留）', () => {
    const first = archiDriftSchema();
    const second = archiDriftSchema();

    expect(first).not.toBe(second);
    for (const payload of [archiDriftReport(), reportWith(['status'], 'unknown')]) {
      expect(first.safeParse(payload).success).toBe(second.safeParse(payload).success);
    }
  });

  it('argsSchemas.archiDrift.shape 键序恰 [root, mode]（与 bindings.ts 形参序对位）', () => {
    const schema = Reflect.get(argsSchemas, 'archiDrift') as ZodObject;

    expect(Object.keys(schema.shape)).toEqual(['root', 'mode']);
  });
});
