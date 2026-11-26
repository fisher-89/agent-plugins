import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { renderToStaticMarkup } from 'react-dom/server';
import { toast } from 'sonner';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { Toaster } from '@/components/ui/sonner';
import type { ArchiApiCheck, ArchiApiFinding } from '@/types/generated/bindings';

import { ContractPanel, type ContractPanelProps } from './contract-panel';

/**
 * `ContractPanel` 单测：两轴并列与各自
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
    // 生成成功经门面落库一次（旁路：本件不观测，回 null 使结果面 soft-report 自洽）
    if (command === 'notification_record') return Promise.resolve(null);
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

  it('应在七组各一条时渲七个 chip（序按类别名，零退役 kind 参与聚合）', async () => {
    const categories = [
      'constructor',
      'contract',
      'doc',
      'entry',
      'nesting',
      'reexport',
      'signature',
    ];
    renderPanel({
      findings: categories.map((category) => finding({ category })),
    });
    await waitFor(() => expect(itemsIn('findings')).toHaveLength(7));

    const chips = within(axis('findings')).queryAllByTestId('archi-contract-chip');
    expect(chips.map((chip) => chip.getAttribute('data-category'))).toEqual(categories);
    expect(chips.map((chip) => chip.textContent)).toEqual(categories.map((name) => `${name} 1`));
  });

  it('应在单组 38 条 docMissing 时渲恰一个 doc chip 且计数 38', async () => {
    renderPanel({
      findings: Array.from({ length: 38 }, (_, index) =>
        finding({ category: 'doc', element: `App.E${index}`, kind: 'docMissing' }),
      ),
    });
    await waitFor(() => expect(itemsIn('findings')).toHaveLength(38));
    const chips = within(axis('findings')).queryAllByTestId('archi-contract-chip');
    expect(chips).toHaveLength(1);
    expect(chips[0]!.textContent).toBe('doc 38');
  });

  it('应在 findings = [] 时零 chip 零条目且不崩', async () => {
    renderPanel({ findings: [] });
    await waitFor(() =>
      expect(screen.getByTestId('archi-contract-status').textContent).toContain('判据 0 条'),
    );
    expect(within(axis('findings')).queryAllByTestId('archi-contract-chip')).toHaveLength(0);
    expect(itemsIn('findings')).toHaveLength(0);
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
    expect(status).toContain('接口面未启用');
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
    expect(readonly).toContain('或让左侧会话生成');
    expect(readonly).not.toContain('本仓的生成命令');
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
    const source = sourceOf('src/views/architecture/components/dashboard/contract-panel.tsx');
    for (const hook of ['useArchiGraph', 'useArchiDrift', 'useArchiModule']) {
      expect(source.includes(hook), `contract-panel.tsx 零 ${hook}`).toBe(false);
    }
    for (const poller of ['setInterval', 'setTimeout', 'watchSubscribe']) {
      expect(source.includes(poller), `contract-panel.tsx 零 ${poller}`).toBe(false);
    }
    expect(source).toContain("from '../../hooks/use-archi-api-check'");
  });

  it('应零前端类别常量表（chip 的类别与计数由 findings 派生）', () => {
    const source = sourceOf('src/views/architecture/components/dashboard/contract-panel.tsx');
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
    for (const retiredKind of ['apiReexportDrift', 'docNotFound', 'apiDocMissing']) {
      expect(
        source.includes(`'${retiredKind}'`),
        `contract-panel.tsx 零退役 kind 字面 ${retiredKind}`,
      ).toBe(false);
    }
    expect(source).toContain('new Map<string, number>()');
  });
});

// ---------------------------------------------------------------------------
// 通知门面旁路（desktop-notification-hub AC-2）：写契约成功的裸 toast 对位
// `notify('info', …)`——DOM 文案零改 + 落库一次（info 档、无副文案）
// ---------------------------------------------------------------------------

/** `notification_record` 的落库入参序列。 */
function recordCalls(): { level: string; message: string; body: string | null }[] {
  return callsOf('notification_record').length === 0
    ? []
    : invokeMock.mock.calls
        .filter(([command]) => command === 'notification_record')
        .map(
          ([, params]) =>
            (params as { record: { level: string; message: string; body: string | null } }).record,
        );
}

describe('ContractPanel：写契约成功的通知门面旁路', () => {
  /** 生成成功的分派面（落库可注入 reject）。 */
  function serveGenerate(recordReject: boolean): void {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'archi_api_check')
        return Promise.resolve(apiCheck({ mismatched: ['m.i.rs'] }));
      if (command === 'archi_api_generate') {
        return Promise.resolve({ written: ['m1.i.rs', 'm2.i.rs'], unchanged: [] });
      }
      if (command === 'read_file') return Promise.resolve(sliceOf('// 生成物\n'));
      if (command === 'notification_record') {
        return recordReject ? Promise.reject(new Error('落库失败')) : Promise.resolve(null);
      }
      return Promise.reject(new Error(`未预期命令: ${command}`));
    });
  }

  async function generateOnce(): Promise<void> {
    renderPanel();
    await waitFor(() => expect(itemsIn('artifacts')).toHaveLength(1));
    fireEvent.click(screen.getByTestId('archi-contract-generate'));
    await waitFor(() => expect(screen.getByRole('button', { name: '提交' })).toBeTruthy());
    fireEvent.click(screen.getByRole('button', { name: '提交' }));
    await waitFor(() => expect(callsOf('archi_api_generate')).toHaveLength(1));
  }

  it('正向：生成成功 → DOM 文案不变且落库一次（info 档、无副文案）', async () => {
    serveGenerate(false);
    await generateOnce();

    await waitFor(() => expect(recordCalls()).toHaveLength(1));
    expect(recordCalls()[0].level).toBe('info');
    expect(recordCalls()[0].body).toBeNull();
    expect(recordCalls()[0].message).toContain('写入 2 份');

    // 真实 sonner：既有 DOM 文案断言零改
    await waitFor(() =>
      expect(document.querySelector('[data-sonner-toast]')?.textContent ?? '').toContain(
        '写入 2 份',
      ),
    );
  });

  it('边界：生成失败走 inline 错误态，零落库（失败不经提示轨）', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'archi_api_check')
        return Promise.resolve(apiCheck({ mismatched: ['m.i.rs'] }));
      if (command === 'archi_api_generate') {
        return Promise.reject(new Error('写入失败（已写 2 份，不回滚）'));
      }
      if (command === 'read_file') return Promise.resolve(sliceOf('// 生成物\n'));
      if (command === 'notification_record') return Promise.resolve(null);
      return Promise.reject(new Error(`未预期命令: ${command}`));
    });
    renderPanel();
    await waitFor(() => expect(itemsIn('artifacts')).toHaveLength(1));
    fireEvent.click(screen.getByTestId('archi-contract-generate'));
    await waitFor(() => expect(screen.getByRole('button', { name: '提交' })).toBeTruthy());
    fireEvent.click(screen.getByRole('button', { name: '提交' }));

    await waitFor(() =>
      expect(screen.getByTestId('archi-contract-error').textContent).toContain('写入失败'),
    );
    expect(recordCalls()).toHaveLength(0);
  });

  it('异常：落库 reject 时 DOM toast 文案断言仍成立、面板状态不变', async () => {
    serveGenerate(true);
    await generateOnce();

    expect(recordCalls()).toHaveLength(1);
    await waitFor(() =>
      expect(document.querySelector('[data-sonner-toast]')?.textContent ?? '').toContain(
        '写入 2 份',
      ),
    );
    // 面板状态零变化：复检后生成物轴仍是同一条不符、零错误块（落库失败是旁路）
    expect(itemsIn('artifacts')).toHaveLength(1);
    expect(screen.queryByTestId('archi-contract-error')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 本变更：`full` 模式的只读提示文案（生成能力归会话后不再承诺本仓命令）
// ---------------------------------------------------------------------------

describe('ContractPanel：只读提示文案', () => {
  it('应在 full 模式下提示先开 change 或由会话生成', async () => {
    renderPanel({ mode: 'full' });
    await waitFor(() => expect(screen.getByTestId('archi-contract-check')).toBeTruthy());

    const readonly = screen.getByTestId('archi-contract-readonly');
    expect(readonly.textContent).toContain('生成需开 change');
    expect(readonly.textContent).toContain('或让左侧会话生成');
    // 旧死指代（本仓命令名）零残留
    expect(readonly.textContent).not.toContain('本仓的生成命令');
    expect(readonly.textContent).not.toContain('生成命令');
  });

  it('应在文案内零命令名与路径字面且保持一行短句', async () => {
    renderPanel({ mode: 'full' });
    await waitFor(() => expect(screen.getByTestId('archi-contract-readonly')).toBeTruthy());

    const readonly = screen.getByTestId('archi-contract-readonly');
    const text = readonly.textContent ?? '';
    for (const literal of ['api:export', 'pnpm', 'cargo', 'bindings', '.dev-team', 'archi_']) {
      expect(text).not.toContain(literal);
    }
    // 一行短句：无内联元素、无换行（单一文本节点）
    expect(readonly.children).toHaveLength(0);
    expect(text).not.toContain('\n');
    expect(text.length).toBeLessThanOrEqual(20);

    // 位次与类名零改动：仍是 header 之后、meta 行之前的首个提示位
    const panel = screen.getByTestId('archi-contract-panel');
    const ordered = Array.from(panel.querySelectorAll('[data-testid]'));
    expect(ordered.indexOf(readonly)).toBeGreaterThanOrEqual(0);
    expect(readonly.className).toContain('text-[11px]');
    expect(readonly.className).toContain('text-muted-foreground');

    // 旧串片段零命中（整页文本）
    expect(document.body.textContent ?? '').not.toContain('本仓的生成命令');
    expect(document.body.textContent ?? '').not.toContain('执行本仓');
  });

  it('应在 incremental 模式下零只读提示且生成入口可用', async () => {
    renderPanel({ mode: 'incremental' });
    await waitFor(() =>
      expect((screen.getByTestId('archi-contract-generate') as HTMLButtonElement).disabled).toBe(
        false,
      ),
    );

    expect(screen.queryByTestId('archi-contract-readonly')).toBeNull();
  });

  it('应在 full 模式下零生成按钮且零生成调用（入口契约不因文案改写）', async () => {
    renderPanel({ mode: 'full' });
    await waitFor(() => expect(screen.getByTestId('archi-contract-readonly')).toBeTruthy());

    expect(screen.queryByTestId('archi-contract-generate')).toBeNull();
    expect(callsOf('archi_api_generate')).toHaveLength(0);
    // 复检入口仍在（只读模式不是死面板）
    expect(screen.getByTestId('archi-contract-check')).toBeTruthy();
  });

  it('应在 check 未就绪时仍渲只读提示且无额外第二行', async () => {
    serve(new Error('检查失败：网络不可达'));
    renderPanel({ mode: 'full' });

    await waitFor(() =>
      expect(screen.getByTestId('archi-contract-error').getAttribute('data-source')).toBe('check'),
    );
    const readonly = screen.getByTestId('archi-contract-readonly');
    expect(readonly.textContent).toBe('生成需开 change，或让左侧会话生成');
    expect(screen.getAllByTestId('archi-contract-readonly')).toHaveLength(1);
  });

  it('应在源码面为 full 分支零生成调用（文案行不含任何动作）', () => {
    const source = readFileSync(
      resolve(process.cwd(), 'src/views/architecture/components/dashboard/contract-panel.tsx'),
      'utf8',
    );
    const marker = 'data-testid="archi-contract-readonly"';
    const start = source.indexOf(marker);
    expect(start).toBeGreaterThanOrEqual(0);
    // 只取该 `div` 的元素与文本窗口（属性起点之后 120 字符），不含上方 header 的 props
    const block = source.slice(start, start + 120);
    expect(block).not.toContain('archiApiGenerate');
    expect(block).not.toContain('onGenerate');
    expect(block).not.toContain('onClick');
  });
});

// ---------------------------------------------------------------------------
// 两向断言：每个判据两侧都钉住（只钉一侧会放过另一半突变体）——状态词 / 条目文案 /
// chip 选中态 / 生成钮复原 / chip 计数随动
// ---------------------------------------------------------------------------

describe('ContractPanel：状态词两向', () => {
  it('应在三类差异非零且 findings 空时零状态词（合计非零不得误判为最新）', async () => {
    serve(
      apiCheck({
        mismatched: ['m.i.rs'],
        orphaned: ['o.i.ts'],
        missing: ['x.schema.json', 'y.schema.json'],
      }),
    );
    renderPanel({ findings: [] });
    await waitFor(() => expect(itemsIn('artifacts')).toHaveLength(4));

    expect(axis('artifacts').textContent).toContain('生成物差异（4）');
    expect(screen.queryByTestId('archi-contract-status')).toBeNull();
  });

  it('应在孤儿数多于两列之和时仍零状态词（合计对每一列敏感）', async () => {
    serve(
      apiCheck({
        mismatched: ['m.i.rs'],
        orphaned: ['o1.i.ts', 'o2.i.ts', 'o3.i.ts'],
        missing: ['x.schema.json', 'y.schema.json'],
      }),
    );
    renderPanel({ findings: [] });
    await waitFor(() => expect(itemsIn('artifacts')).toHaveLength(6));

    expect(axis('artifacts').textContent).toContain('生成物差异（6）');
    expect(screen.queryByTestId('archi-contract-status')).toBeNull();
  });

  it('应在差异全 0 而判据非空时零状态词（不得谎称判据 0 条）', async () => {
    renderPanel({ findings: [finding()] });
    await waitFor(() => expect(itemsIn('findings')).toHaveLength(1));

    expect(axis('artifacts').textContent).toContain('生成物差异（0）');
    expect(screen.queryByTestId('archi-contract-status')).toBeNull();
  });
});

describe('ContractPanel：判据条目文案两向', () => {
  it('应在 element / target 四组合下逐字呈现（缺项不留分隔符、不渲 null）', async () => {
    renderPanel({
      findings: [
        finding({ element: 'App.Auth', target: 'login' }),
        finding({ element: null, target: 'login' }),
        finding({ element: 'App.Auth', target: null }),
        finding({ element: null, target: null }),
      ],
    });
    await waitFor(() => expect(itemsIn('findings')).toHaveLength(4));

    expect(itemsIn('findings').map((row) => row.firstElementChild?.textContent)).toEqual([
      'apiSignatureMismatch · App.Auth · login',
      'apiSignatureMismatch · login',
      'apiSignatureMismatch · App.Auth',
      'apiSignatureMismatch',
    ]);
  });

  it('应在 element 非空时带 cursor-pointer、为 null 时零交互类（类名逐 token 对拍）', async () => {
    renderPanel({
      findings: [finding({ element: 'App.Auth' }), finding({ element: null })],
    });
    await waitFor(() => expect(itemsIn('findings')).toHaveLength(2));

    const [withElement, withoutElement] = itemsIn('findings');
    const tokensOf = (row: HTMLElement | undefined): string[] =>
      (row?.className ?? '').split(/\s+/).filter((token) => token.length > 0);

    expect(tokensOf(withElement)).toEqual([
      'flex',
      'flex-col',
      'gap-0.5',
      'rounded-md',
      'px-1',
      'py-0.5',
      'cursor-pointer',
      'hover:bg-muted',
    ]);
    expect(tokensOf(withoutElement)).toEqual([
      'flex',
      'flex-col',
      'gap-0.5',
      'rounded-md',
      'px-1',
      'py-0.5',
    ]);
    expect(tokensOf(withoutElement)).not.toContain('cursor-pointer');
  });
});

describe('ContractPanel：chip 选中态两向', () => {
  function chips(): HTMLElement[] {
    return within(axis('findings')).queryAllByTestId('archi-contract-chip');
  }

  function chipOf(category: string): HTMLElement {
    return chips().find((chip) => chip.getAttribute('data-category') === category)!;
  }

  it('应未选中片 aria-pressed=false 且 border-transparent、选中片两者反转', async () => {
    renderPanel({
      findings: [finding({ category: 'entry' }), finding({ category: 'signature' })],
    });
    await waitFor(() => expect(itemsIn('findings')).toHaveLength(2));

    for (const category of ['entry', 'signature']) {
      expect(chipOf(category).getAttribute('aria-pressed')).toBe('false');
      expect(chipOf(category).className).toContain('border-transparent');
      expect(chipOf(category).className).not.toContain('border-primary/40');
    }

    fireEvent.click(chipOf('entry'));
    await waitFor(() => expect(itemsIn('findings')).toHaveLength(1));

    expect(chipOf('entry').getAttribute('aria-pressed')).toBe('true');
    expect(chipOf('entry').className).toContain('border-primary/40');
    expect(chipOf('entry').className).not.toContain('border-transparent');
    // 未选中片仍保持未选中形态（选中态只落一片）
    expect(chipOf('signature').getAttribute('aria-pressed')).toBe('false');
    expect(chipOf('signature').className).toContain('border-transparent');
    expect(chipOf('signature').className).not.toContain('border-primary/40');
  });

  it('应在 findings = [] 时渲「无契约判据」占位且零 chip', async () => {
    renderPanel({ findings: [] });
    await waitFor(() =>
      expect(screen.getByTestId('archi-contract-status').textContent).toContain('判据 0 条'),
    );

    expect(chips()).toHaveLength(0);
    expect(axis('findings').textContent).toContain('无契约判据');
  });

  it('应在判据非空时零「无契约判据」占位（占位与列表互斥）', async () => {
    renderPanel({ findings: [finding()] });
    await waitFor(() => expect(itemsIn('findings')).toHaveLength(1));

    expect(axis('findings').textContent).not.toContain('无契约判据');
  });
});

describe('ContractPanel：生成钮可用性两向', () => {
  async function generateOnce(): Promise<void> {
    fireEvent.click(screen.getByTestId('archi-contract-generate'));
    await waitFor(() => expect(screen.getByRole('button', { name: '提交' })).toBeTruthy());
    fireEvent.click(screen.getByRole('button', { name: '提交' }));
    await waitFor(() => expect(callsOf('archi_api_generate')).toHaveLength(1));
  }

  it('应在生成成功后「生成」与「检查」双钮复原可点（忙态只属于进行中）', async () => {
    serve(apiCheck({ mismatched: ['m.i.rs'] }));
    renderPanel();
    await waitFor(() => expect(itemsIn('artifacts')).toHaveLength(1));

    await generateOnce();

    await waitFor(() =>
      expect((screen.getByTestId('archi-contract-generate') as HTMLButtonElement).disabled).toBe(
        false,
      ),
    );
    await waitFor(() =>
      expect((screen.getByTestId('archi-contract-check') as HTMLButtonElement).disabled).toBe(
        false,
      ),
    );
    // 复检仍回同一条差异 ⇒ 钮可用性与「结果是否为空」无关
    expect(itemsIn('artifacts')).toHaveLength(1);
  });

  it('应在生成失败后「生成」钮复原可点且错误块在场（失败不永久禁用）', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'archi_api_check')
        return Promise.resolve(apiCheck({ mismatched: ['m.i.rs'] }));
      if (command === 'archi_api_generate') {
        return Promise.reject(new Error('写入失败（已写 1 份，不回滚）'));
      }
      return Promise.reject(new Error(`未预期命令: ${command}`));
    });
    renderPanel();
    await waitFor(() => expect(itemsIn('artifacts')).toHaveLength(1));

    await generateOnce();

    await waitFor(() => expect(screen.getByTestId('archi-contract-error')).toBeTruthy());
    await waitFor(() =>
      expect((screen.getByTestId('archi-contract-generate') as HTMLButtonElement).disabled).toBe(
        false,
      ),
    );
    expect((screen.getByTestId('archi-contract-check') as HTMLButtonElement).disabled).toBe(false);
  });
});

describe('ContractPanel：chip 计数随 findings 入参随动', () => {
  function chipsText(): (string | null)[] {
    return within(axis('findings'))
      .queryAllByTestId('archi-contract-chip')
      .map((chip) => chip.textContent);
  }

  it('应在 findings 换新后 chip 计数随之重算（派生面依赖入参而非常量）', async () => {
    const onFocus = vi.fn();
    const { rerender } = render(
      <ContractPanel
        root={ROOT}
        mode="incremental"
        findings={[finding({ category: 'entry' })]}
        onFocus={onFocus}
      />,
    );
    await waitFor(() => expect(itemsIn('findings')).toHaveLength(1));
    expect(chipsText()).toEqual(['entry 1']);

    rerender(
      <ContractPanel
        root={ROOT}
        mode="incremental"
        findings={[
          finding({ category: 'entry' }),
          finding({ category: 'signature' }),
          finding({ category: 'signature' }),
        ]}
        onFocus={onFocus}
      />,
    );
    await waitFor(() => expect(itemsIn('findings')).toHaveLength(3));
    expect(chipsText()).toEqual(['entry 1', 'signature 2']);

    rerender(<ContractPanel root={ROOT} mode="incremental" findings={[]} onFocus={onFocus} />);
    await waitFor(() => expect(itemsIn('findings')).toHaveLength(0));
    expect(chipsText()).toEqual([]);
  });
});
