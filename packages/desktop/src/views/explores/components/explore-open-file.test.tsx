import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { driftCount, resetDrift } from '../../../lib/ipc-drift';
import { ExploreOpenFile } from './explore-open-file';

// 进程边界 Mock：仅 mock IPC（invoke）与 toast（失败面只上报，不进 DOM）。
// 下拉菜单本体（base-ui Menu）真实渲染，开合以 mousedown + 异步收敛断言承载
// （与 app-sidebar 的 workspace 选择器用例同式）。
const { invokeMock, toastMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  toastMock: { error: vi.fn() },
}));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('sonner', () => ({ toast: toastMock }));

const ROOT = 'C:\\demo\\alpha';

type OnCreated = (name: string, autoPrompt: string | null) => void;

let onCreated: OnCreated;

/** 合法 `ExploreRecord`（九字段全在场——strict 面零漂移的干净基线）。 */
function exploreRecord(name: string): Record<string, unknown> {
  return {
    id: 1,
    root: ROOT,
    name,
    title: name,
    workflowType: 'requirement',
    promotedTo: null,
    sessionId: null,
    createdAt: 1,
    updatedAt: 1,
  };
}

/** 扫描应答（可编程；null = 命令挂起）。 */
let scanAnswer: () => Promise<unknown>;

/** 建档应答（可编程）。 */
let createAnswer: () => Promise<unknown>;

/** 按命令名分发的进程边界应答面。 */
function ipc(command: string): Promise<unknown> {
  if (command === 'scan_explores') return scanAnswer();
  if (command === 'create_explore_record') return createAnswer();
  return Promise.resolve(null);
}

/** 挂载导入下拉（触发器 = 消费方渲染的图标按钮；base-ui 触发 props 原样落地）。 */
function renderOpenFile(): ReturnType<typeof render> {
  return render(
    <ExploreOpenFile
      root={ROOT}
      onCreated={onCreated}
      trigger={(triggerProps) => (
        <button type="button" {...triggerProps}>
          导入探索文档
        </button>
      )}
    />,
  );
}

/** 以指定 root 挂载（root 切换用例的 rerender 面）。 */
function openFileAt(root: string): React.JSX.Element {
  return (
    <ExploreOpenFile
      root={root}
      onCreated={onCreated}
      trigger={(triggerProps) => (
        <button type="button" {...triggerProps}>
          导入探索文档
        </button>
      )}
    />
  );
}

/** 打开下拉：base-ui Menu 以 mousedown 开启（开启动作为 frame 异步）。 */
async function openMenu(): Promise<void> {
  fireEvent.mouseDown(screen.getByRole('button', { name: '导入探索文档' }));
  await screen.findByRole('menu');
}

/** 建档命令调用载荷清单。 */
function createCalls(): Array<Record<string, unknown>> {
  return invokeMock.mock.calls
    .filter(([command]) => command === 'create_explore_record')
    .map(([, args]) => args as Record<string, unknown>);
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => ipc(command));
  toastMock.error.mockReset();
  resetDrift();
  onCreated = vi.fn<OnCreated>();
  scanAnswer = () => Promise.resolve([]);
  createAnswer = () => Promise.resolve(null);
});

afterEach(() => {
  cleanup();
});

// ---------------------------------------------------------------------------
// 扫描面：挂载即扫、扫描中 / 空态 / 候选三种呈现
// ---------------------------------------------------------------------------

describe('ExploreOpenFile：扫描面', () => {
  it('正向：挂载即以 root 发起 scan_explores，持有候选期间呈「扫描中…」', async () => {
    scanAnswer = () => new Promise(() => {});
    renderOpenFile();

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('scan_explores', { root: ROOT }));
    await openMenu();
    expect(screen.getByTestId('explore-import-loading').textContent).toBe('扫描中…');
    expect(screen.queryByTestId('explore-import-item')).toBeNull();
  });

  it('边界：扫描返回空 → 空态提示、零候选、零错误', async () => {
    renderOpenFile();

    await openMenu();
    expect(await screen.findByTestId('explore-import-empty')).toBeTruthy();
    expect(screen.queryAllByTestId('explore-import-item')).toHaveLength(0);
    expect(toastMock.error).not.toHaveBeenCalled();
  });

  it('正向：候选逐条呈现 stem，modifiedAt 非空时另附修改时间', async () => {
    const modifiedAt = 1727000000000;
    scanAnswer = () =>
      Promise.resolve([
        { name: 'with-time', modifiedAt },
        { name: 'no-time', modifiedAt: null },
      ]);
    renderOpenFile();

    await openMenu();
    const items = await screen.findAllByTestId('explore-import-item');
    expect(items).toHaveLength(2);
    expect(items[0]?.textContent).toContain('with-time');
    expect(items[0]?.textContent).toContain(new Date(modifiedAt).toLocaleString());
    expect(items[1]?.textContent).toBe('no-time');
  });

  it('异常：扫描 reject → toast.error 呈现后端原文、清单保持「扫描中…」空面（不崩）', async () => {
    scanAnswer = () => Promise.reject('目录不可读');
    renderOpenFile();

    await waitFor(() => expect(toastMock.error).toHaveBeenCalledWith('目录不可读'));
    await openMenu();
    expect(screen.getByTestId('explore-import-loading')).toBeTruthy();
  });
});

// ---------------------------------------------------------------------------
// 建档面：选中候选即绑定（显式名 invoke），成功后上抛 onCreated(name, null)
// ---------------------------------------------------------------------------

describe('ExploreOpenFile：选中建档', () => {
  it('正向：点击候选 → create_explore_record 携显式名、onCreated(name, null)（autoPrompt 恒 null）', async () => {
    scanAnswer = () => Promise.resolve([{ name: 'valid-topic', modifiedAt: null }]);
    createAnswer = () => Promise.resolve(exploreRecord('valid-topic'));
    renderOpenFile();

    await openMenu();
    fireEvent.click(await screen.findByTestId('explore-import-item'));

    await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
    expect(createCalls()).toEqual([{ root: ROOT, name: 'valid-topic' }]);
    expect(onCreated).toHaveBeenCalledWith('valid-topic', null);
  });

  it('异常：建档 reject → toast.error 呈现后端原文、onCreated 零调用', async () => {
    scanAnswer = () => Promise.resolve([{ name: 'valid-topic', modifiedAt: null }]);
    createAnswer = () => Promise.reject('笔记已被占用');
    renderOpenFile();

    await openMenu();
    fireEvent.click(await screen.findByTestId('explore-import-item'));

    await waitFor(() => expect(toastMock.error).toHaveBeenCalledWith('笔记已被占用'));
    expect(onCreated).not.toHaveBeenCalled();
  });

  it('边界：候选名超长（>1000 字符）/ 含换行与 emoji → 参数面放行、零漂移', async () => {
    const names = ['a'.repeat(1200), 'with\nnewline 🚀 topic'];
    for (const name of names) {
      scanAnswer = () => Promise.resolve([{ name, modifiedAt: null }]);
      createAnswer = () => Promise.resolve(exploreRecord(name));
      renderOpenFile();

      await openMenu();
      fireEvent.click(await screen.findByTestId('explore-import-item'));

      await waitFor(() =>
        expect(invokeMock).toHaveBeenCalledWith('create_explore_record', { root: ROOT, name }),
      );
      cleanup();
    }

    expect(driftCount()).toBe(0);
  });
});

// ---------------------------------------------------------------------------
// root 切换：以新 root 重扫，旧 root 迟到应答被取消抑制吞掉
// ---------------------------------------------------------------------------

describe('ExploreOpenFile：root 切换的取消抑制', () => {
  it('边界：root 切换 → 以新 root 重扫；旧 root 迟到应答不覆盖新清单', async () => {
    const ROOT2 = 'C:\\demo\\beta';
    let resolveStale: (value: unknown) => void = () => {};
    invokeMock.mockImplementation((command: string, params?: { root?: string }) => {
      if (command === 'scan_explores') {
        if (params?.root === ROOT) {
          return new Promise((resolve) => {
            resolveStale = resolve;
          });
        }
        return Promise.resolve([{ name: 'new-topic', modifiedAt: null }]);
      }
      return Promise.resolve(null);
    });
    const view = renderOpenFile();

    view.rerender(openFileAt(ROOT2));

    await openMenu();
    expect(await screen.findByTestId('explore-import-item')).toBeTruthy();
    expect(screen.getByText('new-topic')).toBeTruthy();

    // 旧 root 迟到应答被取消抑制吞掉（不覆盖新清单）
    resolveStale([{ name: 'stale-topic', modifiedAt: null }]);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('scan_explores', { root: ROOT2 }));
    expect(screen.queryByText('stale-topic')).toBeNull();
    expect(screen.getByText('new-topic')).toBeTruthy();
  });

  it('边界：旧 root 扫描迟到 reject → 不再 toast 错误', async () => {
    const ROOT2 = 'C:\\demo\\beta';
    let rejectStale: (err: unknown) => void = () => {};
    invokeMock.mockImplementation((command: string, params?: { root?: string }) => {
      if (command === 'scan_explores') {
        if (params?.root === ROOT) {
          return new Promise((_resolve, reject) => {
            rejectStale = reject;
          });
        }
        return Promise.resolve([{ name: 'new-topic', modifiedAt: null }]);
      }
      return Promise.resolve(null);
    });
    const view = renderOpenFile();

    view.rerender(openFileAt(ROOT2));
    await openMenu();
    await screen.findByText('new-topic');

    rejectStale('旧 root 目录不可读');
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('scan_explores', { root: ROOT2 }));
    expect(toastMock.error).not.toHaveBeenCalled();
    expect(screen.getByText('new-topic')).toBeTruthy();
  });
});

// ---------------------------------------------------------------------------
// 经 ipc 的结果面：只上报不阻断（漂移 collector 真实断言）
// ---------------------------------------------------------------------------

describe('ExploreOpenFile：结果面软失败', () => {
  it('异常：结果面契约外（返回记录缺 name）→ 漂移恰 1 条、onCreated 照旧被调用', async () => {
    scanAnswer = () => Promise.resolve([{ name: 'valid-topic', modifiedAt: null }]);
    createAnswer = () => {
      const dirty = exploreRecord('valid-topic');
      delete dirty.name;
      return Promise.resolve(dirty);
    };
    renderOpenFile();

    await openMenu();
    fireEvent.click(await screen.findByTestId('explore-import-item'));

    await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
    expect(driftCount()).toBe(1);
    expect(toastMock.error).not.toHaveBeenCalled();
  });
});
