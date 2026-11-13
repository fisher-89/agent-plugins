import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { renderToStaticMarkup } from 'react-dom/server';
import { toast } from 'sonner';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { Toaster } from '@/components/ui/sonner';

import type { ArchiApiCheck, ArchiApiFinding } from '../../../types/generated/bindings';
import { ContractPanel, type ContractPanelProps } from './contract-panel';

/**
 * `ContractPanel` 单测（AC-9 / AC-10 / AC-11 / AC-12 / D16 / D17 / D18）：两轴并列与各自
 * 计数（零跨轴合计）、三分组与组内计数、chip 由数据派生与过滤、条目点击 = 聚焦、路径行经
 * `FileViewer` + `read_file`、五态（加载 / 未启用 / 零差异零判据 / 检查失败行内 / 生成失败
 * 顶部）与首帧未载入态的「解析中…」判定序、生成二次确认与调用序、`full` 零生成入口、
 * `meta` 头部短摘要、零第二份取数。
 *
 * 进程边界 mock：`@tauri-apps/api/core` 的 `invoke`，按命令名分发 `archi_api_check` /
 * `archi_api_generate` / `read_file`。`useArchiApiCheck` / `FileViewer` / `StandardDialog` /
 * `sonner` 的 `<Toaster />` 均真实渲染（确认文案与 toast 内容是被测面）。
 *
 * 首帧未载入态（effect 未跑）由 `react-dom/server` 的静态渲染取得——不跑 effect 即该组合，
 * 无需替身化 hook。
 *
 * 查询锚点一律 `data-testid` / `data-axis` / `data-kind` / `data-category` / `data-source`。
 */

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

const ROOT = '/ws';

/** 五字段 DTO 夹具（三列缺省空、`enabled` 缺省 true；头部载元取不含「6」的形态，
 * 便于「跨轴合计不得出现」的文本判据不被版本号误伤）。 */
function apiCheck(overrides: Partial<ArchiApiCheck> = {}): ArchiApiCheck {
  return {
    enabled: true,
    meta: { toolVersion: '0.1.0', commonDigest: `sha256:${'b'.repeat(64)}` },
    mismatched: [],
    orphaned: [],
    missing: [],
    ...overrides,
  };
}

/** 六字段判据夹具。 */
function finding(overrides: Partial<ArchiApiFinding> = {}): ArchiApiFinding {
  return {
    kind: 'apiSignatureMismatch',
    category: 'signature',
    element: 'App.Auth',
    target: 'login',
    message: '契约签名与代码签名不一致',
    occurrences: [{ file: 'src/auth/lib.rs', line: 3, specifier: null, typeOnly: false }],
    ...overrides,
  };
}

/** 后端 `FileSlice` 的最小书面形状。 */
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

function callsOf(command: string): Record<string, unknown>[] {
  return invokeMock.mock.calls
    .filter(([name]) => name === command)
    .map(([, params]) => (params ?? {}) as Record<string, unknown>);
}

/** 缺省取数实现：`archi_api_check` 回传入 DTO、`archi_api_generate` 回全 written、
 * `read_file` 回一段文本。 */
function serve(check: ArchiApiCheck | Error): void {
  invokeMock.mockImplementation((command: string) => {
    if (command === 'archi_api_check') {
      return check instanceof Error ? Promise.reject(check) : Promise.resolve(check);
    }
    if (command === 'archi_api_generate') {
      return Promise.resolve({ written: ['a.i.rs'], unchanged: [] });
    }
    if (command === 'read_file') return Promise.resolve(sliceOf('// 生成物\n'));
    return Promise.reject(new Error(`未预期命令: ${command}`));
  });
}

/** 渲染面板（+ 真实 `<Toaster />`），返回 `onFocus` 观察器。 */
function renderPanel(overrides: Partial<ContractPanelProps> = {}) {
  const onFocus = vi.fn();
  const props: ContractPanelProps = {
    root: ROOT,
    mode: 'incremental',
    findings: [],
    onFocus,
    ...overrides,
  };
  render(
    <>
      <ContractPanel {...props} />
      <Toaster />
    </>,
  );
  return { onFocus };
}

function axis(axisName: 'artifacts' | 'findings'): HTMLElement {
  return screen.getByTestId(`archi-contract-axis-${axisName}`);
}

function itemsIn(axisName: 'artifacts' | 'findings'): HTMLElement[] {
  return within(axis(axisName)).queryAllByTestId('archi-contract-item');
}

beforeEach(() => {
  invokeMock.mockReset();
  toast.dismiss();
  serve(apiCheck());
});

afterEach(() => {
  cleanup();
  toast.dismiss();
});

// ---------------------------------------------------------------------------
// 两轴并列 / 三分组 / chip
// ---------------------------------------------------------------------------

describe('ContractPanel：两轴并列', () => {
  it('应分轴计数且零跨轴合计（上轴生成物差异、下轴判据各自取值）', async () => {
    serve(apiCheck({ mismatched: ['m.i.rs'], orphaned: ['o.i.ts'], missing: ['x.schema.json'] }));
    renderPanel({
      findings: [
        finding({ category: 'entry' }),
        finding({ category: 'signature' }),
        finding({ category: 'signature' }),
      ],
    });
    await waitFor(() => expect(itemsIn('artifacts')).toHaveLength(3));

    expect(axis('artifacts').textContent).toContain('生成物差异（3）');
    expect(axis('findings').textContent).toContain('判据（3）');
    for (const item of itemsIn('artifacts')) {
      expect(item.getAttribute('data-axis')).toBe('artifacts');
    }
    for (const item of itemsIn('findings')) {
      expect(item.getAttribute('data-axis')).toBe('findings');
    }
    // 跨轴合计（6）不得出现在任一容器或全局面文本里
    expect(axis('artifacts').textContent).not.toContain('（6）');
    expect(axis('findings').textContent).not.toContain('（6）');
    expect(document.body.textContent).not.toContain('（6）');
  });

  it('应三分组呈现且组内计数随数据（某列空即计数 0 且零条目）', async () => {
    serve(apiCheck({ mismatched: ['m1.i.rs', 'm2.i.rs'], orphaned: ['o1.i.ts'], missing: [] }));
    renderPanel();
    await waitFor(() => expect(itemsIn('artifacts')).toHaveLength(3));

    expect(axis('artifacts').textContent).toContain('不符 2');
    expect(axis('artifacts').textContent).toContain('孤儿 1');
    expect(axis('artifacts').textContent).toContain('缺失 0');
    expect(
      itemsIn('artifacts').filter((item) => item.getAttribute('data-kind') === 'mismatched'),
    ).toHaveLength(2);
    expect(
      itemsIn('artifacts').filter((item) => item.getAttribute('data-kind') === 'orphaned'),
    ).toHaveLength(1);
    expect(
      itemsIn('artifacts').filter((item) => item.getAttribute('data-kind') === 'missing'),
    ).toHaveLength(0);
  });

  it('应由数据派生 chip（去重 + 排序 + 零计数类别不渲）', async () => {
    renderPanel({
      findings: [
        finding({ category: 'entry' }),
        finding({ category: 'signature' }),
        finding({ category: 'signature' }),
        finding({ category: 'nesting' }),
      ],
    });
    await waitFor(() => expect(itemsIn('findings')).toHaveLength(4));

    const chips = within(axis('findings')).queryAllByTestId('archi-contract-chip');
    expect(chips.map((chip) => chip.textContent)).toEqual(['entry 1', 'nesting 1', 'signature 2']);
    expect(chips.map((chip) => chip.getAttribute('data-category'))).toEqual([
      'entry',
      'nesting',
      'signature',
    ]);
    expect(chips.some((chip) => chip.getAttribute('data-category') === 'contract')).toBe(false);
  });

  it('应支持 chip 过滤与取消（再点同 chip 恢复全量）', async () => {
    renderPanel({
      findings: [
        finding({ category: 'entry' }),
        finding({ category: 'signature' }),
        finding({ category: 'signature' }),
        finding({ category: 'nesting' }),
      ],
    });
    await waitFor(() => expect(itemsIn('findings')).toHaveLength(4));

    const signature = within(axis('findings'))
      .queryAllByTestId('archi-contract-chip')
      .find((chip) => chip.getAttribute('data-category') === 'signature');
    fireEvent.click(signature!);
    await waitFor(() => expect(itemsIn('findings')).toHaveLength(2));
    expect(
      within(axis('findings'))
        .queryAllByTestId('archi-contract-chip')
        .find((chip) => chip.getAttribute('data-category') === 'signature')
        ?.getAttribute('aria-pressed'),
    ).toBe('true');

    fireEvent.click(
      within(axis('findings'))
        .queryAllByTestId('archi-contract-chip')
        .find((chip) => chip.getAttribute('data-category') === 'signature')!,
    );
    await waitFor(() => expect(itemsIn('findings')).toHaveLength(4));

    fireEvent.click(
      within(axis('findings'))
        .queryAllByTestId('archi-contract-chip')
        .find((chip) => chip.getAttribute('data-category') === 'entry')!,
    );
    await waitFor(() => expect(itemsIn('findings')).toHaveLength(1));
  });
});

// ---------------------------------------------------------------------------
// 条目交互
// ---------------------------------------------------------------------------

describe('ContractPanel：条目交互', () => {
  it('应在判据条目点击时调一次 onFocus（实参为该条的 element）', async () => {
    const { onFocus } = renderPanel({
      findings: [
        finding({ element: 'App.Auth' }),
        finding({ element: 'App.Billing', category: 'entry' }),
      ],
    });
    await waitFor(() => expect(itemsIn('findings')).toHaveLength(2));

    fireEvent.click(itemsIn('findings')[0]!);
    expect(onFocus).toHaveBeenCalledTimes(1);
    expect(onFocus).toHaveBeenCalledWith('App.Auth');

    fireEvent.click(itemsIn('findings')[1]!);
    expect(onFocus).toHaveBeenCalledTimes(2);
    expect(onFocus).toHaveBeenLastCalledWith('App.Billing');
  });

  it('应对 element = null 的判据只渲文本、点击零 onFocus', async () => {
    const { onFocus } = renderPanel({
      findings: [finding({ element: null, category: 'contract', message: '无归属判据' })],
    });
    await waitFor(() => expect(itemsIn('findings')).toHaveLength(1));

    const row = itemsIn('findings')[0]!;
    expect(row.textContent).toContain('无归属判据');
    expect(within(row).queryByRole('button')).toBeNull();
    fireEvent.click(row);
    expect(onFocus).not.toHaveBeenCalled();
  });

  it('应在生成物条目「打开」时经 FileViewer 触达 read_file（路径由 root 与 repo 相对串拼出）', async () => {
    serve(apiCheck({ mismatched: ['crates/core/archi/src/lib.i.rs'] }));
    renderPanel();
    await waitFor(() => expect(itemsIn('artifacts')).toHaveLength(1));

    fireEvent.click(within(itemsIn('artifacts')[0]!).getByRole('button', { name: '打开' }));
    await waitFor(() => expect(callsOf('read_file')).toHaveLength(1));
    const payload = callsOf('read_file')[0]!;
    expect(payload.path).toBe('/ws/crates/core/archi/src/lib.i.rs');
    expect(payload.offset).toBeNull();
    expect(payload.limit).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 五态
// ---------------------------------------------------------------------------

describe('ContractPanel：状态矩阵', () => {
  it('应在加载态渲「解析中…」且两轴零条目、检查与生成双禁用', async () => {
    invokeMock.mockImplementation(() => new Promise(() => {}));
    renderPanel();
    await waitFor(() => expect(callsOf('archi_api_check')).toHaveLength(1));

    expect(screen.getByTestId('archi-contract-status').textContent).toContain('解析中');
    expect(itemsIn('artifacts')).toHaveLength(0);
    expect(itemsIn('findings')).toHaveLength(0);
    expect((screen.getByTestId('archi-contract-check') as HTMLButtonElement).disabled).toBe(true);
    // 生成入口在场但禁用（载入期不可生成，MUST NOT 缺席）
    expect((screen.getByTestId('archi-contract-generate') as HTMLButtonElement).disabled).toBe(
      true,
    );
  });

  it('应在 enabled = false 的冷启动仍列出 missing 一条（不渲成空面板）', async () => {
    serve(apiCheck({ enabled: false, missing: ['INDEX.toml'] }));
    renderPanel();
    await waitFor(() => expect(itemsIn('artifacts')).toHaveLength(1));

    const status = screen.getByTestId('archi-contract-status').textContent ?? '';
    expect(status).toContain('契约面未启用');
    expect(status).toContain('INDEX.toml');
    expect(itemsIn('artifacts')[0]?.getAttribute('data-kind')).toBe('missing');
  });

  it('应在零差异零判据时渲「生成物最新，判据 0 条」且零 chip 零条目', async () => {
    renderPanel();
    await waitFor(() =>
      expect(screen.getByTestId('archi-contract-status').textContent).toContain(
        '生成物最新，判据 0 条',
      ),
    );
    expect(itemsIn('artifacts')).toHaveLength(0);
    expect(itemsIn('findings')).toHaveLength(0);
    expect(within(axis('findings')).queryAllByTestId('archi-contract-chip')).toHaveLength(0);
  });

  it('应在检查失败时渲行内错误且不渲状态词（不谎称「解析中…」）', async () => {
    serve(new Error('root 无效'));
    renderPanel();
    await waitFor(() => expect(screen.getByTestId('archi-contract-error')).toBeTruthy());

    const error = screen.getByTestId('archi-contract-error');
    expect(error.getAttribute('data-source')).toBe('check');
    expect(error.textContent).toContain('root 无效');
    expect(screen.queryByTestId('archi-contract-status')).toBeNull();

    // 重试：成功后错误消失
    serve(apiCheck({ mismatched: ['m.i.rs'] }));
    fireEvent.click(screen.getByTestId('archi-contract-check'));
    await waitFor(() => expect(callsOf('archi_api_check')).toHaveLength(2));
    await waitFor(() => expect(screen.queryByTestId('archi-contract-error')).toBeNull());
    await waitFor(() => expect(itemsIn('artifacts')).toHaveLength(1));
  });

  it('应在首帧未载入态渲「解析中…」且 error 非空时同一组合下不渲状态词', async () => {
    // 形态一：`root !== null` + `check === null` + `loading === false` + `error === null`
    // （effect 未跑的首帧）。静态渲染不跑 effect，得到的正是这一组合 ⇒ 判定序里
    // `loading` 判据含 `check === null`，状态词仍为「解析中…」，两轴照常挂载（不渲成空面板）。
    const markup = renderToStaticMarkup(
      <ContractPanel root={ROOT} mode="incremental" findings={[]} onFocus={vi.fn()} />,
    );
    expect(markup).toContain('data-testid="archi-contract-status"');
    expect(markup).toContain('解析中');
    expect(markup).toContain('data-testid="archi-contract-axis-artifacts"');
    expect(markup).toContain('data-testid="archi-contract-axis-findings"');
    expect(markup).not.toContain('data-testid="archi-contract-error"');
    // `meta` 头部行只在 `check !== null` 时挂载 ⇒ 其缺席即「未载入」组合的形态侧证
    expect(markup).not.toContain('映射');
    expect(invokeMock).not.toHaveBeenCalled();

    // 形态二（对照臂）：同一「`check === null` + `loading === false`」组合下 `error !== null`
    // ⇒ 状态词不渲，成因只由错误块承载（判定序在 `error !== null` 处短路）。
    serve(new Error('模型结构错'));
    renderPanel();
    await waitFor(() => expect(screen.getByTestId('archi-contract-error')).toBeTruthy());
    expect(screen.queryByTestId('archi-contract-status')).toBeNull();
    expect(screen.getByTestId('archi-contract-error').textContent).toContain('模型结构错');
  });

  it('应在生成失败时顶部回显且断言成因与检查错误分挂不同 data-source', async () => {
    renderPanel();
    await waitFor(() =>
      expect(screen.getByTestId('archi-contract-status').textContent).toContain('生成物最新'),
    );

    invokeMock.mockImplementation((command: string) => {
      if (command === 'archi_api_check') return Promise.resolve(apiCheck());
      if (command === 'archi_api_generate') {
        return Promise.reject(new Error('写入失败（已写 2 份，不回滚）'));
      }
      return Promise.reject(new Error(`未预期命令: ${command}`));
    });

    fireEvent.click(screen.getByTestId('archi-contract-generate'));
    await waitFor(() => expect(screen.getByRole('button', { name: '提交' })).toBeTruthy());
    fireEvent.click(screen.getByRole('button', { name: '提交' }));
    await waitFor(() => expect(screen.getByTestId('archi-contract-error')).toBeTruthy());

    const error = screen.getByTestId('archi-contract-error');
    expect(error.getAttribute('data-source')).toBe('generate');
    expect(error.textContent).toContain('写入失败（已写 2 份，不回滚）');
    expect(error.textContent).not.toContain('全部失败');
    // 状态词仍由 check 派生（写盘失败不改写状态词）
    expect(screen.getByTestId('archi-contract-status').textContent).toContain('生成物最新');
    // 顶部：错误块在生成物差异轴之前
    const body = screen.getByTestId('archi-contract-panel');
    const positionError = Array.from(body.querySelectorAll('[data-testid]')).indexOf(error);
    const positionAxis = Array.from(body.querySelectorAll('[data-testid]')).indexOf(
      axis('artifacts'),
    );
    expect(positionError).toBeGreaterThanOrEqual(0);
    expect(positionError).toBeLessThan(positionAxis);
  });

  it('应在 root = null 时渲「无工作区根」且零 invoke 零条目', async () => {
    renderPanel({ root: null });
    await waitFor(() =>
      expect(screen.getByTestId('archi-contract-status').textContent).toContain('无工作区根'),
    );
    expect(invokeMock).not.toHaveBeenCalled();
    expect(itemsIn('artifacts')).toHaveLength(0);
    expect(itemsIn('findings')).toHaveLength(0);
    // 生成入口不可用（无根即无从生成），以禁用态而非缺席呈现
    expect((screen.getByTestId('archi-contract-generate') as HTMLButtonElement).disabled).toBe(
      true,
    );
  });
});

// ---------------------------------------------------------------------------
// 生成交互与模式权限
// ---------------------------------------------------------------------------

describe('ContractPanel：生成交互', () => {
  it('应在 incremental 下渲染可用的「生成」且零只读提示', async () => {
    renderPanel({ mode: 'incremental' });
    await waitFor(() =>
      expect((screen.getByTestId('archi-contract-generate') as HTMLButtonElement).disabled).toBe(
        false,
      ),
    );
    expect(screen.queryByTestId('archi-contract-readonly')).toBeNull();
    expect(screen.getByTestId('archi-contract-check')).toBeTruthy();
  });

  it('应在 full 下零生成入口且出一行只读提示', async () => {
    renderPanel({ mode: 'full' });
    await waitFor(() => expect(screen.getByTestId('archi-contract-check')).toBeTruthy());

    expect(screen.queryByTestId('archi-contract-generate')).toBeNull();
    const readonly = screen.getByTestId('archi-contract-readonly').textContent ?? '';
    expect(readonly).toContain('生成需开 change');
    expect(readonly).toContain('pnpm api:export');
    expect(callsOf('archi_api_generate')).toHaveLength(0);
  });

  it('应走二次确认 → 写盘一次 → toast → 复检一次（对话框三件文案齐备）', async () => {
    serve(
      apiCheck({
        mismatched: ['m1.i.rs', 'm2.i.rs'],
        missing: ['x.schema.json'],
        orphaned: ['o1.i.ts', 'o2.i.ts', 'o3.i.ts', 'o4.i.ts'],
      }),
    );
    renderPanel();
    await waitFor(() => expect(itemsIn('artifacts')).toHaveLength(7));

    fireEvent.click(screen.getByTestId('archi-contract-generate'));
    await waitFor(() => expect(screen.getByRole('button', { name: '提交' })).toBeTruthy());
    const dialogText = document.body.textContent ?? '';
    expect(dialogText).toContain('将写入 3 份');
    expect(dialogText).toContain('不符 2');
    expect(dialogText).toContain('缺失 1');
    expect(dialogText).toContain('保留 4 份孤儿生成物不做处理');
    expect(dialogText).toContain('写盘非原子');

    invokeMock.mockImplementation((command: string) => {
      if (command === 'archi_api_check') return Promise.resolve(apiCheck());
      if (command === 'archi_api_generate') {
        return Promise.resolve({ written: ['m1.i.rs', 'm2.i.rs', 'x.schema.json'], unchanged: [] });
      }
      return Promise.reject(new Error(`未预期命令: ${command}`));
    });
    fireEvent.click(screen.getByRole('button', { name: '提交' }));

    await waitFor(() => expect(callsOf('archi_api_generate')).toHaveLength(1));
    expect(callsOf('archi_api_generate')[0]?.root).toBe(ROOT);
    await waitFor(() => expect(callsOf('archi_api_check')).toHaveLength(2));
    expect(callsOf('archi_api_check')[1]?.root).toBe(ROOT);
    await waitFor(() =>
      expect(document.querySelector('[data-sonner-toast]')?.textContent ?? '').toContain(
        '写入 3 份',
      ),
    );
    expect(document.querySelector('[data-sonner-toast]')?.textContent ?? '').toContain('未变 0 份');
    // 复检清空后回到成功态
    await waitFor(() =>
      expect(screen.getByTestId('archi-contract-status').textContent).toContain('生成物最新'),
    );
  });

  it('应在生成进行中禁用两钮且连点不产生第二次写盘', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'archi_api_check')
        return Promise.resolve(apiCheck({ mismatched: ['m.i.rs'] }));
      if (command === 'archi_api_generate') return new Promise(() => {});
      return Promise.reject(new Error(`未预期命令: ${command}`));
    });
    renderPanel();
    await waitFor(() => expect(itemsIn('artifacts')).toHaveLength(1));

    fireEvent.click(screen.getByTestId('archi-contract-generate'));
    await waitFor(() => expect(screen.getByRole('button', { name: '提交' })).toBeTruthy());
    fireEvent.click(screen.getByRole('button', { name: '提交' }));
    await waitFor(() => expect(callsOf('archi_api_generate')).toHaveLength(1));

    await waitFor(() =>
      expect((screen.getByTestId('archi-contract-generate') as HTMLButtonElement).disabled).toBe(
        true,
      ),
    );
    expect((screen.getByTestId('archi-contract-check') as HTMLButtonElement).disabled).toBe(true);

    fireEvent.click(screen.getByTestId('archi-contract-generate'));
    fireEvent.click(screen.getByTestId('archi-contract-check'));
    expect(callsOf('archi_api_generate')).toHaveLength(1);
    expect(callsOf('archi_api_check')).toHaveLength(1);
  });
});

// ---------------------------------------------------------------------------
// 头部载元 / 越签名脏值 / 零第二份取数
// ---------------------------------------------------------------------------

describe('ContractPanel：头部与边界', () => {
  it('应在头部呈现工具版本与映射摘要前 12 位（不渲 64 位全量）', async () => {
    renderPanel();
    await waitFor(() => expect(screen.getByTestId('archi-contract-check')).toBeTruthy());
    await waitFor(() => expect(document.body.textContent).toContain('0.1.0'));

    const text = document.body.textContent ?? '';
    expect(text).toContain('bbbbbbbbbbbb');
    expect(text).not.toContain('b'.repeat(64));
  });

  it('应在 1000 条判据与超长路径下不抛错且逐字保留', async () => {
    const longPath = `${'x'.repeat(1200)}/契约🎯.i.rs`;
    serve(apiCheck({ mismatched: [longPath] }));
    renderPanel({
      findings: Array.from({ length: 1000 }, (_, index) => finding({ element: `App.E${index}` })),
    });

    await waitFor(() => expect(itemsIn('findings')).toHaveLength(1000));
    expect(itemsIn('artifacts')).toHaveLength(1);
    expect(itemsIn('artifacts')[0]?.textContent).toContain('契约🎯.i.rs');
    expect(itemsIn('artifacts')[0]?.textContent).toContain(longPath);
  });

  it('应只取 archi_api_check（零 archi_graph / archi_module / archi_drift）', async () => {
    renderPanel({ findings: [finding()] });
    await waitFor(() => expect(itemsIn('findings')).toHaveLength(1));

    const commandsCalled = invokeMock.mock.calls.map(([name]) => name as string);
    expect(commandsCalled).toEqual(['archi_api_check']);
    for (const command of ['archi_graph', 'archi_module', 'archi_drift', 'archi_file_tree']) {
      expect(commandsCalled).not.toContain(command);
    }
  });
});

// ---------------------------------------------------------------------------
// 零第二份取数（源码自证：切到契约视图不重取三既有命令）
// ---------------------------------------------------------------------------

describe('ContractPanel：零第二份取数（源码自证）', () => {
  /** 源文件全文（vitest 以包根为 cwd，`src/**` 为稳定相对面）。 */
  function sourceOf(relative: string): string {
    return readFileSync(resolve(process.cwd(), relative), 'utf8');
  }

  it('应零引用三既有 hook 与零定时器零 watch（取数只走 archi_api_check）', () => {
    const source = sourceOf('src/views/architecture/components/contract-panel.tsx');
    for (const hook of ['useArchiGraph', 'useArchiDrift', 'useArchiModule']) {
      expect(source.includes(hook), `contract-panel.tsx 零 ${hook}`).toBe(false);
    }
    for (const poller of ['setInterval', 'setTimeout', 'watchSubscribe']) {
      expect(source.includes(poller), `contract-panel.tsx 零 ${poller}`).toBe(false);
    }
    expect(source).toContain("from '../hooks/use-archi-api-check'");
  });

  it('应零前端类别常量表（chip 的类别与计数由 findings 派生）', () => {
    const source = sourceOf('src/views/architecture/components/contract-panel.tsx');
    for (const category of [
      'entry',
      'contract',
      'signature',
      'reexport',
      'nesting',
      'constructor',
    ]) {
      expect(source.includes(`'${category}'`), `contract-panel.tsx 零类别常量 ${category}`).toBe(
        false,
      );
    }
    expect(source).toContain('new Map<string, number>()');
  });
});
