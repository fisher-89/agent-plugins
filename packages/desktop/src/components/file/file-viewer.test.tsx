import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { FileViewer } from './file-viewer';

// ---------------------------------------------------------------------------
// FileViewer 单测：空态 → 抽屉开合 → 页签命名（路径末段）→ 按后缀 / 显式 type
// 分派渲染（md → markdown，其余 → 文本）→ 取数三态（加载 / 失败 / 成文）→
// 多文件按需取数（非激活页签零 invoke）。
//
// 进程边界 Mock：`@tauri-apps/api/core` 的 `invoke`（唯一读盘命令
// `archi_read_doc`）；`StandardDrawer` / base-ui Tabs 真实渲染。
// ---------------------------------------------------------------------------

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

const ROOT = 'C:\\demo\\alpha';

function readCalls(): Record<string, unknown>[] {
  return invokeMock.mock.calls
    .filter(([command]) => command === 'read_file')
    .map(([, params]) => (params ?? {}) as Record<string, unknown>);
}

/** 触发器（trigger prop 经 DrawerTrigger render 落地）。 */
function trigger() {
  return <button type="button">预览文档</button>;
}

/** 打开抽屉（trigger 档；每条用例只开一次）。 */
async function open(): Promise<void> {
  fireEvent.click(screen.getByRole('button', { name: '预览文档' }));
  await waitFor(() => expect(screen.getByRole('tablist') !== null).toBe(true));
}

/** 可控应答：`promise` 由用例决定何时落定（观测加载态 / 错误态）。 */
function deferred<T>() {
  let resolve: (value: T) => void = () => {};
  let reject: (reason: unknown) => void = () => {};
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

/** 后端 FileSlice 的最小书面形状（内容 + stat + 游标）。 */
function sliceOf(content: string) {
  return {
    content,
    stat: { size: content.length, modifiedMs: null, isFile: true, isDir: false },
    offset: 0,
    nextOffset: content.length,
    truncated: false,
    encoding: 'utf8',
  };
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation(() => Promise.resolve(sliceOf('# 认证说明\n\n正文')));
});

afterEach(() => {
  cleanup();
});

describe('FileViewer：空态', () => {
  it('files 为空：呈空态且零 invoke、零触发器', async () => {
    render(<FileViewer root={ROOT} files={[]} trigger={trigger()} />);

    expect(screen.getByTestId('file-viewer-empty').textContent).toContain('无文件可预览');
    expect(screen.queryByRole('button', { name: '预览文档' })).toBeNull();
    await act(async () => {
      await Promise.resolve();
    });
    expect(invokeMock).not.toHaveBeenCalled();
  });
});

describe('FileViewer：抽屉与页签', () => {
  it('打开抽屉：标题缺省「文档预览」，页签名取路径末段且按序排列', async () => {
    render(
      <FileViewer
        root={ROOT}
        trigger={trigger()}
        files={[{ path: 'src/auth/README.md' }, { path: 'docs\\guide.md' }]}
      />,
    );
    expect(screen.queryByRole('heading', { name: '文档预览' })).toBeNull();

    await open();

    expect(screen.getByRole('heading', { name: '文档预览' }) !== null).toBe(true);
    expect(screen.getAllByRole('tab').map((tab) => tab.textContent)).toEqual([
      'README.md',
      'guide.md',
    ]);
  });

  it('自定义 title 透传给抽屉标题', async () => {
    render(
      <FileViewer root={ROOT} title="架构文档" files={[{ path: 'a.md' }]} trigger={trigger()} />,
    );
    await open();
    expect(screen.getByRole('heading', { name: '架构文档' }) !== null).toBe(true);
  });
});

describe('FileViewer：按类型分派渲染', () => {
  it('md 后缀 → markdown 渲染（标题 / 列表 / 代码块）', async () => {
    invokeMock.mockImplementation(() =>
      Promise.resolve(
        sliceOf('# 令牌收口\n\n- 收口到 Token 模块\n\n```ts\nconst sink = 1;\n```\n'),
      ),
    );
    render(<FileViewer root={ROOT} files={[{ path: 'docs/adr.md' }]} trigger={trigger()} />);
    await open();

    const body = await screen.findByTestId('file-content-markdown');
    expect(body.querySelector('h1')?.textContent).toBe('令牌收口');
    expect(body.querySelector('li')?.textContent).toContain('收口到 Token 模块');
    expect(body.querySelector('code')?.textContent).toContain('const sink = 1;');
    expect(readCalls()).toEqual([{ path: `${ROOT}/docs/adr.md`, offset: null, limit: null }]);
  });

  it('非 md 后缀 → 纯文本渲染（不解析 markdown 语法）', async () => {
    invokeMock.mockImplementation(() => Promise.resolve(sliceOf('# 不是标题\n')));
    render(<FileViewer root={ROOT} files={[{ path: 'src/auth/index.ts' }]} trigger={trigger()} />);
    await open();

    const body = await screen.findByTestId('file-content-text');
    expect(body.textContent).toBe('# 不是标题\n');
    expect(body.querySelector('h1')).toBeNull();
  });

  it('显式 type 覆盖路径后缀（无后缀路径也可按 md 渲染）', async () => {
    render(
      <FileViewer root={ROOT} files={[{ path: 'docs/NOTICE', type: 'md' }]} trigger={trigger()} />,
    );
    await open();

    expect(await screen.findByTestId('file-content-markdown')).toBeTruthy();
    expect(screen.queryByTestId('file-content-text')).toBeNull();
  });

  it('显式 type 大小写归一（MD 等同 md）', async () => {
    render(
      <FileViewer
        root={ROOT}
        files={[{ path: 'docs/note.txt', type: 'MD' }]}
        trigger={trigger()}
      />,
    );
    await open();

    expect(await screen.findByTestId('file-content-markdown')).toBeTruthy();
  });
});

describe('FileViewer：取数三态', () => {
  it('取数未回：呈加载行，不渲染正文 / 错误', async () => {
    const pending = deferred<ReturnType<typeof sliceOf> | null>();
    invokeMock.mockImplementation(() => pending.promise);
    render(<FileViewer root={ROOT} files={[{ path: 'a.md' }]} trigger={trigger()} />);
    await open();

    expect(await screen.findByTestId('file-content-loading')).toBeTruthy();
    expect(screen.queryByTestId('file-content-markdown')).toBeNull();
    expect(screen.queryByTestId('file-content-error')).toBeNull();

    await act(async () => {
      pending.resolve(sliceOf('# 迟到正文'));
    });
    await waitFor(() => expect(screen.getByTestId('file-content-markdown')).toBeTruthy());
    expect(screen.queryByTestId('file-content-loading')).toBeNull();
  });

  it('取数失败：行内呈现错误串，正文与加载行均不在场', async () => {
    invokeMock.mockImplementation(() => Promise.reject(new Error('读取失败：EPERM')));
    render(<FileViewer root={ROOT} files={[{ path: 'a.md' }]} trigger={trigger()} />);
    await open();

    const error = await screen.findByTestId('file-content-error');
    expect(error.textContent).toContain('读取失败');
    expect(screen.queryByTestId('file-content-loading')).toBeNull();
    expect(screen.queryByTestId('file-content-markdown')).toBeNull();
  });

  it('路径缺失（后端返回 null）：呈「文件不存在」错误态，不空白', async () => {
    invokeMock.mockImplementation(() => Promise.resolve(null));
    render(<FileViewer root={ROOT} files={[{ path: 'missing.md' }]} trigger={trigger()} />);
    await open();

    const error = await screen.findByTestId('file-content-error');
    expect(error.textContent).toContain('文件不存在');
  });

  it('二进制文件：呈二进制提示，不渲染正文', async () => {
    invokeMock.mockImplementation(() => Promise.resolve({ ...sliceOf(''), encoding: 'binary' }));
    render(<FileViewer root={ROOT} files={[{ path: 'blob.bin' }]} trigger={trigger()} />);
    await open();

    expect(await screen.findByTestId('file-content-binary')).toBeTruthy();
    expect(screen.queryByTestId('file-content-text')).toBeNull();
  });
});

describe('FileViewer：多文件按需取数', () => {
  it('只取激活页签：切换后新页签才发 invoke，且入参为对应路径', async () => {
    render(
      <FileViewer
        root={ROOT}
        files={[{ path: 'src/auth/README.md' }, { path: 'src/auth/index.ts' }]}
        trigger={trigger()}
      />,
    );
    await open();

    await waitFor(() =>
      expect(readCalls()).toEqual([
        { path: `${ROOT}/src/auth/README.md`, offset: null, limit: null },
      ]),
    );
    expect(screen.getByTestId('file-content-markdown')).toBeTruthy();

    fireEvent.click(screen.getByRole('tab', { name: 'index.ts' }));

    await waitFor(() =>
      expect(readCalls()).toEqual([
        { path: `${ROOT}/src/auth/README.md`, offset: null, limit: null },
        { path: `${ROOT}/src/auth/index.ts`, offset: null, limit: null },
      ]),
    );
    expect(await screen.findByTestId('file-content-text')).toBeTruthy();
  });

  it('页签切换按挂载取数：面板随激活卸载 / 重挂，切回即重取（无跨页签缓存）', async () => {
    render(
      <FileViewer root={ROOT} files={[{ path: 'a.md' }, { path: 'b.md' }]} trigger={trigger()} />,
    );
    await open();
    await waitFor(() => expect(readCalls()).toHaveLength(1));

    fireEvent.click(screen.getByRole('tab', { name: 'b.md' }));
    await waitFor(() => expect(readCalls()).toHaveLength(2));

    fireEvent.click(screen.getByRole('tab', { name: 'a.md' }));
    await waitFor(() =>
      expect(readCalls().map((call) => call.path)).toEqual([
        `${ROOT}/a.md`,
        `${ROOT}/b.md`,
        `${ROOT}/a.md`,
      ]),
    );
  });
});
