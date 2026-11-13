import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ArchiElement, ArchiRelation } from '../../../types/generated/bindings';
import { ElementDrawer } from './element-drawer';

/**
 * `ElementDrawer` 单测（AC-6 / AC-18 / AC-19 / D22）：概览字段与声明位置、文档
 * 四字段条目按强度序、ADR 正文按需取（展开才 invoke）与行内错误、`incremental`
 * 改档（三档值 / 成功回调 / 失败行内错误且不触发重取）、`full` 只读提示、零「相邻
 * 报告」区块。
 *
 * 进程边界 mock：`@tauri-apps/api/core` 的 `invoke`（`archi_module` /
 * `read_file` / `archi_set_gate` 按命令名分发）；`onChanged` 以 `vi.fn()`
 * 观测（重取触发即被测语义）。
 */

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

function element(overrides: Partial<ArchiElement> = {}): ArchiElement {
  return {
    fqn: 'App.Auth',
    kind: 'domain',
    gate: 'strict',
    api: 'declared',
    path: ['./src/auth/', './src/token/'],
    summary: '认证域',
    metadata: { owner: 'core' },
    modelFile: '.dev-team/architecture/models/01-core.toml',
    modelLine: 6,
    mark: 'none',
    ...overrides,
  };
}

function relation(overrides: Partial<ArchiRelation> = {}): ArchiRelation {
  return {
    from: 'App.Auth',
    to: 'App.Billing',
    description: '认证调用计费',
    mark: 'none',
    reason: null,
    ...overrides,
  };
}

/** 文档条目 fixture（强度序：explicit → adjacent → text）。 */
function docsFixture() {
  return [
    {
      path: 'src/auth/README.md',
      title: '认证说明',
      kind: 'md' as const,
      source: 'explicit' as const,
    },
    {
      path: 'src/auth/guide.md',
      title: '指南',
      kind: 'md' as const,
      source: 'adjacent' as const,
    },
    {
      path: '.dev-team/architecture/decisions/0007-token-sink.md',
      title: '令牌收口',
      kind: 'adr' as const,
      source: 'text' as const,
    },
  ];
}

function callsOf(command: string): Record<string, unknown>[] {
  return invokeMock.mock.calls
    .filter(([name]) => name === command)
    .map(([, params]) => (params ?? {}) as Record<string, unknown>);
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
    if (command === 'archi_module') {
      const fqn = typeof params?.element === 'string' ? params.element : '';
      return Promise.resolve({
        element: element({ fqn }),
        dependsOn: [{ target: 'App.Billing', description: '认证调用计费' }],
        dependedBy: [],
        children: [],
        docs: docsFixture(),
        warnings: [],
        // 契约状态投影：既有用例不涉契约段，取「无契约」事实态（契约段呈一行空态）
        api: { status: 'none', contractPath: null, schemaPath: null, generatedPath: null },
      });
    }
    if (command === 'read_file') {
      const content = '# 令牌收口\n\n- 收口到 Token 模块\n\n```ts\nconst sink = 1;\n```\n';
      return Promise.resolve({
        content,
        stat: { size: content.length, modifiedMs: null, isFile: true, isDir: false },
        offset: 0,
        nextOffset: content.length,
        truncated: false,
        encoding: 'utf8',
      });
    }
    if (command === 'archi_set_gate') {
      return Promise.resolve(null);
    }
    return Promise.reject(new Error(`未预期命令: ${command}`));
  });
});

afterEach(() => {
  cleanup();
});

describe('抽屉概览', () => {
  it('应呈现 kind / path / gate / modelFile:line 与摘要', async () => {
    render(
      <ElementDrawer
        root={'C:\\demo\\alpha'}
        mode="full"
        element={element()}
        relations={[]}
        findings={[]}
        onChanged={vi.fn()}
      />,
    );
    await waitFor(() => expect(callsOf('archi_module')).toHaveLength(1));
    const drawer = screen.getByTestId('archi-drawer');
    expect(drawer.textContent).toContain('App.Auth');
    expect(drawer.textContent).toContain('kind domain · gate strict');
    expect(drawer.textContent).toContain('path ./src/auth/、./src/token/');
    expect(screen.getByTestId('archi-drawer-position').textContent).toBe(
      '.dev-team/architecture/models/01-core.toml:6',
    );
    expect(drawer.textContent).toContain('认证域');
  });

  it('应在行号未知（modelLine = 0）时只渲文件', async () => {
    render(
      <ElementDrawer
        root={'C:\\demo\\alpha'}
        mode="full"
        element={element({ modelLine: 0 })}
        relations={[]}
        findings={[]}
        onChanged={vi.fn()}
      />,
    );
    await waitFor(() => expect(callsOf('archi_module')).toHaveLength(1));
    expect(screen.getByTestId('archi-drawer-position').textContent).toBe(
      '.dev-team/architecture/models/01-core.toml',
    );
  });

  it('应在无 path 时呈现未声明占位（不出现空串）', async () => {
    render(
      <ElementDrawer
        root={'C:\\demo\\alpha'}
        mode="full"
        element={element({ path: [], summary: null })}
        relations={[]}
        findings={[]}
        onChanged={vi.fn()}
      />,
    );
    await waitFor(() => expect(callsOf('archi_module')).toHaveLength(1));
    expect(screen.getByTestId('archi-drawer').textContent).toContain('path （未声明）');
  });
});

describe('文档关联段', () => {
  it('应按强度序渲染四字段条目（路径经 title 属性透出，不含正文）', async () => {
    render(
      <ElementDrawer
        root={'C:\\demo\\alpha'}
        mode="full"
        element={element()}
        relations={[]}
        findings={[]}
        onChanged={vi.fn()}
      />,
    );
    await waitFor(() => expect(screen.queryAllByTestId('archi-doc-item')).toHaveLength(3));
    const items = screen.queryAllByTestId('archi-doc-item');
    expect(items.map((item) => item.getAttribute('data-source'))).toEqual([
      'explicit',
      'adjacent',
      'text',
    ]);
    expect(items[0]?.textContent).toContain('认证说明');
    expect(screen.getByTitle('src/auth/README.md')).toBeTruthy();
    expect(callsOf('read_file')).toHaveLength(0);
  });

  it('应在 docs 为空时呈现空态且不告警', async () => {
    invokeMock.mockImplementation(() =>
      Promise.resolve({
        element: element(),
        dependsOn: [],
        dependedBy: [],
        children: [],
        docs: [],
        warnings: [],
        api: { status: 'none', contractPath: null, schemaPath: null, generatedPath: null },
      }),
    );
    render(
      <ElementDrawer
        root={'C:\\demo\\alpha'}
        mode="full"
        element={element()}
        relations={[]}
        findings={[]}
        onChanged={vi.fn()}
      />,
    );
    await waitFor(() => expect(screen.getByTestId('archi-docs-empty')).toBeTruthy());
    expect(screen.queryAllByTestId('archi-doc-item')).toHaveLength(0);
    expect(screen.getByTestId('archi-docs-empty').textContent).toBe('无关联文档');
  });

  it('应在点击 ADR 条目时打开预览抽屉并取该路径正文，默认页签渲染 markdown（未点击零 invoke）', async () => {
    render(
      <ElementDrawer
        root={'C:\\demo\\alpha'}
        mode="full"
        element={element()}
        relations={[]}
        findings={[]}
        onChanged={vi.fn()}
      />,
    );
    await waitFor(() => expect(screen.queryAllByTestId('archi-doc-item')).toHaveLength(3));
    expect(callsOf('read_file')).toHaveLength(0);

    const adrButton = screen.getByTitle('.dev-team/architecture/decisions/0007-token-sink.md');
    fireEvent.click(adrButton);
    await waitFor(() => expect(screen.getByTestId('file-content-markdown')).toBeTruthy());
    expect(callsOf('read_file')).toEqual([
      {
        path: 'C:\\demo\\alpha/.dev-team/architecture/decisions/0007-token-sink.md',
        offset: null,
        limit: null,
      },
    ]);
    const body = screen.getByTestId('file-content-markdown');
    expect(body.textContent).toContain('令牌收口');
    expect(body.textContent).toContain('收口到 Token 模块');
    expect(body.querySelector('h1')).toBeTruthy();
    expect(body.querySelector('li')).toBeTruthy();
    expect(body.querySelector('code')).toBeTruthy();
  });

  it('应在 ADR 取数失败时抽屉内行内呈现错误且抽屉其余区块不变', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'archi_module') {
        return Promise.resolve({
          element: element(),
          dependsOn: [],
          dependedBy: [],
          children: [],
          docs: docsFixture(),
          warnings: [],
          // 契约状态投影：本组用例不涉契约段，取「无契约」事实态（契约段呈一行空态）
          api: { status: 'none', contractPath: null, schemaPath: null, generatedPath: null },
        });
      }
      return Promise.reject(new Error('路径不在模块文档关联集内：a.md'));
    });
    render(
      <ElementDrawer
        root={'C:\\demo\\alpha'}
        mode="full"
        element={element()}
        relations={[]}
        findings={[]}
        onChanged={vi.fn()}
      />,
    );
    await waitFor(() => expect(screen.queryAllByTestId('archi-doc-item')).toHaveLength(3));
    fireEvent.click(screen.getByTitle('.dev-team/architecture/decisions/0007-token-sink.md'));
    await waitFor(() => expect(screen.getByTestId('file-content-error')).toBeTruthy());
    expect(screen.getByTestId('file-content-error').textContent).toContain(
      '路径不在模块文档关联集内',
    );
    expect(screen.getByTestId('archi-drawer')).toBeTruthy();
  });
});

describe('incremental 改档', () => {
  it('应提供三档选择并在提交时调用 archiSetGate 且成功后触发父级重取', async () => {
    const onChanged = vi.fn();
    render(
      <ElementDrawer
        root={'C:\\demo\\alpha'}
        mode="incremental"
        element={element()}
        relations={[]}
        findings={[]}
        onChanged={onChanged}
      />,
    );
    await waitFor(() => expect(callsOf('archi_module')).toHaveLength(1));

    const select = screen.getByTestId('archi-gate-select') as HTMLSelectElement;
    expect(Array.from(select.options).map((option) => option.value)).toEqual([
      'off',
      'warn',
      'strict',
    ]);

    for (const tier of ['off', 'warn', 'strict']) {
      fireEvent.change(select, { target: { value: tier } });
      await act(async () => {
        fireEvent.click(screen.getByTestId('archi-gate-submit'));
      });
      await waitFor(() => {
        const calls = callsOf('archi_set_gate');
        expect(calls.at(-1)).toEqual({
          root: 'C:\\demo\\alpha',
          element: 'App.Auth',
          gate: tier,
        });
      });
    }
    expect(callsOf('archi_set_gate')).toHaveLength(3);
    expect(onChanged).toHaveBeenCalledTimes(3);
  });

  it('应在改档失败时行内呈现错误且不触发重取', async () => {
    const onChanged = vi.fn();
    invokeMock.mockImplementation((command: string) => {
      if (command === 'archi_module') {
        return Promise.resolve({
          element: element(),
          dependsOn: [],
          dependedBy: [],
          children: [],
          docs: [],
          warnings: [],
          api: { status: 'none', contractPath: null, schemaPath: null, generatedPath: null },
        });
      }
      return Promise.reject(new Error('改档渲染自证失败：回读档位为 off'));
    });
    render(
      <ElementDrawer
        root={'C:\\demo\\alpha'}
        mode="incremental"
        element={element()}
        relations={[]}
        findings={[]}
        onChanged={onChanged}
      />,
    );
    await waitFor(() => expect(callsOf('archi_module')).toHaveLength(1));
    await act(async () => {
      fireEvent.click(screen.getByTestId('archi-gate-submit'));
    });
    await waitFor(() => expect(screen.getByTestId('archi-gate-error')).toBeTruthy());
    expect(screen.getByTestId('archi-gate-error').textContent).toContain('改档渲染自证失败');
    expect(onChanged).not.toHaveBeenCalled();
  });
});

describe('full 只读与本期边界', () => {
  it('应在 full 模式下零可提交控件并给出开 change 提示', async () => {
    render(
      <ElementDrawer
        root={'C:\\demo\\alpha'}
        mode="full"
        element={element()}
        relations={[]}
        findings={[]}
        onChanged={vi.fn()}
      />,
    );
    await waitFor(() => expect(callsOf('archi_module')).toHaveLength(1));
    expect(screen.queryByTestId('archi-gate-select')).toBeNull();
    expect(screen.queryByTestId('archi-gate-submit')).toBeNull();
    expect(screen.getByTestId('archi-gate-readonly').textContent).toContain(
      '改档需要开一个 change',
    );
    expect(callsOf('archi_set_gate')).toHaveLength(0);
  });

  it('应无「相邻报告」区块与计数占位（本期射程外）', async () => {
    render(
      <ElementDrawer
        root={'C:\\demo\\alpha'}
        mode="full"
        element={element()}
        relations={[relation()]}
        findings={[]}
        onChanged={vi.fn()}
      />,
    );
    await waitFor(() => expect(callsOf('archi_module')).toHaveLength(1));
    const drawer = screen.getByTestId('archi-drawer');
    expect(drawer.textContent).not.toContain('相邻报告');
    expect(drawer.textContent).not.toContain('errors');
    expect(drawer.textContent).not.toContain('warnings');
  });

  it('应在豁免边存在时呈现 reason 行', async () => {
    render(
      <ElementDrawer
        root={'C:\\demo\\alpha'}
        mode="full"
        element={element()}
        relations={[relation({ mark: 'exempt', reason: '平台层特例' })]}
        findings={[]}
        onChanged={vi.fn()}
      />,
    );
    await waitFor(() => expect(callsOf('archi_module')).toHaveLength(1));
    expect(screen.getByTestId('archi-exempt-reason').textContent).toContain('平台层特例');
  });
});

describe('抽屉取数态与 gate 编辑边界', () => {
  it('应在 ADR 正文取数未回时呈加载行（不空白、不提前显错误）', async () => {
    let resolveDoc: ((body: string) => void) | undefined;
    invokeMock.mockImplementation((command: string) => {
      if (command === 'archi_module') {
        return Promise.resolve({
          element: element(),
          dependsOn: [],
          dependedBy: [],
          children: [],
          docs: docsFixture(),
          warnings: [],
          // 契约状态投影：本组用例不涉契约段，取「无契约」事实态（契约段呈一行空态）
          api: { status: 'none', contractPath: null, schemaPath: null, generatedPath: null },
        });
      }
      return new Promise<string>((resolve) => {
        resolveDoc = resolve;
      });
    });
    render(
      <ElementDrawer
        root={'C:\\demo\\alpha'}
        mode="full"
        element={element()}
        relations={[]}
        findings={[]}
        onChanged={vi.fn()}
      />,
    );
    await waitFor(() => expect(screen.queryAllByTestId('archi-doc-item')).toHaveLength(3));

    fireEvent.click(screen.getByTitle('.dev-team/architecture/decisions/0007-token-sink.md'));
    await waitFor(() => expect(screen.getByTestId('file-content-loading')).toBeTruthy());
    expect(screen.queryByTestId('file-content-markdown')).toBeNull();
    expect(screen.queryByTestId('file-content-error')).toBeNull();

    await act(async () => {
      resolveDoc?.('# 令牌收口\n');
    });
    await waitFor(() => expect(screen.getByTestId('file-content-markdown')).toBeTruthy());
    expect(screen.queryByTestId('file-content-loading')).toBeNull();
  });

  it('应在 gate 提交未回时禁用写回按钮，落定后恢复可提交', async () => {
    let resolveGate: (() => void) | undefined;
    invokeMock.mockImplementation((command: string) => {
      if (command === 'archi_module') {
        return Promise.resolve({
          element: element(),
          dependsOn: [],
          dependedBy: [],
          children: [],
          docs: [],
          warnings: [],
          api: { status: 'none', contractPath: null, schemaPath: null, generatedPath: null },
        });
      }
      return new Promise<null>((resolve) => {
        resolveGate = () => resolve(null);
      });
    });
    render(
      <ElementDrawer
        root={'C:\\demo\\alpha'}
        mode="incremental"
        element={element()}
        relations={[]}
        findings={[]}
        onChanged={vi.fn()}
      />,
    );
    await waitFor(() => expect(screen.getByTestId('archi-gate-submit')).toBeTruthy());
    const submit = screen.getByTestId<HTMLButtonElement>('archi-gate-submit');
    expect(submit.disabled).toBe(false);

    fireEvent.click(submit);
    await waitFor(() =>
      expect(screen.getByTestId<HTMLButtonElement>('archi-gate-submit').disabled).toBe(true),
    );

    await act(async () => {
      resolveGate?.();
    });
    await waitFor(() =>
      expect(screen.getByTestId<HTMLButtonElement>('archi-gate-submit').disabled).toBe(false),
    );
  });

  it('应在 root 为 null 时提交 gate 给出「无工作根」行内错误且零 invoke', async () => {
    render(
      <ElementDrawer
        root={null}
        mode="incremental"
        element={element()}
        relations={[]}
        findings={[]}
        onChanged={vi.fn()}
      />,
    );
    await act(async () => {
      fireEvent.click(screen.getByTestId('archi-gate-submit'));
    });
    await waitFor(() => expect(screen.getByTestId('archi-gate-error')).toBeTruthy());
    expect(screen.getByTestId('archi-gate-error').textContent).toBe('无工作根，无法改档');
    expect(callsOf('archi_set_gate')).toHaveLength(0);
    expect(callsOf('archi_module')).toHaveLength(0);
  });

  it('应在无 gate 错误时零错误行（不渲空占位）', async () => {
    render(
      <ElementDrawer
        root={'C:\\demo\\alpha'}
        mode="incremental"
        element={element()}
        relations={[]}
        findings={[]}
        onChanged={vi.fn()}
      />,
    );
    await waitFor(() => expect(screen.getByTestId('archi-gate-select')).toBeTruthy());
    expect(screen.queryByTestId('archi-gate-error')).toBeNull();
    expect(screen.queryByTestId('archi-drawer-error')).toBeNull();
  });

  it('应在非法取值被拒后保持原草稿（写回值不漂移）', async () => {
    render(
      <ElementDrawer
        root={'C:\\demo\\alpha'}
        mode="incremental"
        element={element({ gate: 'strict' })}
        relations={[]}
        findings={[]}
        onChanged={vi.fn()}
      />,
    );
    await waitFor(() => expect(screen.getByTestId('archi-gate-select')).toBeTruthy());
    const select = screen.getByTestId<HTMLSelectElement>('archi-gate-select');
    expect(select.value).toBe('strict');

    // 词汇外取值（类型层不可表示，运行期容错面）：草稿不变
    fireEvent.change(select, { target: { value: 'bogus' } });
    expect(select.value).toBe('strict');

    await act(async () => {
      fireEvent.click(screen.getByTestId('archi-gate-submit'));
    });
    await waitFor(() => expect(callsOf('archi_set_gate')).toHaveLength(1));
    expect(callsOf('archi_set_gate')[0]).toEqual({
      root: 'C:\\demo\\alpha',
      element: 'App.Auth',
      gate: 'strict',
    });
  });
});

describe('文档条目交互', () => {
  it('应只让 ADR 条目可展开（md / adjacent 条目按钮禁用）', async () => {
    render(
      <ElementDrawer
        root={'C:\\demo\\alpha'}
        mode="full"
        element={element()}
        relations={[]}
        findings={[]}
        onChanged={vi.fn()}
      />,
    );
    await waitFor(() => expect(screen.queryAllByTestId('archi-doc-item')).toHaveLength(3));
    expect(screen.getByTitle('src/auth/README.md').hasAttribute('disabled')).toBe(true);
    expect(screen.getByTitle('src/auth/guide.md').hasAttribute('disabled')).toBe(true);
    expect(
      screen
        .getByTitle('.dev-team/architecture/decisions/0007-token-sink.md')
        .hasAttribute('disabled'),
    ).toBe(false);
  });

  it('应再次点击同一条目重新打开预览（抽屉关闭后正文卸载，取数不重复累积）', async () => {
    render(
      <ElementDrawer
        root={'C:\\demo\\alpha'}
        mode="full"
        element={element()}
        relations={[]}
        findings={[]}
        onChanged={vi.fn()}
      />,
    );
    await waitFor(() => expect(screen.queryAllByTestId('archi-doc-item')).toHaveLength(3));
    const adr = screen.getByTitle('.dev-team/architecture/decisions/0007-token-sink.md');
    fireEvent.click(adr);
    await waitFor(() => expect(screen.getByTestId('file-content-markdown')).toBeTruthy());

    fireEvent.click(screen.getByRole('button', { name: '关闭' }));
    await waitFor(() => expect(screen.queryByTestId('file-content-markdown')).toBeNull());

    fireEvent.click(adr);
    await waitFor(() => expect(screen.getByTestId('file-content-markdown')).toBeTruthy());
    // 每次打开各取一次（关闭即卸载，无跨开合的缓存复用）
    expect(callsOf('read_file')).toHaveLength(2);
  });
});

describe('豁免段', () => {
  it('应只呈现与当前元素相关且带 reason 的边', async () => {
    render(
      <ElementDrawer
        root={'C:\\demo\\alpha'}
        mode="full"
        element={element()}
        relations={[
          relation({ from: 'App.Auth', to: 'App.Billing', mark: 'exempt', reason: '平台层特例' }),
          relation({ from: 'App.Billing', to: 'App.Auth', mark: 'exempt', reason: '反向豁免' }),
          relation({ from: 'App.Order', to: 'App.Pay', mark: 'exempt', reason: '无关边' }),
          relation({ from: 'App.Auth', to: 'App.Order', mark: 'unfulfilled', reason: null }),
        ]}
        findings={[]}
        onChanged={vi.fn()}
      />,
    );
    await waitFor(() => expect(callsOf('archi_module')).toHaveLength(1));
    const rows = screen.queryAllByTestId('archi-exempt-reason');
    expect(rows).toHaveLength(2);
    expect(rows[0]?.textContent).toContain('平台层特例');
    expect(rows[1]?.textContent).toContain('反向豁免');
    expect(screen.getByTestId('archi-drawer').textContent).not.toContain('无关边');
  });

  it('应在无豁免边时整段隐藏（零「豁免」标题）', async () => {
    render(
      <ElementDrawer
        root={'C:\\demo\\alpha'}
        mode="full"
        element={element()}
        relations={[
          relation({ from: 'App.Order', to: 'App.Pay', mark: 'exempt', reason: '无关边' }),
          relation({ from: 'App.Auth', to: 'App.Order', mark: 'none', reason: null }),
        ]}
        findings={[]}
        onChanged={vi.fn()}
      />,
    );
    await waitFor(() => expect(callsOf('archi_module')).toHaveLength(1));
    expect(screen.queryAllByTestId('archi-exempt-reason')).toHaveLength(0);
    expect(screen.getByTestId('archi-drawer').textContent).not.toContain('豁免');
  });
});

describe('取数异常', () => {
  it('应在 archi_module 返回 Err 时呈现错误文本且不渲半截概览', async () => {
    invokeMock.mockImplementation(() =>
      Promise.reject(new Error('元素不存在：App.Missing（模型共 3 个元素）')),
    );
    render(
      <ElementDrawer
        root={'C:\\demo\\alpha'}
        mode="full"
        element={element({ fqn: 'App.Missing' })}
        relations={[]}
        findings={[]}
        onChanged={vi.fn()}
      />,
    );
    await waitFor(() => expect(screen.getByTestId('archi-drawer-error')).toBeTruthy());
    expect(screen.getByTestId('archi-drawer-error').textContent).toContain('元素不存在');
    expect(screen.queryAllByTestId('archi-doc-item')).toHaveLength(0);
  });

  it('应在未选中元素时零 invoke 并列空态', async () => {
    render(
      <ElementDrawer
        root={'C:\\demo\\alpha'}
        mode="full"
        element={null}
        relations={[]}
        findings={[]}
        onChanged={vi.fn()}
      />,
    );
    await act(async () => {
      await Promise.resolve();
    });
    expect(callsOf('archi_module')).toHaveLength(0);
    expect(screen.getByTestId('archi-drawer').textContent).toContain('未选中元素');
  });
});

// ---------------------------------------------------------------------------
// 本变更：契约段（`api.status` + 三路径行 + 本元素判据 + 空态）
// ---------------------------------------------------------------------------

/** 契约段 detail 覆盖（`api` 投影可指定；其余字段取既有缺省）。 */
function serveModuleWithApi(api: Record<string, unknown>): void {
  invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
    if (command === 'archi_module') {
      const fqn = typeof params?.element === 'string' ? params.element : '';
      return Promise.resolve({
        element: element({ fqn }),
        dependsOn: [],
        dependedBy: [],
        children: [],
        docs: docsFixture(),
        warnings: [],
        api,
      });
    }
    if (command === 'read_file') {
      const content = '# 令牌收口\n';
      return Promise.resolve({
        content,
        stat: { size: content.length, modifiedMs: null, isFile: true, isDir: false },
        offset: 0,
        nextOffset: content.length,
        truncated: false,
        encoding: 'utf8',
      });
    }
    if (command === 'archi_set_gate') return Promise.resolve(null);
    return Promise.reject(new Error(`未预期命令: ${command}`));
  });
}

/** 判据夹具（六字段全给）。 */
function finding(overrides: Record<string, unknown> = {}) {
  return {
    kind: 'apiSignatureMismatch',
    category: 'signature',
    element: 'App.Auth',
    target: 'login',
    message: '契约签名与代码签名不一致',
    occurrences: [],
    ...overrides,
  };
}

/** 渲染抽屉（契约段语料由 `api` 指定）。 */
function renderWith(findings: Record<string, unknown>[], api: Record<string, unknown>) {
  serveModuleWithApi(api);
  render(
    <ElementDrawer
      root={'/ws'}
      mode="full"
      element={element()}
      relations={[]}
      findings={findings as never}
      onChanged={vi.fn()}
    />,
  );
}

const DECLARED_API = {
  status: 'declared',
  contractPath: '.dev-team/architecture/api/Core/App.Auth.md',
  schemaPath: '.dev-team/architecture/api/Core/App.Auth.schema.json',
  generatedPath: 'src/auth.i.ts',
};

describe('抽屉契约段', () => {
  it('应在概览与文档之间插入契约段（段标题为 h3）', async () => {
    renderWith([], DECLARED_API);
    await waitFor(() => expect(callsOf('archi_module')).toHaveLength(1));
    await waitFor(() =>
      expect(screen.getAllByRole('heading', { level: 3 }).length).toBeGreaterThan(0),
    );

    const headings = screen.getAllByRole('heading', { level: 3 }).map((node) => node.textContent);
    expect(headings).toContain('契约');
    const drawer = screen.getByTestId('archi-drawer');
    const text = drawer.textContent ?? '';
    const overviewIndex = text.indexOf('App.Auth');
    const contractIndex = text.indexOf('契约');
    const docsIndex = text.indexOf('文档');
    expect(overviewIndex).toBeLessThan(contractIndex);
    expect(contractIndex).toBeLessThan(docsIndex);
  });

  it('应让 declared 三路径行各自可打开（read_file 载荷 = root + 该 repo 相对串）', async () => {
    const paths = [DECLARED_API.contractPath, DECLARED_API.schemaPath, DECLARED_API.generatedPath];
    for (const path of paths) {
      cleanup();
      invokeMock.mockReset();
      renderWith([], DECLARED_API);
      await waitFor(() => expect(callsOf('archi_module')).toHaveLength(1));
      await waitFor(() => expect(screen.queryAllByTestId('archi-contract-path')).toHaveLength(3));

      const row = screen
        .queryAllByTestId('archi-contract-path')
        .find((candidate) => candidate.textContent?.includes(path))!;
      expect(row.getAttribute('title')).toBe(path);
      fireEvent.click(row.querySelector('button')!);
      await waitFor(() => expect(callsOf('read_file')).toHaveLength(1));
      const payload = callsOf('read_file')[0]!;
      expect(payload.path).toBe(`/ws/${path}`);
      expect(payload.offset).toBeNull();
      expect(payload.limit).toBeNull();
    }
  });

  it('应对路径为 null 的槽位渲禁用行（行在场、「查看」不可点）', async () => {
    renderWith([], { ...DECLARED_API, schemaPath: null });
    await waitFor(() => expect(callsOf('archi_module')).toHaveLength(1));
    await waitFor(() => expect(screen.queryAllByTestId('archi-contract-path')).toHaveLength(3));

    const rows = screen.queryAllByTestId('archi-contract-path');
    const labels = rows.map((row) => row.querySelector('span')?.textContent);
    expect(labels).toEqual(['契约', 'schema', '生成物']);
    // 缺席路径：行仍在场（不吞行），但「查看」禁用、点击零 read_file
    const nullRow = rows[1]!;
    expect(nullRow.textContent).toContain('（该路径未生成）');
    expect(nullRow.getAttribute('title')).toBeNull();
    const disabledView = nullRow.querySelector('button')!;
    expect(disabledView.disabled).toBe(true);
    fireEvent.click(disabledView);
    await act(async () => {
      await Promise.resolve();
    });
    expect(callsOf('read_file')).toHaveLength(0);
    // 另两行照常可点（禁用只作用于缺席槽位）
    for (const row of [rows[0]!, rows[2]!]) {
      expect(row.querySelector('button')!.disabled).toBe(false);
    }
  });

  it('应只列本元素判据（element === fqn 严格过滤）', async () => {
    renderWith(
      [
        finding({ element: 'App.Auth', kind: 'apiSignatureMismatch' }),
        finding({ element: 'App.Auth', kind: 'apiMissing', message: '实现缺失' }),
        finding({ element: 'App.Billing', kind: 'apiUnexpected', message: '邻元素判据' }),
        // 脏归属值：空串与 emoji 都不得被 `===` 判据误配到本元素（`App.Auth`）
        finding({ element: '', kind: 'apiMissing', message: '空归属判据' }),
        finding({ element: 'App.Aut🚀', kind: 'apiMissing', message: 'emoji判据' }),
      ],
      DECLARED_API,
    );
    await waitFor(() => expect(callsOf('archi_module')).toHaveLength(1));
    const drawer = screen.getByTestId('archi-drawer');
    await waitFor(() => expect(drawer.textContent).toContain('实现缺失'));

    expect(drawer.textContent).toContain('契约签名与代码签名不一致');
    for (const foreign of ['邻元素判据', '空归属判据', 'emoji判据']) {
      expect(drawer.textContent).not.toContain(foreign);
    }
  });

  it('应在本元素零判据时渲「本元素无漂移」', async () => {
    renderWith([finding({ element: 'App.Billing' })], DECLARED_API);
    await waitFor(() => expect(callsOf('archi_module')).toHaveLength(1));
    await waitFor(() =>
      expect(screen.getByTestId('archi-contract-drawer-empty').textContent).toContain(
        '本元素无漂移',
      ),
    );
  });

  it('应在 api.status = none 时只渲一行空态且三路径行均不渲', async () => {
    renderWith([], {
      status: 'none',
      contractPath: null,
      schemaPath: null,
      generatedPath: null,
    });
    await waitFor(() => expect(callsOf('archi_module')).toHaveLength(1));
    await waitFor(() =>
      expect(screen.getByTestId('archi-contract-drawer-empty').textContent).toContain('无契约'),
    );
    expect(screen.queryAllByTestId('archi-contract-path')).toHaveLength(0);
  });

  it('应零新增 invoke（只有 archi_module 与按需 read_file）', async () => {
    renderWith([finding()], DECLARED_API);
    await waitFor(() => expect(callsOf('archi_module')).toHaveLength(1));
    await waitFor(() => expect(screen.queryAllByTestId('archi-contract-path')).toHaveLength(3));
    const commands = invokeMock.mock.calls.map(([name]) => name as string);
    expect(commands).toEqual(['archi_module']);
    expect(commands).not.toContain('archi_api_check');
    expect(commands).not.toContain('archi_api_generate');
    expect(commands).not.toContain('archi_drift');
  });

  it('应全量列出超大判据列表（1000 条不裁剪）', async () => {
    const many = Array.from({ length: 1000 }, (_, index) =>
      finding({ element: 'App.Auth', kind: 'apiMissing', message: `判据 ${index}` }),
    );
    renderWith(many, DECLARED_API);
    await waitFor(() => expect(callsOf('archi_module')).toHaveLength(1));
    const drawer = screen.getByTestId('archi-drawer');
    await waitFor(() => expect(drawer.textContent).toContain('判据 999'));
    expect(drawer.querySelectorAll('[data-kind]').length).toBe(1000);
  });

  it('应在 root = null 时不渲契约段且零 archi_module 取数', async () => {
    serveModuleWithApi(DECLARED_API);
    render(
      <ElementDrawer
        root={null}
        mode="full"
        element={element()}
        relations={[]}
        findings={[]}
        onChanged={vi.fn()}
      />,
    );
    await act(async () => {
      await Promise.resolve();
    });
    expect(callsOf('archi_module')).toHaveLength(0);
    expect(screen.queryByTestId('archi-contract-drawer-empty')).toBeNull();
    expect(screen.queryAllByTestId('archi-contract-path')).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// 零新增 invoke（源码自证：契约段数据只来自既有 props）
// ---------------------------------------------------------------------------

describe('抽屉契约段：零新增 invoke（源码自证）', () => {
  it('应零引用契约面两命令与漂移命令（数据只经既有 archi_module 与 read_file）', () => {
    const source = readFileSync(
      resolve(process.cwd(), 'src/views/architecture/components/element-drawer.tsx'),
      'utf8',
    );
    for (const command of ['archiApiCheck', 'archiApiGenerate', 'archiDrift']) {
      expect(source.includes(command), `element-drawer.tsx 零 ${command} 引用`).toBe(false);
    }
    // 数据来源仍是既有模块详情 hook（零第二份取数）
    expect(source).toContain('useArchiModule');
  });
});
