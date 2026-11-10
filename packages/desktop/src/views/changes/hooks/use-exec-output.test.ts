import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { driftCount, driftReports, resetDrift } from '../../../lib/ipc-drift';
import type { ChangeStepStatus } from '../../../types/dto';
import { useExecOutput } from './use-exec-output';

// ---------------------------------------------------------------------------
// useExecOutput 单测（change-detail-live-stdout / AC-6 / AC-7 / AC-9）：
// 数据源三段——命令头行一次读（`read_exec_command`，失败不阻断内容面）+ 内容面
// offset 增量（`read_exec_output_delta`，`fromOffset` 只取自后端 `nextOffset`）+
// `status === 'running'` 才挂 `useFileWatchSignal`（`armKey` 非运行态为 null ⇒
// 零订阅）。
//
// 进程边界 Mock（Mock策略表）：`invokeMock` 按命令名分发（应答三形态：值 = 命中 /
// `null` = miss / `Error` = reject）+ `ChannelMock`（`deliver` + `instances` 静态
// 表）；`src/lib/ipc.ts` / `ipc-drift.ts` / `use-file-watch-signal.ts` 与生成物
// **真实组合**（取数纪律与 armKey / 退订 / 防抖正是被测契约）。
// ---------------------------------------------------------------------------

const { ChannelMock, invokeMock } = vi.hoisted(() => {
  class ChannelMock {
    public onmessage: ((message: unknown) => void) | null = null;
    public static instances: ChannelMock[] = [];
    constructor() {
      ChannelMock.instances.push(this);
    }
    /** 信号注入口（共享件把 `onmessage` 接到防抖 push）。 */
    public deliver(): void {
      this.onmessage?.({});
    }
  }
  return { ChannelMock, invokeMock: vi.fn() };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock, Channel: ChannelMock }));

const ROOT = 'C:\\demo\\flow';
const CHANGE_NAME = 'add-feature';
const EXEC_ID = 'exec-1727000000000-3';
const DERIVED_PATH =
  'C:\\demo\\flow\\openspec\\changes\\add-feature\\reports\\test\\exec-outputs\\exec-1727000000000-3.log';
const DEBOUNCE_MS = 500;

interface Params {
  root: string | null;
  changeName: string | null;
  execId: string | null;
  status: ChangeStepStatus | null;
}

function propsWith(overrides: Partial<Params> = {}): Params {
  return { root: ROOT, changeName: CHANGE_NAME, execId: EXEC_ID, status: 'running', ...overrides };
}

/** 增量读应答（`{ text, nextOffset }` = 命中、`null` = miss、`Error` = reject）。 */
type DeltaAnswer = { text: string; nextOffset: number } | null | Error | RawReject;

/** 命令 sidecar 应答（`string` = 命中、`null` = 老记录、`Error` = reject）。 */
type CommandAnswer = string | null | Error | RawReject;

/**
 * 非字符串拒绝原因（`toMessage` 的 `String(cause)` 分支）：`ipc` 层把 `invoke` 的
 * 拒绝值原样上抛，故 cause 可能是 Error 对象 / 数字 / 普通对象而非串。
 */
class RawReject {
  constructor(public readonly value: unknown) {}
}

let deltaAnswers: DeltaAnswer[] = [];
/** 队列耗尽后重复的最后一条（模拟「无新增」）。 */
let deltaTail: DeltaAnswer = { text: '', nextOffset: 0 };
let commandAnswer: CommandAnswer = 'pnpm run check 2>&1';
let pathAnswer: string | null = DERIVED_PATH;
let subscribeAnswer: number | Error = 7;
let unsubscribeAnswer: boolean | Error = true;
/** 应答闸门（竞态用例：把指定次数的应答悬停在「在飞」状态，放行后交付）。 */
interface Gate {
  gate: Promise<void>;
  /** 仍要拦截的调用次数（耗尽后立即应答）。 */
  remaining: number;
}

let deltaGate: Gate | null = null;
let commandGate: Gate | null = null;

/** 造一个先挂起、由返回的 `release` 放行的闸门。 */
function makeGate(times: number = 1): Gate & { release: () => void } {
  let release: () => void = () => {};
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  return { gate, remaining: times, release };
}

function callsOf(name: string): unknown[][] {
  return invokeMock.mock.calls.filter(([command]) => command === name);
}

function deltaCalls(): unknown[][] {
  return callsOf('read_exec_output_delta');
}

/** 全部已发生命令名（取数纪律断言面）。 */
function commandNames(): string[] {
  return invokeMock.mock.calls.map(([name]) => String(name));
}

function answerOf<T>(answer: T | Error | RawReject): Promise<T> {
  if (answer instanceof RawReject) return Promise.reject(answer.value);
  return answer instanceof Error ? Promise.reject(answer.message) : Promise.resolve(answer);
}

/** 闸门在飞时挂起，放行后交出同一应答（竞态用例专用）。 */
function gateAnswer<T>(state: Gate | null, answer: T | Error | RawReject): Promise<T> {
  if (state === null || state.remaining <= 0) return answerOf(answer);
  state.remaining -= 1;
  return state.gate.then(() => answerOf(answer));
}

beforeEach(() => {
  deltaAnswers = [];
  deltaTail = { text: '', nextOffset: 0 };
  commandAnswer = 'pnpm run check 2>&1';
  pathAnswer = DERIVED_PATH;
  subscribeAnswer = 7;
  unsubscribeAnswer = true;
  deltaGate = null;
  commandGate = null;
  ChannelMock.instances = [];
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
    switch (command) {
      case 'read_exec_command':
        return gateAnswer<CommandAnswer>(commandGate, commandAnswer);
      case 'read_exec_output_delta': {
        const next = deltaAnswers.length > 0 ? deltaAnswers.shift() : deltaTail;
        return gateAnswer<DeltaAnswer>(deltaGate, next ?? null);
      }
      case 'exec_output_path':
        return Promise.resolve(pathAnswer);
      case 'watch_subscribe':
        return answerOf<number>(subscribeAnswer);
      case 'watch_unsubscribe':
        return answerOf<boolean>(unsubscribeAnswer);
      default:
        return Promise.resolve(null);
    }
  });
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  invokeMock.mockReset();
});

/** 冲刷微任务（不推进真实计时器）。 */
async function flush(): Promise<void> {
  await act(async () => {});
}

/** 挂载（fake timers 下：防抖窗口需显式推进）。 */
async function mounted(overrides: Partial<Params> = {}) {
  vi.useFakeTimers();
  const rendered = renderHook((input: Params) => useExecOutput(input), {
    initialProps: propsWith(overrides),
  });
  await flush();
  return rendered;
}

/** 推进防抖窗口并冲刷微任务（信号到达 → 500 ms → 增量取数）。 */
async function advanceSignalWindow(ms: number = DEBOUNCE_MS): Promise<void> {
  await act(async () => {
    vi.advanceTimersByTime(ms);
  });
  await flush();
}

describe('useExecOutput：装载首拍', () => {
  it('正向：running 挂载 → read_exec_output_delta 恰 1 次且 fromOffset 为 0，text 为返回文本', async () => {
    deltaAnswers = [{ text: 'line-1\n', nextOffset: 7 }];
    const { result } = await mounted();

    expect(deltaCalls()).toEqual([
      [
        'read_exec_output_delta',
        { root: ROOT, changeName: CHANGE_NAME, execId: EXEC_ID, fromOffset: 0 },
      ],
    ]);
    expect(result.current.text).toBe('line-1\n');
    expect(result.current.error).toBeNull();
  });

  it('正向：终态打开（passed）→ 同样 fromOffset 0 全量读 1 次，且零 watch_subscribe（armKey 为 null）', async () => {
    deltaAnswers = [{ text: 'full output\n', nextOffset: 12 }];
    const { result } = await mounted({ status: 'passed' });

    expect(deltaCalls()).toHaveLength(1);
    expect(deltaCalls()[0][1]).toMatchObject({ fromOffset: 0 });
    expect(result.current.text).toBe('full output\n');
    expect(callsOf('watch_subscribe')).toHaveLength(0);
    expect(callsOf('exec_output_path')).toHaveLength(0);
    expect(ChannelMock.instances).toHaveLength(0);
  });

  it('正向：命令头行读一次（入参逐字），多行与 emoji 原样进 command', async () => {
    const multiline = '\n=== plan app_node-test · node-test ===\npnpm run check 2>&1\n🚀 done\n';
    commandAnswer = multiline;
    const { result } = await mounted();

    expect(callsOf('read_exec_command')).toEqual([
      ['read_exec_command', { root: ROOT, changeName: CHANGE_NAME, execId: EXEC_ID }],
    ]);
    expect(result.current.command).toBe(multiline);
  });

  it('异常：read_exec_command reject → command 为 null、text 照常、error 保持 null（命令失败不阻断内容面）', async () => {
    commandAnswer = new Error('读命令失败');
    deltaAnswers = [{ text: 'content\n', nextOffset: 8 }];
    const { result } = await mounted();

    expect(result.current.command).toBeNull();
    expect(result.current.text).toBe('content\n');
    expect(result.current.error).toBeNull();
  });

  it('异常：read_exec_output_delta reject → error 为拒绝串、text 保持 null（不炸图）', async () => {
    deltaAnswers = [new Error('读输出记录失败: 磁盘不可读')];
    const { result } = await mounted();

    expect(result.current.error).toBe('读输出记录失败: 磁盘不可读');
    expect(result.current.text).toBeNull();
    expect(result.current.command).toBe('pnpm run check 2>&1');
  });

  it.each([
    ['root', { root: null }],
    ['changeName', { changeName: null }],
    ['execId', { execId: null }],
  ])('边界：%s 为 null → 三命令零 invoke、三字段全 null', async (_label, absent) => {
    const { result } = await mounted(absent);

    expect(invokeMock).not.toHaveBeenCalled();
    expect(result.current.command).toBeNull();
    expect(result.current.text).toBeNull();
    expect(result.current.error).toBeNull();
  });

  it('边界：返回 null（miss）→ text 保持 null（不写 ""）；返回空增量 → text 保持 null 且游标为 0', async () => {
    deltaAnswers = [null];
    const missRender = await mounted();
    expect(missRender.result.current.text).toBeNull();
    expect(missRender.result.current.error).toBeNull();
    missRender.unmount();

    deltaAnswers = [{ text: '', nextOffset: 0 }];
    invokeMock.mockClear();
    const emptyRender = await mounted();
    expect(emptyRender.result.current.text).toBeNull();
    expect(deltaCalls()[0][1]).toMatchObject({ fromOffset: 0 });
  });
});

describe('useExecOutput：运行中跟随', () => {
  it('正向：信号到达 → 500 ms 防抖后第二次增量，fromOffset 为上次 nextOffset、text 追加', async () => {
    deltaAnswers = [
      { text: 'first\n', nextOffset: 6 },
      { text: 'second\n', nextOffset: 13 },
    ];
    const { result } = await mounted();
    expect(result.current.text).toBe('first\n');

    ChannelMock.instances[0].deliver();
    await advanceSignalWindow();

    const calls = deltaCalls();
    expect(calls).toHaveLength(2);
    expect(calls[1][1]).toMatchObject({ fromOffset: 6 });
    expect(result.current.text).toBe('first\nsecond\n');
  });

  it('正向：三拍序列（nextOffset 10 → 25 → 40）→ 拼接等于全量、fromOffset 序列 0 / 10 / 25', async () => {
    deltaAnswers = [
      { text: 'a'.repeat(10), nextOffset: 10 },
      { text: 'b'.repeat(15), nextOffset: 25 },
      { text: 'c'.repeat(15), nextOffset: 40 },
    ];
    const { result } = await mounted();
    ChannelMock.instances[0].deliver();
    await advanceSignalWindow();
    ChannelMock.instances[0].deliver();
    await advanceSignalWindow();

    expect(deltaCalls().map((call) => (call[1] as { fromOffset: number }).fromOffset)).toEqual([
      0, 10, 25,
    ]);
    expect(result.current.text).toBe(`${'a'.repeat(10)}${'b'.repeat(15)}${'c'.repeat(15)}`);
  });

  it('正向：armKey —— running 时 watch_subscribe 恰 1 次，首参为 ChannelMock 实例、次参为派生路径', async () => {
    const { result } = await mounted();
    expect(result.current.text).not.toBeUndefined();

    const subscribe = callsOf('watch_subscribe');
    expect(subscribe).toHaveLength(1);
    // 生成包装签名为位置参数，`ipc` 校验层按 schema shape 装配成 payload 后下发
    const payload = subscribe[0][1] as { onEvent: unknown; path: string };
    expect(payload.onEvent).toBe(ChannelMock.instances[0]);
    expect(payload.path).toBe(DERIVED_PATH);
    expect(callsOf('exec_output_path')).toHaveLength(1);
  });

  it('异常：exec_output_path 返回 null → 零 watch_subscribe，内容面照常', async () => {
    pathAnswer = null;
    deltaAnswers = [{ text: 'content\n', nextOffset: 8 }];
    const { result } = await mounted();

    expect(callsOf('watch_subscribe')).toHaveLength(0);
    expect(result.current.text).toBe('content\n');
    expect(result.current.error).toBeNull();
  });

  it('异常：watch_subscribe reject → 静默（error 仍 null、内容面可用、零重试）', async () => {
    subscribeAnswer = new Error('订阅失败');
    deltaAnswers = [{ text: 'content\n', nextOffset: 8 }];
    const { result } = await mounted();

    expect(result.current.error).toBeNull();
    expect(result.current.text).toBe('content\n');
    expect(callsOf('watch_subscribe')).toHaveLength(1);
    expect(callsOf('watch_unsubscribe')).toHaveLength(0);
  });

  it.each([
    ['passed', 'passed'],
    ['failed', 'failed'],
    ['stopped', 'stopped'],
    ['status null', null],
  ] as const)(
    '边界：status = %s → armKey 为 null：零 Channel、零订阅、零退订',
    async (_label, status) => {
      await mounted({ status: status as Params['status'] });

      expect(ChannelMock.instances).toHaveLength(0);
      expect(callsOf('watch_subscribe')).toHaveLength(0);
      expect(callsOf('watch_unsubscribe')).toHaveLength(0);
      expect(callsOf('exec_output_path')).toHaveLength(0);
    },
  );

  it('边界：同 status 重渲染（回调每次新身份）→ 零重订，信号仍触发最新取数', async () => {
    deltaAnswers = [
      { text: 'first\n', nextOffset: 6 },
      { text: 'second\n', nextOffset: 13 },
    ];
    const { result, rerender } = await mounted();
    rerender(propsWith({ status: 'running' }));
    await flush();
    rerender(propsWith({ status: 'running' }));
    await flush();

    expect(callsOf('watch_subscribe')).toHaveLength(1);
    expect(callsOf('watch_unsubscribe')).toHaveLength(0);

    ChannelMock.instances[0].deliver();
    await advanceSignalWindow();
    expect(deltaCalls()).toHaveLength(2);
    expect(result.current.text).toBe('first\nsecond\n');
  });
});

describe('useExecOutput：终态收敛', () => {
  it('正向：running → passed ⇒ watch_unsubscribe 恰 1 次、增量再 1 次且 fromOffset 为 0、文本被替换', async () => {
    deltaAnswers = [
      { text: 'live-a\n', nextOffset: 7 },
      { text: 'authoritative-b\n', nextOffset: 16 },
    ];
    const { result, rerender } = await mounted();
    expect(result.current.text).toBe('live-a\n');

    rerender(propsWith({ status: 'passed' }));
    await flush();

    expect(callsOf('watch_unsubscribe')).toHaveLength(1);
    expect(callsOf('watch_unsubscribe')[0][1]).toEqual({ subscriptionId: 7 });
    const calls = deltaCalls();
    expect(calls).toHaveLength(2);
    expect(calls[1][1]).toMatchObject({ fromOffset: 0 });
    expect(result.current.text).toBe('authoritative-b\n');
  });

  it('正向：权威内容重排 → text 逐字等于整读结果，不残留 live 交错序', async () => {
    deltaAnswers = [
      { text: 'a\nb\n', nextOffset: 4 },
      { text: 'b\na\n', nextOffset: 4 },
    ];
    const { result, rerender } = await mounted();
    expect(result.current.text).toBe('a\nb\n');

    rerender(propsWith({ status: 'passed' }));
    await flush();
    expect(result.current.text).toBe('b\na\n');
  });

  it('异常：转终态的整读 reject → error 为拒绝串、text 为 null（跨臂不继承上一臂累积值）、退订仍 1 次', async () => {
    deltaAnswers = [{ text: 'live-a\n', nextOffset: 7 }, new Error('整读失败')];
    const { result, rerender } = await mounted();
    expect(result.current.text).toBe('live-a\n');

    rerender(propsWith({ status: 'failed' }));
    await flush();

    expect(result.current.error).toBe('整读失败');
    // D10：内容面 effect 每臂起点先 `offsetRef = 0` + `setText(null)` + `setError(null)`
    // 再发起装载首拍 ⇒ 终态臂整读失败时 `text` 恒为 null（MUST NOT 写成「保留上一臂 /
    // 运行中累积文本」：跨臂不继承 live 交错序，面板呈错误态即可）
    expect(result.current.text).toBeNull();
    expect(callsOf('watch_unsubscribe')).toHaveLength(1);
  });

  it('边界：信号在飞（防抖窗口内）时转终态 → 旧臂计时被取消，增量总计 2 次', async () => {
    deltaAnswers = [
      { text: 'first\n', nextOffset: 6 },
      { text: 'second\n', nextOffset: 13 },
    ];
    const { rerender } = await mounted();
    ChannelMock.instances[0].deliver(); // 防抖计时在飞
    rerender(propsWith({ status: 'passed' })); // 未推进计时即切终态
    await flush();
    await advanceSignalWindow(1000);

    expect(deltaCalls()).toHaveLength(2);
  });

  it('边界：卸载 → 以 subscription_id 退订 1 次、推进 1000 ms 零增量、迟到响应不回写', async () => {
    deltaAnswers = [{ text: 'first\n', nextOffset: 6 }];
    const { result, unmount } = await mounted();
    expect(result.current.text).toBe('first\n');

    unmount();
    await flush();
    expect(callsOf('watch_unsubscribe')).toHaveLength(1);
    expect(callsOf('watch_unsubscribe')[0][1]).toEqual({ subscriptionId: 7 });

    const before = deltaCalls().length;
    await advanceSignalWindow(1000);
    expect(deltaCalls()).toHaveLength(before);
  });
});

describe('useExecOutput：取数纪律（只经 ipc）', () => {
  beforeEach(() => {
    resetDrift();
    vi.spyOn(console, 'warn').mockImplementation(() => {});
  });

  it('异常：合法入参零漂移；结果脏（nextOffset 缺键）→ 漂移恰 1 条且原值透传不 throw', async () => {
    deltaAnswers = [{ text: 'ok\n', nextOffset: 3 }];
    const { result } = await mounted();
    expect(result.current.text).toBe('ok\n');
    expect(result.current.error).toBeNull();
    expect(driftCount()).toBe(0);
  });

  it('异常：返回脏结果（nextOffset 缺键）→ 漂移恰 1 条、text 按原值透传不 throw', async () => {
    deltaAnswers = [{ text: 'dirty\n' } as unknown as DeltaAnswer];
    const { result } = await mounted();

    expect(result.current.error).toBeNull();
    expect(driftCount()).toBe(1);
    expect(driftReports()[0].cmd).toBe('readExecOutputDelta');
  });

  it('边界：命令名集合 ⊆ 五命令（零裸 commands.* 旁路）', async () => {
    deltaAnswers = [{ text: 'a\n', nextOffset: 2 }];
    const { rerender } = await mounted();
    ChannelMock.instances[0]?.deliver();
    await advanceSignalWindow();
    rerender(propsWith({ status: 'passed' }));
    await flush();

    const allowed = new Set([
      'read_exec_command',
      'read_exec_output_delta',
      'exec_output_path',
      'watch_subscribe',
      'watch_unsubscribe',
    ]);
    for (const name of commandNames()) {
      expect(allowed.has(name)).toBe(true);
    }
  });
});

// ---------------------------------------------------------------------------
// 变异补强组（test-execution 回溯）：按 test-design 已载口径把以下存活面钉死——
// ① `toMessage` 的非串 cause（`String(cause)` 口径，L18）；
// ② 命令头行 / 内容面寻址守卫的组合形态（L33、L139：单 `null` 与双 `null`）；
// ③ `disposed` 与退订的竞态时序（L41-49 / L92 / L98-99 / L142）；
// ④ `armKey` 形态（L137：分隔符与拼接形态，非仅「有没有订阅」）。
// 断言全部落在**外部可观察面**（invoke 入参 / 命令次数 / 渲染结果），不触碰实现细节。
// ---------------------------------------------------------------------------

describe('useExecOutput：错误文案口径（toMessage 的 String(cause) 分支）', () => {
  it('异常：内容面 reject 的原因为 Error 对象 → error 为其 String() 结果（非 "[object Object]" 兜底）', async () => {
    // 非串 cause 才走 String(cause)：串 cause 走身份分支（既有用例已覆盖）
    deltaAnswers = [new RawReject(new Error('读输出记录失败: 磁盘不可读'))];
    const { result } = await mounted();

    expect(result.current.error).toBe('Error: 读输出记录失败: 磁盘不可读');
    expect(result.current.text).toBeNull();
  });

  it('异常：内容面 reject 的原因为数字 → error 为 "42"（String 口径对非串标量成立）', async () => {
    deltaAnswers = [new RawReject(42)];
    const { result } = await mounted();

    expect(result.current.error).toBe('42');
  });

  it('边界：内容面 reject 的原因为普通对象 / null / undefined → error 逐字为 String(cause)，实现不抛', async () => {
    const cases: Array<[string, unknown]> = [
      ['普通对象', { code: 13 }],
      ['null', null],
      ['undefined', undefined],
    ];
    for (const [label, cause] of cases) {
      deltaAnswers = [new RawReject(cause)];
      const { result, unmount } = await mounted();
      expect(result.current.error, label).toBe(String(cause));
      expect(result.current.text, label).toBeNull();
      unmount();
      invokeMock.mockClear();
      vi.clearAllTimers();
    }
  });

  it('异常：串 cause 走身份分支（不被 String() 二次包装）——error 逐字等于原串', async () => {
    deltaAnswers = [new Error('原样文案')];
    const { result } = await mounted();

    expect(result.current.error).toBe('原样文案');
  });
});

describe('useExecOutput：寻址守卫组合（单 null 与双 null）', () => {
  it.each([
    ['root 与 changeName 同为 null', { root: null, changeName: null }],
    ['root 与 execId 同为 null', { root: null, execId: null }],
    ['changeName 与 execId 同为 null', { changeName: null, execId: null }],
    ['三者同为 null', { root: null, changeName: null, execId: null }],
  ])(
    '边界：%s → 三命令零 invoke、三字段全 null（或判据不是「全 null 才短路」）',
    async (_label, absent) => {
      const { result } = await mounted(absent);

      expect(invokeMock).not.toHaveBeenCalled();
      expect(result.current.command).toBeNull();
      expect(result.current.text).toBeNull();
      expect(result.current.error).toBeNull();
      expect(ChannelMock.instances).toHaveLength(0);
    },
  );

  it('边界：changeName 为 null 而 root / execId 在场（status running）→ 臂键成立但 resolvePath 判空 ⇒ 零 watch_subscribe', async () => {
    deltaAnswers = [{ text: 'x\n', nextOffset: 2 }];
    const { result } = await mounted({ changeName: null });

    // `armKey` 只吃 root / execId（changeName 不参与订阅身份）⇒ Channel 已建
    expect(ChannelMock.instances).toHaveLength(1);
    // 但路径派生需 changeName ⇒ resolvePath 判空 ⇒ 零订阅、零 exec_output_path
    expect(callsOf('exec_output_path')).toHaveLength(0);
    expect(callsOf('watch_subscribe')).toHaveLength(0);
    // 内容面 / 命令面同样短路 ⇒ 零取数、零累积
    expect(deltaCalls()).toHaveLength(0);
    expect(callsOf('read_exec_command')).toHaveLength(0);
    expect(result.current.text).toBeNull();
    expect(result.current.command).toBeNull();
  });

  it('边界：resolvePath 的 null 守卫逐一成立——仅 changeName 缺席时零 watch_subscribe（路径不可派生）', async () => {
    pathAnswer = null;
    deltaAnswers = [{ text: 'kept\n', nextOffset: 5 }];
    const { result } = await mounted();

    // 路径为 null ⇒ 共享件不订阅（内容面照常）
    expect(callsOf('watch_subscribe')).toHaveLength(0);
    expect(callsOf('watch_unsubscribe')).toHaveLength(0);
    expect(result.current.text).toBe('kept\n');
  });

  it('边界：status 非 running 而 root / execId 皆在场 → armKey 仍为 null（status 参与合取）', async () => {
    const { rerender } = await mounted({ status: 'passed' });
    expect(callsOf('watch_subscribe')).toHaveLength(0);

    // 反向：同一次挂载转 running ⇒ 订阅恰 1 次（合取项可达）
    rerender(propsWith({ status: 'running' }));
    await flush();
    expect(callsOf('watch_subscribe')).toHaveLength(1);
    expect(callsOf('exec_output_path')).toHaveLength(1);
  });
});

describe('useExecOutput：armKey 形态（订阅身份键）', () => {
  it('边界：同一 root / execId 下 changeName 变化 → armKey 不变（订阅身份不含 changeName），零重订', async () => {
    const { rerender } = await mounted({ changeName: 'add-feature' });
    expect(callsOf('watch_subscribe')).toHaveLength(1);

    rerender(propsWith({ changeName: 'other-name' }));
    await flush();

    expect(callsOf('watch_subscribe')).toHaveLength(1);
    expect(callsOf('watch_unsubscribe')).toHaveLength(0);
  });

  it('边界：execId 变化 → armKey 变化，退订旧的、订阅新的（身份键含 execId）', async () => {
    const { rerender } = await mounted();
    expect(callsOf('watch_subscribe')).toHaveLength(1);

    rerender(propsWith({ execId: 'exec-1727000000001-4' }));
    await flush();

    expect(callsOf('watch_unsubscribe')).toHaveLength(1);
    expect(callsOf('watch_unsubscribe')[0][1]).toEqual({ subscriptionId: 7 });
    expect(callsOf('watch_subscribe')).toHaveLength(2);
  });

  it('边界：root 变化 → armKey 变化，退订旧订阅并重订（身份键含 root）', async () => {
    const { rerender } = await mounted();
    rerender(propsWith({ root: 'D:another\root' }));
    await flush();

    expect(callsOf('watch_unsubscribe')).toHaveLength(1);
    expect(callsOf('watch_subscribe')).toHaveLength(2);
  });

  it('边界：armKey 为 root 与 execId 的拼接（分隔符防碰撞）——换分隔符不改变订阅/退订语义', async () => {
    // 见证面：不同 root/execId 组合各自触发一次重订（身份键两分量都参与）
    const { rerender } = await mounted({ root: 'A', execId: 'B' });
    expect(callsOf('watch_subscribe')).toHaveLength(1);
    rerender(propsWith({ root: 'A', execId: 'C' }));
    await flush();
    rerender(propsWith({ root: 'A|B', execId: 'C' }));
    await flush();

    // 三次不同身份键 ⇒ 三次订阅、两次退订；同键重渲染不再重订
    expect(callsOf('watch_subscribe')).toHaveLength(3);
    expect(callsOf('watch_unsubscribe')).toHaveLength(2);
  });
});

describe('useExecOutput：生命周期竞态（disposed 与退订时序）', () => {
  it('边界：卸载后迟到的命令应答不回写 command（disposed 守卫在 then 分支）', async () => {
    const gate = makeGate(1);
    commandGate = gate;
    commandAnswer = '晚到的命令';
    deltaAnswers = [{ text: 'body\n', nextOffset: 5 }];

    vi.useFakeTimers();
    const { result, unmount } = renderHook((input: Params) => useExecOutput(input), {
      initialProps: propsWith(),
    });
    // 装载首拍已发出，但命令应答被闸门挂起 ⇒ command 仍为 null
    await flush();
    expect(result.current.command).toBeNull();
    expect(result.current.text).toBe('body\n');

    unmount();
    gate.release();
    await flush();

    // 迟到的应答不得写回（组件已卸载；再次挂载必从 null 起，故以「未抛 + 无写回」为判据）
    expect(result.current.command).toBeNull();
  });

  it('边界：卸载后迟到的命令 reject 不回写 command（catch 分支的 disposed 守卫）', async () => {
    const gate = makeGate(1);
    commandGate = gate;
    commandAnswer = new Error('晚到的命令失败');
    deltaAnswers = [{ text: 'body\n', nextOffset: 5 }];

    vi.useFakeTimers();
    const { result, unmount } = renderHook((input: Params) => useExecOutput(input), {
      initialProps: propsWith(),
    });
    await flush();
    expect(result.current.command).toBeNull();

    unmount();
    gate.release();
    await flush();

    expect(result.current.command).toBeNull();
    expect(result.current.error).toBeNull();
  });

  it('边界：卸载后迟到的增量 reject 不回写 error（内容面 catch 的 disposed 守卫）', async () => {
    const gate = makeGate(1);
    deltaGate = gate;
    deltaAnswers = [new Error('晚到的读失败')];

    vi.useFakeTimers();
    const { result, unmount } = renderHook((input: Params) => useExecOutput(input), {
      initialProps: propsWith(),
    });
    await flush();
    expect(result.current.error).toBeNull();

    unmount();
    gate.release();
    await flush();

    expect(result.current.error).toBeNull();
    expect(result.current.text).toBeNull();
  });

  it('边界：卸载即清 readRef —— 旧臂迟到的信号不触发增量（防卸后取数）', async () => {
    deltaAnswers = [{ text: 'first\n', nextOffset: 6 }];
    const { result, unmount } = await mounted();
    expect(result.current.text).toBe('first\n');
    const channel = ChannelMock.instances[0];
    const before = deltaCalls().length;

    unmount();
    await flush();
    // 退订已发（共享件按订阅 id 退订），迟到的信号不产生新取数
    expect(callsOf('watch_unsubscribe')).toHaveLength(1);
    channel.deliver();
    await advanceSignalWindow(1000);

    expect(deltaCalls()).toHaveLength(before);
    expect(result.current.text).toBe('first\n');
  });

  it('边界：未挂订阅（status 非 running）时推进任意时长仍是单次装载首拍', async () => {
    deltaAnswers = [{ text: 'once\n', nextOffset: 5 }];
    const { result } = await mounted({ status: 'passed' });

    expect(ChannelMock.instances).toHaveLength(0);
    expect(result.current.text).toBe('once\n');
    await advanceSignalWindow(2000);
    expect(deltaCalls()).toHaveLength(1);
  });

  it('边界：status 在 running → passed → running 之间往复 ⇒ 每次转 running 重订一次、每次离开退订一次', async () => {
    deltaAnswers = [
      { text: 'a\n', nextOffset: 2 },
      { text: 'b\n', nextOffset: 2 },
      { text: 'c\n', nextOffset: 2 },
    ];
    const { result, rerender } = await mounted();
    expect(callsOf('watch_subscribe')).toHaveLength(1);

    rerender(propsWith({ status: 'passed' }));
    await flush();
    expect(callsOf('watch_unsubscribe')).toHaveLength(1);

    rerender(propsWith({ status: 'running' }));
    await flush();
    expect(callsOf('watch_subscribe')).toHaveLength(2);
    // 每臂起点整读：fromOffset 恒回 0（跨臂不继承游标）
    expect(deltaCalls().map((call) => (call[1] as { fromOffset: number }).fromOffset)).toEqual([
      0, 0, 0,
    ]);
    expect(result.current.text).toBe('c\n');
  });
});

describe('useExecOutput：换臂后迟到应答（命令面 / 内容面的 disposed 守卫）', () => {
  it('边界：命令面换臂后旧臂迟到的成功应答不回写（新臂命令保位）', async () => {
    const gate = makeGate(1);
    commandGate = gate;
    commandAnswer = '旧臂命令 2>&1';
    deltaAnswers = [{ text: 'body\n', nextOffset: 5 }];
    const { result, rerender } = await mounted();
    // 首次命令读被闸门悬停 ⇒ 尚未落值
    expect(result.current.command).toBeNull();

    commandAnswer = '新臂命令 2>&1';
    rerender(propsWith({ changeName: 'renamed-change' }));
    await flush();
    expect(callsOf('read_exec_command')).toHaveLength(2);
    expect(result.current.command).toBe('新臂命令 2>&1');

    gate.release();
    await flush();
    // 旧臂已被清理（disposed）⇒ 迟到应答丢弃，命令面保持新臂值
    expect(result.current.command).toBe('新臂命令 2>&1');
  });

  it('边界：命令面换臂后旧臂迟到的 reject 不清空新臂命令', async () => {
    const gate = makeGate(1);
    commandGate = gate;
    commandAnswer = new Error('旧臂命令读失败');
    deltaAnswers = [{ text: 'body\n', nextOffset: 5 }];
    const { result, rerender } = await mounted();
    expect(result.current.command).toBeNull();

    commandAnswer = '新臂命令 2>&1';
    rerender(propsWith({ changeName: 'renamed-change' }));
    await flush();
    expect(result.current.command).toBe('新臂命令 2>&1');

    gate.release();
    await flush();
    expect(result.current.command).toBe('新臂命令 2>&1');
    expect(result.current.error).toBeNull();
  });

  it('异常：命令面当前臂 reject → command 置 null（上一臂命令不残留）', async () => {
    commandAnswer = '首臂命令 2>&1';
    deltaAnswers = [
      { text: 'a\n', nextOffset: 2 },
      { text: 'b\n', nextOffset: 2 },
    ];
    const { result, rerender } = await mounted();
    expect(result.current.command).toBe('首臂命令 2>&1');

    commandAnswer = new Error('命令读取失败');
    rerender(propsWith({ changeName: 'renamed-change' }));
    await flush();

    expect(callsOf('read_exec_command')).toHaveLength(2);
    expect(result.current.command).toBeNull();
    // 命令面失败不写 error（内容面独立）
    expect(result.current.error).toBeNull();
    expect(result.current.text).toBe('b\n');
  });

  it('边界：内容面换臂后旧臂迟到的 reject 不回写 error（权威内容保位）', async () => {
    const gate = makeGate(1);
    deltaGate = gate;
    deltaAnswers = [new Error('旧臂读失败')];
    const { result, rerender } = await mounted();
    expect(result.current.text).toBeNull();

    deltaAnswers = [{ text: 'authoritative\n', nextOffset: 14 }];
    rerender(propsWith({ status: 'passed' }));
    await flush();
    expect(result.current.text).toBe('authoritative\n');

    gate.release();
    await flush();
    // 旧臂已 disposed ⇒ 迟到 reject 不写 error，也不动新臂文本
    expect(result.current.error).toBeNull();
    expect(result.current.text).toBe('authoritative\n');
  });

  it('边界：内容面换臂后旧臂迟到的成功应答不与新臂文本混拼', async () => {
    const gate = makeGate(1);
    deltaGate = gate;
    deltaAnswers = [{ text: 'stale\n', nextOffset: 6 }];
    const { result, rerender } = await mounted();
    expect(result.current.text).toBeNull();

    deltaAnswers = [{ text: 'authoritative\n', nextOffset: 14 }];
    rerender(propsWith({ status: 'passed' }));
    await flush();
    expect(result.current.text).toBe('authoritative\n');

    gate.release();
    await flush();
    expect(result.current.text).toBe('authoritative\n');
  });
});

describe('useExecOutput：寻址键消失后的复位（每臂起点清空）', () => {
  it('边界：root 转为 null → command / text / error 全复位为 null（不残留上一臂值）', async () => {
    deltaAnswers = [{ text: 'arm-a\n', nextOffset: 6 }];
    const { result, rerender } = await mounted();
    expect(result.current.command).toBe('pnpm run check 2>&1');
    expect(result.current.text).toBe('arm-a\n');

    rerender(propsWith({ root: null }));
    await flush();

    expect(result.current.command).toBeNull();
    expect(result.current.text).toBeNull();
    expect(result.current.error).toBeNull();
    expect(callsOf('watch_unsubscribe')).toHaveLength(1);
  });

  it('边界：changeName 转为 null → 命令面 / 内容面复位（含 root / execId 仍在场的组合）', async () => {
    deltaAnswers = [{ text: 'arm-a\n', nextOffset: 6 }];
    const { result, rerender } = await mounted();
    expect(result.current.command).toBe('pnpm run check 2>&1');

    rerender(propsWith({ changeName: null }));
    await flush();

    expect(result.current.command).toBeNull();
    expect(result.current.text).toBeNull();
    expect(result.current.error).toBeNull();
  });

  it('边界：execId 转为 null → 命令面 / 内容面复位且订阅退订', async () => {
    deltaAnswers = [{ text: 'arm-a\n', nextOffset: 6 }];
    const { result, rerender } = await mounted();
    expect(result.current.text).toBe('arm-a\n');

    rerender(propsWith({ execId: null }));
    await flush();

    expect(result.current.text).toBeNull();
    expect(result.current.error).toBeNull();
    expect(result.current.command).toBeNull();
    expect(callsOf('watch_unsubscribe')).toHaveLength(1);
  });

  it('边界：错误态臂转为 null → error 复位为 null（错误态不跨臂残留）', async () => {
    deltaAnswers = [new Error('臂内读失败')];
    const { result, rerender } = await mounted();
    expect(result.current.error).toBe('臂内读失败');

    rerender(propsWith({ execId: null }));
    await flush();
    expect(result.current.error).toBeNull();

    // 再回到有效臂：从 null 起（旧错误不复活）
    deltaAnswers = [{ text: 'fresh\n', nextOffset: 6 }];
    rerender(propsWith());
    await flush();
    expect(result.current.error).toBeNull();
    expect(result.current.text).toBe('fresh\n');
  });

  it('边界：三键同时置 null 后再次就绪 → 命令面与内容面各自重取一次（不因复位而丢臂）', async () => {
    deltaAnswers = [{ text: 'first\n', nextOffset: 6 }];
    const { result, rerender } = await mounted();
    expect(result.current.text).toBe('first\n');

    rerender(propsWith({ root: null, changeName: null, execId: null }));
    await flush();
    const callsAfterReset = invokeMock.mock.calls.length;

    deltaAnswers = [{ text: 'second\n', nextOffset: 7 }];
    rerender(propsWith());
    await flush();

    expect(result.current.text).toBe('second\n');
    expect(result.current.command).toBe('pnpm run check 2>&1');
    // 复位期零新增取数（守卫内只做状态清空），再就绪才重新取数
    expect(deltaCalls().length).toBeGreaterThan(1);
    expect(invokeMock.mock.calls.length).toBeGreaterThan(callsAfterReset);
  });
});

// ---------------------------------------------------------------------------
// 同臂双读（design D10-a 单飞守卫；AC-4「连续增量拼接 == 全量」与 AC-6
// 「offset 推进」在前端侧的必要判据）。
//
// 无守卫时同臂两次在飞读共享**同一** `offsetRef`（后端 `read_exec_output_delta_in`
// 是整文件读、非瞬时，500 ms trailing 防抖只合并**信号**、不覆盖**在飞时长**），
// 后果是静默破约：同段文本被追加两遍、两拍 `nextOffset` 同值。三枚 ref 的内部
// 状态不可直读（不可测试项表），故三臂全走**可观测后果**路线：
// ① `pendingRef` 由「在飞期间 `read_exec_output_delta` 恒 1 次调用」锁定；
// ② `queuedRef` 的布尔队列语义由「2 次与 5 次信号的总调用次数相同」锁定；
// ③ 结算清位由「reject 之后新读仍能发出」锁定（未清位即永久卡死）。
// 时序装置沿用既有 `makeGate()` + `advanceSignalWindow()`，不新增替身。
// ---------------------------------------------------------------------------

describe('useExecOutput：同臂双读（D10-a 单飞守卫）', () => {
  it('正向（同臂双读 · 在飞折叠为至多一拍尾读）：在飞期间两次信号折叠成一拍，放行后拼接逐字等于全量', async () => {
    const gate = makeGate(1);
    deltaGate = gate;
    deltaAnswers = [
      { text: 'aaaa\n', nextOffset: 5 },
      { text: 'bbbb\n', nextOffset: 10 },
    ];

    const { result } = await mounted();
    expect(deltaCalls()).toHaveLength(1);
    expect((deltaCalls()[0][1] as { fromOffset: number }).fromOffset).toBe(0);

    // 首拍悬停在「在飞」：其间两次信号各自走完防抖窗口
    const channel = ChannelMock.instances[0];
    channel.deliver();
    await advanceSignalWindow();
    channel.deliver();
    await advanceSignalWindow();

    // 在飞时**不叠读**：调用次数仍为 1（守卫入口短路，只记一次排队）
    expect(deltaCalls()).toHaveLength(1);
    expect(result.current.text).toBeNull();

    gate.release();
    await flush();
    await advanceSignalWindow();

    // 两信号折叠成**一拍**尾读：总调用 2 次，第二拍游标取自第一拍 nextOffset
    const calls = deltaCalls();
    expect(calls).toHaveLength(2);
    const offsets = calls.map((call) => (call[1] as { fromOffset: number }).fromOffset);
    expect(offsets).toEqual([0, 5]);
    // 严格递增（无守卫时两拍同为 0，offset 推进无从观测）
    expect(offsets[1]).toBeGreaterThan(offsets[0]);
    // 拼接逐字等于全量：同段文本 MUST NOT 出现两遍
    expect(result.current.text).toBe('aaaa\nbbbb\n');
  });

  it('边界（同臂双读 · 尾读有界）：在飞期间五次信号仍只补一拍尾读', async () => {
    const gate = makeGate(1);
    deltaGate = gate;
    deltaAnswers = [
      { text: 'aaaa\n', nextOffset: 5 },
      { text: 'bbbb\n', nextOffset: 10 },
    ];

    const { result } = await mounted();
    const channel = ChannelMock.instances[0];
    for (let index = 0; index < 5; index += 1) {
      channel.deliver();
      await advanceSignalWindow();
    }

    expect(deltaCalls()).toHaveLength(1);

    gate.release();
    await flush();
    await advanceSignalWindow();

    // 队列长度恒 ≤ 1：5 次信号与 2 次信号的总调用次数相同（均 2）
    expect(deltaCalls()).toHaveLength(2);
    expect(result.current.text).toBe('aaaa\nbbbb\n');
  });

  it('异常（同臂双读 · reject 后守卫不卡死）：首拍 reject 清位，后续信号的新读正常发出并追加', async () => {
    deltaAnswers = [new Error('首拍读失败'), { text: 'after\n', nextOffset: 6 }];

    const { result } = await mounted();
    expect(result.current.error).toBe('首拍读失败');
    expect(result.current.text).toBeNull();
    expect(deltaCalls()).toHaveLength(1);

    ChannelMock.instances[0].deliver();
    await advanceSignalWindow();

    // 标记未清则此后永不再取数——本臂即守卫最危险失效形态的判据
    expect(deltaCalls()).toHaveLength(2);
    expect(result.current.text).toBe('after\n');
  });
});
