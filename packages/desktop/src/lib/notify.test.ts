import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { NotificationLevel } from '../types/generated/bindings';
import { notify, present } from './notify';

// ---------------------------------------------------------------------------
// 进程边界 Mock：`sonner`（渲染器）与 `@tauri-apps/api/core`（`invoke`）。
// `lib/ipc.ts`（Proxy + zod strict 参数校验 + IpcArgsError 同步抛）**真实组合**——
// 替身化 `./ipc` 会绕开被测的「门面 → ipc 代理 → invoke」链路。
// ---------------------------------------------------------------------------

const { invokeMock, toastMock } = vi.hoisted(() => {
  const toastMock = Object.assign(vi.fn(), {
    success: vi.fn(),
    warning: vi.fn(),
    error: vi.fn(),
  });
  return { invokeMock: vi.fn(), toastMock };
});

vi.mock('sonner', () => ({ toast: toastMock }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

const MESSAGE = '删除 workspace 失败';

/** 落库命令的实参（`invoke('notification_record', { record })` 的第二个实参）。 */
interface RecordCall {
  record: {
    id: string;
    level: NotificationLevel;
    message: string;
    body: string | null;
  };
}

function recordCalls(): RecordCall[] {
  return invokeMock.mock.calls
    .filter(([command]) => command === 'notification_record')
    .map(([, payload]) => payload as RecordCall);
}

function allToastMocks() {
  return [toastMock, toastMock.success, toastMock.warning, toastMock.error];
}

/** 微任务刷盘（fire-and-forget 落库的静默失败面观测窗口）。 */
async function flush(): Promise<void> {
  await new Promise((resolveFlush) => setTimeout(resolveFlush, 0));
}

const UUID_SHAPE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockResolvedValue(null);
  for (const mock of allToastMocks()) mock.mockClear();
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
});

// ---------------------------------------------------------------------------
// present：四档呈现形态（逐字对位收口前的直调）
// ---------------------------------------------------------------------------

describe('通知门面 present：四档呈现形态', () => {
  it('四档各一次且无 body 时零第二实参（info 为裸 toast 调用）', () => {
    present('info', `${MESSAGE}-info`);
    present('success', `${MESSAGE}-success`);
    present('warning', `${MESSAGE}-warning`);
    present('error', `${MESSAGE}-error`);

    expect(toastMock.mock.calls).toEqual([[`${MESSAGE}-info`]]);
    expect(toastMock.success.mock.calls).toEqual([[`${MESSAGE}-success`]]);
    expect(toastMock.warning.mock.calls).toEqual([[`${MESSAGE}-warning`]]);
    expect(toastMock.error.mock.calls).toEqual([[`${MESSAGE}-error`]]);
  });

  it('body 在场时四档的第二实参恰为 { description }（对象只含一个键，无 id）', () => {
    const body = '目录被占用';
    const levels: NotificationLevel[] = ['info', 'success', 'warning', 'error'];
    const targets = [toastMock, toastMock.success, toastMock.warning, toastMock.error];

    levels.forEach((level, index) => {
      present(level, `${MESSAGE}-${level}`, body);
      const calls = targets[index].mock.calls;
      expect(calls).toHaveLength(1);
      expect(calls[0]).toHaveLength(2);
      expect(calls[0][1]).toEqual({ description: body });
      expect(Object.keys(calls[0][1] as object)).toEqual(['description']);
    });
  });

  it('present 全程零落库调用（呈现不写库，供后端事件轨复用）', () => {
    present('error', MESSAGE, '副文案');
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('body 为空串仍补第二实参、body 缺席则零第二实参（判据是 !== undefined）', () => {
    present('error', MESSAGE, '');
    expect(toastMock.error.mock.calls[0]).toEqual([MESSAGE, { description: '' }]);

    toastMock.error.mockClear();
    present('error', MESSAGE, undefined);
    expect(toastMock.error.mock.calls[0]).toEqual([MESSAGE]);
  });

  it('超长主文案与特殊字符原样透传（零截断、零转义）', () => {
    const long = '文'.repeat(1200);
    const special = '含换行\n与 emoji 🎉 与引号"';

    present('warning', long);
    expect(toastMock.warning.mock.calls[0][0]).toBe(long);

    present('warning', special);
    expect(toastMock.warning.mock.calls[1][0]).toBe(special);
  });

  it('四档外的未知 level 不抛错且零 toast 调用', () => {
    expect(() => {
      present('debug' as unknown as NotificationLevel, MESSAGE);
    }).not.toThrow();
    for (const mock of allToastMocks()) expect(mock).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// notify：呈现 + fire-and-forget 落库
// ---------------------------------------------------------------------------

describe('通知门面 notify：呈现一次 + 落库一次', () => {
  it('呈现与落库各一次且落库入参为 UUID 主键与 body 归一', async () => {
    notify('error', MESSAGE);

    expect(toastMock.error.mock.calls).toEqual([[MESSAGE]]);
    const calls = recordCalls();
    expect(calls).toHaveLength(1);
    expect(calls[0].record.id).toMatch(UUID_SHAPE);
    expect(calls[0].record.level).toBe('error');
    expect(calls[0].record.message).toBe(MESSAGE);
    expect(calls[0].record.body).toBeNull();

    await flush();
  });

  it('四档 level 逐档原值出线', async () => {
    const levels: NotificationLevel[] = ['info', 'success', 'warning', 'error'];
    for (const level of levels) notify(level, `${MESSAGE}-${level}`);

    expect(recordCalls().map((call) => call.record.level)).toEqual(levels);
    await flush();
  });

  it('body 缺席出线为 null、空串出线仍为空串（不归一）', async () => {
    notify('warning', MESSAGE);
    notify('warning', MESSAGE, '');

    const [withoutBody, withEmptyBody] = recordCalls();
    expect(withoutBody.record.body).toBeNull();
    expect(withEmptyBody.record.body).toBe('');

    await flush();
  });

  it('两次 notify 的主键互不相同（每次铸造）', async () => {
    notify('info', MESSAGE);
    notify('info', MESSAGE);

    const [first, second] = recordCalls();
    expect(first.record.id).not.toBe(second.record.id);
    expect(first.record.id).toMatch(UUID_SHAPE);
    expect(second.record.id).toMatch(UUID_SHAPE);

    await flush();
  });

  it('落库 reject 时零外溢、toast 仍一次、零重试、DEV 恰一行 warn', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    invokeMock.mockRejectedValueOnce(new Error('IPC 失败'));

    expect(() => {
      notify('error', MESSAGE);
    }).not.toThrow();
    await flush();

    expect(toastMock.error.mock.calls).toEqual([[MESSAGE]]);
    expect(invokeMock.mock.calls).toHaveLength(1);
    expect(warn).toHaveBeenCalledTimes(1);
  });

  it('真实 ipc 参数面同步拒绝（越界 body）时零外溢、toast 一次、零 IPC 往返', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});

    // 越界入参（运行期脏值）：present 照常渲染，落库经真实 lib/ipc.ts 的 zod
    // strict 校验同步抛 IpcArgsError（非替身伪造）
    expect(() => {
      notify('error', MESSAGE, 42 as unknown as string);
    }).not.toThrow();
    await flush();

    expect(toastMock.error.mock.calls).toEqual([[MESSAGE, { description: 42 }]]);
    expect(invokeMock).not.toHaveBeenCalled();
    expect(warn).toHaveBeenCalledTimes(1);
  });

  it('同步抛出（主键铸造失败）时同样零外溢、toast 一次、DEV 恰一行 warn', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const randomUUID = vi.fn(() => {
      throw new Error('randomUUID 不可用');
    });
    vi.stubGlobal('crypto', { randomUUID });

    expect(() => {
      notify('error', MESSAGE);
    }).not.toThrow();
    await flush();

    expect(toastMock.error.mock.calls).toEqual([[MESSAGE]]);
    expect(invokeMock).not.toHaveBeenCalled();
    expect(warn).toHaveBeenCalledTimes(1);
  });

  it('prod 档（DEV=false）落库失败零留痕', async () => {
    vi.stubEnv('DEV', false);
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const error = vi.spyOn(console, 'error').mockImplementation(() => {});
    invokeMock.mockRejectedValueOnce(new Error('IPC 失败'));

    notify('error', MESSAGE);
    await flush();

    expect(warn).not.toHaveBeenCalled();
    expect(error).not.toHaveBeenCalled();
    expect(toastMock.error.mock.calls).toEqual([[MESSAGE]]);
  });
});

// ---------------------------------------------------------------------------
// 结构锚（AC-2）：sonner 唯一持有面 + 调用点全部改经门面
// ---------------------------------------------------------------------------

const SRC = resolve(process.cwd(), 'src');

/** `src/**` 下全部 `.ts` / `.tsx` 源文件（相对路径）。 */
function sourceFiles(dir: string = SRC): string[] {
  const found: string[] = [];
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) {
      found.push(...sourceFiles(path));
      continue;
    }
    if (/\.tsx?$/.test(entry)) found.push(path);
  }
  return found;
}

function relative(path: string): string {
  return path.slice(SRC.length + 1).replace(/\\/g, '/');
}

function textOf(relativePath: string): string {
  return readFileSync(join(SRC, relativePath), 'utf8');
}

/** 去注释后的代码文本（结构锚只数代码字面——doc / 注释里提到同一词不算调用点）。 */
function codeOf(source: string): string {
  return source.replace(/\/\*[\s\S]*?\*\//g, '').replace(/^\s*\/\/.*$/gm, '');
}

/** 12 处业务调用点所在文件（收口后零 sonner 直引）。 */
const CALL_SITES = [
  'hooks/use-workspaces.ts',
  'views/agents/hooks/use-agent-instances.ts',
  'views/agents/hooks/use-agent-providers.ts',
  'views/changes/hooks/use-change-list.ts',
  'views/architecture/components/dashboard/contract-panel.tsx',
  'views/explores/components/explore-open-file.tsx',
];

describe('通知门面结构锚：sonner 唯一持有面', () => {
  it("生产代码里 from 'sonner' 恰两处（sonner 渲染件与门面）", () => {
    const holders = sourceFiles()
      .filter((path) => !/\.test\.tsx?$/.test(path))
      .filter((path) => /from 'sonner'/.test(readFileSync(path, 'utf8')))
      .map(relative)
      .sort();

    expect(holders).toEqual(['components/ui/sonner.tsx', 'lib/notify.ts']);
  });

  it('六个业务调用点零 toast 直调残留且均改引门面', () => {
    for (const site of CALL_SITES) {
      const source = textOf(site);
      expect(source).not.toMatch(/\btoast\(/);
      expect(source).not.toMatch(/\btoast\.(error|success|warning)\(/);
      expect(source).toMatch(/from '[./@\w-]*lib\/notify'/);
    }
  });

  it('门面内 crypto.randomUUID 恰一处且 notificationRecord 调用恰一处', () => {
    const code = codeOf(textOf('lib/notify.ts'));
    expect(code.match(/crypto\.randomUUID/g)).toHaveLength(1);
    expect(code.match(/ipc\s*\.notificationRecord/g)).toHaveLength(1);
    expect(code).not.toMatch(/setTimeout|setInterval/);
  });
});
