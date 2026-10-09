import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { toast } from 'sonner';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import App from './app';
import type {
  ChangeDetail,
  ChangeList,
  ModelInfo,
  RecordEnvelope,
  WorkspaceRecord,
} from './types/dto';
import type { CodeStatsReport, WorkspaceConfigReport } from './types/generated/bindings';

const { checkMock, getVersionMock, invokeMock, openMock } = vi.hoisted(() => ({
  checkMock: vi.fn(),
  getVersionMock: vi.fn(),
  invokeMock: vi.fn(),
  openMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/app', () => ({ getVersion: getVersionMock }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: openMock }));
// updater mock（沿用 use-updater.test.ts 语义）：仅「重试更新」用例经 vi.stubEnv('DEV', false)
// 启用检查路径；其余用例 DEV=true 跳过启动检查
vi.mock('@tauri-apps/plugin-updater', () => ({ check: checkMock }));

// ---------------------------------------------------------------------------
// fixture：workspace 清单为默认序（canonical root 升序），FIRST 即默认序第一名
// ---------------------------------------------------------------------------

const FIRST: WorkspaceRecord = {
  root: 'C:\\demo\\alpha',
  name: 'alpha',
  addedAt: 1,
};
const SECOND: WorkspaceRecord = {
  root: 'C:\\demo\\beta',
  name: 'beta',
  addedAt: 1,
};

const fakeList: ChangeList = {
  active: [
    { name: 'add-feature', source: 'active', status: 'active', activePhase: null, created: null },
  ],
  archiveGroups: [],
};

const fakeDetail: ChangeDetail = {
  name: 'add-feature',
  source: 'active',
  status: 'active',
  created: null,
  pipeline: [],
  activePhase: null,
  artifacts: [],
  worktree: null,
};

// db 查看域 fixture：DbInspectorView 挂载即 invoke("db_models")，mock 必须回
// 数组形态（null 会使 hook state.models 置 null 而崩），信封按 offset/limit 切页
const DB_MODELS: ModelInfo[] = [
  { name: 'workspace', count: 2 },
  { name: 'agent_run', count: 1 },
  { name: 'agent_event', count: 0 },
];
const DB_RECORDS: RecordEnvelope[] = [
  { key: 'C:\\demo\\alpha', value: { root: 'C:\\demo\\alpha', name: 'alpha', addedAt: 1 } },
  { key: 1, value: { id: 1, prompt: '你好', status: 'completed' } },
  { key: { runId: 1, seq: 0 }, value: { eventKey: '0x1', runId: 1, event: { seq: 0 } } },
];

// 基础信息域 fixture：InfoView 挂载即以 (root, depth=5) invoke("code_stats")，
// 三面 DTO（totals.files > 0 → 非空态），树面顶层节点默认展开在 DOM
const CODE_STATS: CodeStatsReport = {
  totals: { files: 12, code: 340, comments: 40, blanks: 20 },
  languages: [
    { name: 'Rust', files: 6, code: 220, comments: 30, blanks: 10, share: 64.7 },
    { name: 'TypeScript', files: 6, code: 120, comments: 10, blanks: 10, share: 35.3 },
  ],
  tree: [
    {
      kind: 'dir',
      node: {
        name: 'src',
        path: 'src',
        files: 9,
        code: 300,
        comments: 35,
        blanks: 18,
        children: [],
      },
    },
  ],
};

// 配置域 fixture：ConfigView 挂载即 invoke("workspace_config")，camelCase 信封
// （config + diagnostics）；context 归属 root 由 configReportFor 注入（切换
// 用例区分新旧根数据）
const WORKSPACE_CONFIG: WorkspaceConfigReport = {
  config: {
    $schema: null,
    schema: 'spec-driven',
    context: null,
    rules: { proposal: ['提案规则'], tasks: null },
    staticAnalysis: 'clippy',
    tests: [
      {
        root: 'packages/desktop',
        framework: 'vite-plus',
        cwd: 'packages/desktop',
        config: null,
        includes: null,
        excludes: null,
        coverage: { lines: 80, branches: 70, functions: 75 },
        mutation: { cwd: null, score: 70 },
      },
    ],
    writeProtection: null,
    extra: [],
  },
  diagnostics: [
    {
      kind: 'defaultApplied',
      path: 'tests[0].coverage.lines',
      message: '覆盖率阈值 lines 未设置，使用默认值 80。',
    },
  ],
};

/** 配置报告 fixture 深拷贝（防用例间引用残留），context 承载归属 root。 */
function configReportFor(root: string): WorkspaceConfigReport {
  const report = JSON.parse(JSON.stringify(WORKSPACE_CONFIG)) as WorkspaceConfigReport;
  report.config.context = root;
  return report;
}

// ---------------------------------------------------------------------------
// 进程边界 Mock：IPC 按命令名分发；文件夹对话框按用例 resolve / reject / cancel；
// 版本号固定 resolve。sonner <Toaster /> 不 mock：App 根真实挂载，toast 断言走
// DOM 文案 + waitFor。sonner toast 状态为模块级单例、跨用例存活，beforeEach
// 统一 dismiss 防残留。
// ---------------------------------------------------------------------------

let remaining: WorkspaceRecord[];
let addBehavior: 'ok' | 'reject' = 'ok';
let addRecord: WorkspaceRecord | null = null;
let removeReject: string | null = null;
let removeMiss = false;
let listReject: string | null = null;
let configReject: string | null = null;
let configPending = false;

function mockIpc() {
  remaining = [FIRST, SECOND];
  addBehavior = 'ok';
  addRecord = null;
  removeReject = null;
  removeMiss = false;
  listReject = null;
  configReject = null;
  configPending = false;
  invokeMock.mockImplementation(
    (
      command: string,
      params?: { root?: string; change?: string; offset?: number; limit?: number },
    ) => {
      if (command === 'list_workspaces') {
        if (listReject !== null) return Promise.reject(new Error(listReject));
        return Promise.resolve([...remaining]);
      }
      if (command === 'remove_workspace') {
        if (removeReject !== null) return Promise.reject(new Error(removeReject));
        if (removeMiss) return Promise.resolve(false); // store miss 幂等（D8 排除项）
        remaining = remaining.filter((r) => r.root !== params?.root);
        return Promise.resolve(true);
      }
      if (command === 'add_workspace') {
        if (addBehavior === 'reject') {
          return Promise.reject(new Error('canonicalize: 入库失败'));
        }
        const rec: WorkspaceRecord = addRecord ?? {
          root: params?.root ?? '',
          name: 'picked',
          addedAt: 1,
        };
        // 对齐后端：库存按默认序（canonical root 升序）返回，新记录未必居首
        remaining = [...remaining.filter((r) => r.root !== rec.root), rec].sort((a, b) =>
          a.root < b.root ? -1 : a.root > b.root ? 1 : 0,
        );
        return Promise.resolve(rec);
      }
      if (command === 'list_changes') {
        return Promise.resolve(fakeList);
      }
      if (command === 'get_change_detail') {
        return Promise.resolve(fakeDetail);
      }
      if (command === 'db_models') {
        return Promise.resolve(DB_MODELS.map((model) => ({ ...model })));
      }
      if (command === 'db_records') {
        const offset = params?.offset ?? 0;
        const limit = params?.limit ?? 50;
        return Promise.resolve(DB_RECORDS.slice(offset, offset + limit).map((e) => ({ ...e })));
      }
      if (command === 'agent_sessions' || command === 'agent_session_transcript') {
        return Promise.resolve([]);
      }
      // agent 选择器挂载取数（useAgentOptions 发起 list_agent_instances）：同回
      // 数组形态（null 会使实例清单置 null 而崩），页面切页断言不涉及其内容
      if (command === 'list_agent_instances') {
        return Promise.resolve([]);
      }
      // 基础信息页挂载取数（useCodeStats 挂载发起 code_stats）：回三面 DTO，
      // fixture 深拷贝防用例间引用残留
      if (command === 'code_stats') {
        return Promise.resolve(JSON.parse(JSON.stringify(CODE_STATS)) as CodeStatsReport);
      }
      // 配置页挂载取数（useWorkspaceConfig 挂载发起 workspace_config）：
      // resolve（context 归属 root）/ reject / 手动 pending 三态控制
      if (command === 'workspace_config') {
        if (configReject !== null) return Promise.reject(new Error(configReject));
        if (configPending) return new Promise<WorkspaceConfigReport>(() => {});
        return Promise.resolve(configReportFor(params?.root ?? ''));
      }
      return Promise.resolve(null);
    },
  );
}

// ---------------------------------------------------------------------------
// 装置
// ---------------------------------------------------------------------------

/** mock.calls 中某命令的调用次数。 */
function countOf(command: string): number {
  return invokeMock.mock.calls.filter(([name]) => name === command).length;
}

/** 以 data-root 定位 sidebar 清单项（同名项由 data-root 区分）。 */
function itemByRoot(root: string): HTMLElement {
  const hit = screen
    .getAllByTestId('workspace-item')
    .find((item) => item.getAttribute('data-root') === root);
  if (!hit) throw new Error(`data-root 为 ${root} 的 workspace-item 不存在`);
  return hit;
}

/** 有记录启动：等待自动恢复第一名进入列表视图（列表已渲染）。 */
async function restored() {
  render(<App />);
  await waitFor(() =>
    expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: FIRST.root }),
  );
  await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
}

function fakeUpdate(version = '0.2.0') {
  return {
    version,
    body: '修复若干问题',
    close: vi.fn().mockResolvedValue(undefined),
    downloadAndInstall: vi.fn().mockResolvedValue(undefined),
  };
}

// 流程图区（建档详情，status 在场）挂载 ReactFlow：jsdom 缺口垫片（环境 stub
// 而非业务 mock）——进入详情视图的用例全局兜底（xyflow 边标签度量另需
// SVGGraphicsElement.getBBox 零包围盒垫片）
class ResizeObserverStub {
  observe = vi.fn();
  unobserve = vi.fn();
  disconnect = vi.fn();
}

beforeEach(() => {
  vi.stubGlobal('ResizeObserver', ResizeObserverStub);
  const svgPrototype = globalThis.SVGElement?.prototype as unknown as
    | Record<string, unknown>
    | undefined;
  if (svgPrototype !== undefined && typeof svgPrototype.getBBox !== 'function') {
    svgPrototype.getBBox = () => ({ x: 0, y: 0, width: 0, height: 0 });
  }
});

describe('App：启动恢复、欢迎屏清单与视图状态（AC-9）', () => {
  beforeEach(() => {
    // 路由化后 App 自含 HashRouter（design D6）：jsdom location 跨用例存活，
    // 上一用例残留的 hash 会改变下一用例启动路由初态，先重置
    window.location.hash = '';
    getVersionMock.mockReset();
    invokeMock.mockReset();
    openMock.mockReset();
    checkMock.mockReset();
    checkMock.mockResolvedValue(null);
    // useUpdater 挂载拉取版本号：走独立 mock，不混入 invoke 调用序列断言
    getVersionMock.mockResolvedValue('0.1.0');
    toast.dismiss(); // sonner 模块级 toast 状态跨用例存活：清残留
    mockIpc();
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('list_workspaces 返回空清单启动：呈现欢迎屏空态与「添加新文件夹」入口，不进入列表视图', async () => {
    remaining = [];
    render(<App />);

    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));

    expect(screen.getByText(/还没有记录/) !== null).toBe(true);
    expect(screen.getByText(/选择一个项目根目录/) !== null).toBe(true);
    // 欢迎屏清单标题与计数（D7：空清单计数为 0）
    expect(screen.getByText(/最近的 workspace/).textContent).toContain('(0)');
    expect(screen.queryByRole('button', { name: '刷新' })).toBeNull();
    // 除挂载自动 load 外无任何取数
    expect(invokeMock.mock.calls.every(([name]) => name === 'list_workspaces')).toBe(true);
  });

  it('欢迎屏加载中：挂载取数未返回时呈现「加载中…」，清单返回后消失', async () => {
    let resolveLoad: (records: WorkspaceRecord[]) => void = () => {};
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_workspaces') {
        return new Promise<WorkspaceRecord[]>((resolve) => {
          resolveLoad = resolve;
        });
      }
      return Promise.resolve(null);
    });
    render(<App />);

    await waitFor(() => expect(screen.getByText('加载中…') !== null).toBe(true));
    await act(async () => {
      resolveLoad([]);
    });
    await waitFor(() => expect(screen.queryByText('加载中…')).toBeNull());
    // 加载结束仍未有根：停留欢迎屏空态
    expect(screen.getByText('添加新文件夹') !== null).toBe(true);
  });

  it('有记录启动：自动恢复默认序第一名为当前根并进入列表视图（无任何 workspace 动作命令）', async () => {
    await restored();

    expect(screen.getByText(/进行中/) !== null).toBe(true);
    // 恢复为纯查询链：挂载取数 → 以默认序第一名取 change 列表，无动作命令
    expect(invokeMock.mock.calls.map(([name]) => name)).toEqual([
      'list_workspaces',
      'list_changes',
    ]);
  });

  it('恢复进入列表后：列表项点击进入详情视图，返回列表后「刷新」重发 list_changes', async () => {
    await restored();

    fireEvent.click(screen.getByText('add-feature'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('get_change_detail', {
        root: FIRST.root,
        change: 'add-feature',
      }),
    );
    expect(screen.getByRole('heading', { name: 'add-feature' }) !== null).toBe(true);

    // 刷新按钮随 header 瘦身迁入清单页头部（图标钮 aria-label=刷新）：详情视图内不在场，返回列表后可操作
    expect(screen.queryByRole('button', { name: '刷新' })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: '← 返回列表' }));
    await waitFor(() => expect(screen.getByRole('button', { name: '刷新' }) !== null).toBe(true));

    const before = invokeMock.mock.calls.filter(([name]) => name === 'list_changes').length;
    fireEvent.click(screen.getByRole('button', { name: '刷新' }));
    await waitFor(() => {
      const after = invokeMock.mock.calls.filter(([name]) => name === 'list_changes').length;
      expect(after).toBe(before + 1);
    });
  });

  it('添加对话框取消（返回 null）：不调用 add_workspace、停留欢迎屏', async () => {
    remaining = [];
    render(<App />);
    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));

    openMock.mockResolvedValue(null);
    fireEvent.click(screen.getByText('添加新文件夹'));

    await waitFor(() => expect(openMock).toHaveBeenCalled());
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(invokeMock.mock.calls.every(([name]) => name === 'list_workspaces')).toBe(true);
    expect(screen.getByText('添加新文件夹') !== null).toBe(true);
  });

  it('添加流以标准参数调用文件夹对话框，并将选中路径入库后打开（欢迎屏「添加新文件夹」入口保留登记）', async () => {
    remaining = [];
    render(<App />);
    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));

    openMock.mockResolvedValue('C:\\picked');
    fireEvent.click(screen.getByText('添加新文件夹'));

    // 对话框参数契约：目录模式 + 单选
    await waitFor(() =>
      expect(openMock).toHaveBeenCalledWith({ directory: true, multiple: false }),
    );
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('add_workspace', { root: 'C:\\picked' }),
    );
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: 'C:\\picked' }),
    );
    expect(screen.queryByText('添加新文件夹')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 关系一：sidebar 列表项交互 → workspace 动作链（切换 / 添加 / 移除）
// 改写自下拉时代用例；IPC 时序/次数/参数断言与现状逐字一致（AC-3 硬约束）。
// ---------------------------------------------------------------------------

describe('App：sidebar 列表项交互 → workspace 动作链（AC-3/AC-4/AC-5）', () => {
  beforeEach(() => {
    // 路由化后 App 自含 HashRouter（design D6）：jsdom location 跨用例存活，
    // 上一用例残留的 hash 会改变下一用例启动路由初态，先重置
    window.location.hash = '';
    getVersionMock.mockReset();
    invokeMock.mockReset();
    openMock.mockReset();
    checkMock.mockReset();
    checkMock.mockResolvedValue(null);
    getVersionMock.mockResolvedValue('0.1.0');
    toast.dismiss();
    mockIpc();
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('点击非当前清单项：本地切换当前根（无 workspace 动作命令）→ list_changes 以新根发起 → change 选中清空', async () => {
    await restored();
    // 先进入详情视图，验证切换后选中被清空
    fireEvent.click(screen.getByText('add-feature'));
    await waitFor(() =>
      expect(screen.getByRole('heading', { name: 'add-feature' }) !== null).toBe(true),
    );

    fireEvent.click(itemByRoot(SECOND.root));

    await waitFor(() => {
      const lastListChanges = invokeMock.mock.calls
        .filter(([name]) => name === 'list_changes')
        .at(-1);
      expect(lastListChanges).toEqual(['list_changes', { root: SECOND.root }]);
    });
    // 本地切换：无 add/remove 等任何 workspace 动作命令
    expect(countOf('add_workspace')).toBe(0);
    expect(countOf('remove_workspace')).toBe(0);
    // change 选中清空：详情视图退出回到列表视图，且不以新根重发 get_change_detail
    expect(screen.queryByRole('heading', { name: 'add-feature' })).toBeNull();
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
    const detailCalls = invokeMock.mock.calls.filter(([name]) => name === 'get_change_detail');
    expect(detailCalls.length).toBeGreaterThan(0);
    expect(detailCalls.every(([, params]) => params?.root === FIRST.root)).toBe(true);
  });

  it('切换后激活态迁移：原当前项退出 isActive、新当前项进入，清单保持默认序不重排', async () => {
    await restored();

    expect(itemByRoot(FIRST.root).hasAttribute('data-active')).toBe(true);
    expect(itemByRoot(SECOND.root).hasAttribute('data-active')).toBe(false);

    fireEvent.click(itemByRoot(SECOND.root));

    await waitFor(() => {
      expect(itemByRoot(SECOND.root).hasAttribute('data-active')).toBe(true);
      expect(itemByRoot(FIRST.root).hasAttribute('data-active')).toBe(false);
    });
    // 本地切换不重排清单：默认序保持
    const roots = screen
      .getAllByTestId('workspace-item')
      .map((item) => item.getAttribute('data-root'));
    expect(roots).toEqual([FIRST.root, SECOND.root]);
  });

  it('对唯一清单项（当前项自身）点击：无命令发起、视图状态无抖动', async () => {
    remaining = [FIRST];
    render(<App />);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: FIRST.root }),
    );

    const callsBefore = invokeMock.mock.calls.length;
    fireEvent.click(itemByRoot(FIRST.root));

    // 本地切换当前项自身：无任何新命令，root 无抖动
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
    });
    expect(invokeMock.mock.calls.length).toBe(callsBefore);
    const lastListChanges = invokeMock.mock.calls
      .filter(([name]) => name === 'list_changes')
      .at(-1);
    expect(lastListChanges).toEqual(['list_changes', { root: FIRST.root }]);
    expect(screen.getByText('add-feature') !== null).toBe(true);
  });

  it('点击 GroupAction 内联图标：open({directory:true, multiple:false}) → add_workspace → 以返回记录打开 → list_changes 以返回记录的 canonical root 发起（改写：欢迎屏入口 → GroupAction 图标）', async () => {
    await restored();
    // 返回记录的 canonical root 与对话框原串书写不等价（模拟 canonicalize）
    addRecord = { root: 'C:\\canonical\\picked', name: 'picked', addedAt: 1 };
    openMock.mockResolvedValue('/raw/PICKED DIR');

    fireEvent.click(screen.getByRole('button', { name: '添加 workspace' }));

    // 对话框参数契约断言逐字保留：目录模式 + 单选
    await waitFor(() =>
      expect(openMock).toHaveBeenCalledWith({ directory: true, multiple: false }),
    );
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('add_workspace', { root: '/raw/PICKED DIR' }),
    );
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: 'C:\\canonical\\picked' }),
    );
    await waitFor(() => expect(screen.getByText('picked') !== null).toBe(true));
    expect(itemByRoot('C:\\canonical\\picked').hasAttribute('data-active')).toBe(true);
  });

  it('对话框取消（null）：不调用 add_workspace、停留当前态——两处入口（欢迎屏 / GroupAction）行为一致', async () => {
    await restored();
    openMock.mockResolvedValue(null);

    // 入口一：壳态 GroupAction
    fireEvent.click(screen.getByRole('button', { name: '添加 workspace' }));
    await waitFor(() => expect(openMock).toHaveBeenCalled());
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
    });
    expect(countOf('add_workspace')).toBe(0);
    expect(screen.getAllByTestId('workspace-item')).toHaveLength(2);

    // 入口二：欢迎屏「添加新文件夹」（空清单）
    remaining = [];
    cleanup();
    render(<App />);
    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));
    fireEvent.click(screen.getByText('添加新文件夹'));
    await waitFor(() => expect(openMock).toHaveBeenCalledTimes(2));
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
    });
    expect(countOf('add_workspace')).toBe(0);
    expect(screen.getByText('添加新文件夹') !== null).toBe(true);
  });

  it('右键当前项 → 点击「移除」：remove_workspace {root} → 切剩余第一名 → list_changes 以新根发起；无确认弹窗（改写：header「移除」按钮 → 右键菜单项）', async () => {
    await restored();

    fireEvent.contextMenu(itemByRoot(FIRST.root));
    fireEvent.click(await screen.findByRole('menuitem', { name: '移除' }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('remove_workspace', { root: FIRST.root }),
    );
    await waitFor(() => {
      const lastListChanges = invokeMock.mock.calls
        .filter(([name]) => name === 'list_changes')
        .at(-1);
      expect(lastListChanges).toEqual(['list_changes', { root: SECOND.root }]);
    });
    // 无确认弹窗：菜单点击后直接下发，无 dialog 节点插入
    expect(screen.queryByRole('dialog')).toBeNull();
    // 清单收缩：仅剩剩余第一名
    const roots = screen
      .getAllByTestId('workspace-item')
      .map((item) => item.getAttribute('data-root'));
    expect(roots).toEqual([SECOND.root]);
  });

  it('右键移除非当前项：当前根不变（list_changes 不以他根重发）、该项从清单消失', async () => {
    await restored();

    fireEvent.contextMenu(itemByRoot(SECOND.root));
    fireEvent.click(await screen.findByRole('menuitem', { name: '移除' }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('remove_workspace', { root: SECOND.root }),
    );
    await waitFor(() => {
      const roots = screen
        .getAllByTestId('workspace-item')
        .map((item) => item.getAttribute('data-root'));
      expect(roots).toEqual([FIRST.root]);
    });
    // 剩余第一名即原当前项：list_changes 不以 SECOND 重发
    const listChangeRoots = invokeMock.mock.calls
      .filter(([name]) => name === 'list_changes')
      .map(([, params]) => params?.root);
    expect(listChangeRoots.every((root) => root === FIRST.root)).toBe(true);
  });

  it('移除至空清单：回欢迎屏（WelcomeView 全屏）且 Toaster 仍挂载', async () => {
    remaining = [FIRST];
    render(<App />);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: FIRST.root }),
    );

    fireEvent.contextMenu(itemByRoot(FIRST.root));
    fireEvent.click(await screen.findByRole('menuitem', { name: '移除' }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('remove_workspace', { root: FIRST.root }),
    );
    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));
    expect(document.querySelector('[data-slot="sidebar-wrapper"]')).toBeNull();
    expect(document.querySelector('section[aria-label^="Notifications"]') !== null).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// 关系二：错误双轨呈现——动作 reject → toast / 查询 reject → inline（AC-8）
// toast 轨经真实 sonner DOM 断言（D8 固定文案前缀 + waitFor）；inline 轨由
// workspace_restore / ChangeListView 等既有套件承载，此处核对 App 层形态。
// ---------------------------------------------------------------------------

describe('App：错误双轨呈现——动作 reject → toast / 查询 reject → inline（AC-8）', () => {
  beforeEach(() => {
    // 路由化后 App 自含 HashRouter（design D6）：jsdom location 跨用例存活，
    // 上一用例残留的 hash 会改变下一用例启动路由初态，先重置
    window.location.hash = '';
    getVersionMock.mockReset();
    invokeMock.mockReset();
    openMock.mockReset();
    checkMock.mockReset();
    checkMock.mockResolvedValue(null);
    getVersionMock.mockResolvedValue('0.1.0');
    toast.dismiss();
    mockIpc();
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('remove reject：toast 呈现「移除 workspace 失败：」+ 错误串（waitFor 文案断言），且 error-note 为 0、停留列表视图（改写：原 Header error-note 呈现断言 → toast 文案断言）', async () => {
    render(<App />);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: FIRST.root }),
    );
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));

    // 无错误基线：inline 通道一个都不渲染
    expect(screen.queryAllByTestId('error-note')).toHaveLength(0);

    removeReject = 'db: 移除失败';
    fireEvent.contextMenu(itemByRoot(FIRST.root));
    fireEvent.click(await screen.findByRole('menuitem', { name: '移除' }));

    await waitFor(() =>
      expect(screen.getByText(/移除 workspace 失败：.*db: 移除失败/) !== null).toBe(true),
    );
    // 动作失败不置 error 态：inline 通道归零（AC-8 前半）
    expect(screen.queryAllByTestId('error-note')).toHaveLength(0);
    // 错误呈现不切换视图：仍停留列表视图
    expect(screen.getByText('add-feature') !== null).toBe(true);
    expect(screen.queryByText('添加新文件夹')).toBeNull();
  });

  it('欢迎屏 add reject：toast 呈现「添加 workspace 失败：」、无 error-note、可重试（改写：原「error-note 呈现」断言 → toast 断言；对话框调用 reject 分支静默保持现状语义）', async () => {
    remaining = []; // 必须先于 render：挂载即自动取数
    render(<App />);
    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));

    // 分支一：对话框调用 reject —— 静默保持欢迎屏可重试
    openMock.mockRejectedValue(new Error('对话框崩溃'));
    fireEvent.click(screen.getByText('添加新文件夹'));
    await waitFor(() => expect(openMock).toHaveBeenCalled());
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
    });
    expect(screen.getByText('添加新文件夹') !== null).toBe(true);

    // 分支二：add_workspace reject —— toast 呈现、inline 通道归零、停留欢迎屏
    openMock.mockResolvedValue('C:\\picked');
    addBehavior = 'reject';
    fireEvent.click(screen.getByText('添加新文件夹'));
    await waitFor(() =>
      expect(screen.getByText(/添加 workspace 失败：.*canonicalize: 入库失败/) !== null).toBe(true),
    );
    expect(screen.queryAllByTestId('error-note')).toHaveLength(0);
    expect(screen.getByText('添加新文件夹') !== null).toBe(true);

    // 可重试：再次添加成功后以返回记录的 root 进入列表视图
    addBehavior = 'ok';
    fireEvent.click(screen.getByText('添加新文件夹'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('add_workspace', { root: 'C:\\picked' }),
    );
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: 'C:\\picked' }),
    );
    expect(screen.queryByText('添加新文件夹')).toBeNull();
  });

  it('remove resolve(false)（store miss 幂等）：无 toast、无 error-note（D8 排除项在壳层的核对）', async () => {
    removeMiss = true;
    render(<App />);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: FIRST.root }),
    );

    fireEvent.contextMenu(itemByRoot(FIRST.root));
    fireEvent.click(await screen.findByRole('menuitem', { name: '移除' }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('remove_workspace', { root: FIRST.root }),
    );
    // 内部刷新照常发生（挂载取数 + 移除刷新 = 2 次）
    await waitFor(() => expect(countOf('list_workspaces')).toBe(2));
    expect(document.querySelector('[data-sonner-toast]')).toBeNull();
    expect(screen.queryAllByTestId('error-note')).toHaveLength(0);
  });

  it('全部动作与查询成功：无 toast、无 error-note（零呈现基线）', async () => {
    render(<App />);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: FIRST.root }),
    );
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
    });

    expect(document.querySelector('[data-sonner-toast]')).toBeNull();
    expect(screen.queryAllByTestId('error-note')).toHaveLength(0);
  });

  it('list_workspaces reject：欢迎屏 error-note 持久渲染不消失（「workspace 清单加载失败」前缀语义）——查询轨 inline 不入双轨', async () => {
    listReject = 'db: 清单打开失败';
    render(<App />);

    await waitFor(() => expect(screen.getByTestId('error-note') !== null).toBe(true));
    expect(screen.getByTestId('error-note').textContent).toContain('workspace 清单加载失败');
    expect(screen.getByTestId('error-note').textContent).toContain('db: 清单打开失败');
    // 持久渲染不消失（inline 轨语义），且不经 toast
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    expect(screen.getByTestId('error-note') !== null).toBe(true);
    expect(document.querySelector('[data-sonner-toast]')).toBeNull();
  });

  it('updater error：「重试更新」按钮在场且点击语义不变（不经 toast，UpdateIndicator 逐字不动）', async () => {
    // 安装版语义：启用启动检查，发现新版本 →「更新到 v0.2.0」
    vi.stubEnv('DEV', false);
    checkMock.mockResolvedValueOnce(fakeUpdate());
    render(<App />);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: '更新到 v0.2.0' }) !== null).toBe(true),
    );

    // 用户触发更新失败 → error 态转「重试更新」
    checkMock.mockRejectedValue(new Error('network: 更新检查失败'));
    fireEvent.click(screen.getByRole('button', { name: '更新到 v0.2.0' }));
    await waitFor(() => expect(checkMock).toHaveBeenCalledTimes(2));
    await waitFor(() =>
      expect(screen.getByRole('button', { name: '重试更新' }) !== null).toBe(true),
    );

    // 点击语义不变：重新 check；错误呈现不经 toast
    fireEvent.click(screen.getByRole('button', { name: '重试更新' }));
    await waitFor(() => expect(checkMock).toHaveBeenCalledTimes(3));
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
    });
    expect(document.querySelector('[data-sonner-toast]')).toBeNull();
    expect(screen.getByRole('button', { name: '重试更新' }) !== null).toBe(true);
  });

  it('更新下载进度呈「下载中 42%」精确文案，Finished 转「正在安装…」终态并退出下载态', async () => {
    // 安装版语义：启用启动检查
    vi.stubEnv('DEV', false);
    type UpdateEvent = { event: string; data?: { chunkLength?: number; contentLength?: number } };
    // downloadAndInstall 挂起以维持 downloading 形态；事件回调经 channel 捕获后按拍派发
    const channel: { emit: ((event: UpdateEvent) => void) | null } = { emit: null };
    const updating = {
      version: '0.2.0',
      body: '修复若干问题',
      close: vi.fn().mockResolvedValue(undefined),
      downloadAndInstall: vi.fn((onEvent: (event: UpdateEvent) => void) => {
        channel.emit = onEvent;
        return new Promise<void>(() => {});
      }),
    };
    checkMock.mockResolvedValueOnce(fakeUpdate()); // 启动检查发现新版本
    render(<App />);
    const entry = await screen.findByRole('button', { name: '更新到 v0.2.0' });

    checkMock.mockResolvedValueOnce(updating); // start 重新 check 取新鲜实例
    fireEvent.click(entry);
    await waitFor(() => expect(channel.emit).not.toBeNull());

    // 下载中段：Started(contentLength=100) + Progress(chunkLength=42) → 42%
    act(() => {
      channel.emit!({ event: 'Started', data: { contentLength: 100 } });
      channel.emit!({ event: 'Progress', data: { chunkLength: 42 } });
    });
    await waitFor(() => expect(screen.getByText('下载中 42%') !== null).toBe(true));
    expect(screen.queryByText('正在安装…')).toBeNull();

    // Finished → installing 终态（Windows 上其后无回调）
    act(() => {
      channel.emit!({ event: 'Finished' });
    });
    await waitFor(() => expect(screen.getByText('正在安装…') !== null).toBe(true));
    expect(screen.queryByText(/下载中/)).toBeNull();
  });

  it('版本号拉取失败：currentVersion=null 时顶栏不呈版本号文本（v 前缀 span 不渲染）', async () => {
    getVersionMock.mockRejectedValue(new Error('version: 不可用'));
    render(<App />);
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));

    // 断言收敛于顶栏（内容区 v2 inventory 徽标等与版本指示无关）
    const header = document.querySelector('header');
    expect(header !== null).toBe(true);
    expect(within(header!).queryByText(/^v\d/)).toBeNull();
    expect(within(header!).queryByText('v')).toBeNull(); // 守卫被移除时会渲染出孤立的「v」
    expect(within(header!).getByText('Dev Team') !== null).toBe(true); // 顶栏其余元素在场
  });
});

// ---------------------------------------------------------------------------
// 关系三：壳层布局与折叠形态——App 壳 ↔ ui/sidebar（AC-1/AC-2/AC-7）
// jsdom 断言拓扑与语义属性（data-slot / data-sidebar / data-state）而非观感。
// ---------------------------------------------------------------------------

describe('App：壳层布局与折叠形态（AC-1/AC-2/AC-7）', () => {
  beforeEach(() => {
    // 路由化后 App 自含 HashRouter（design D6）：jsdom location 跨用例存活，
    // 上一用例残留的 hash 会改变下一用例启动路由初态，先重置
    window.location.hash = '';
    getVersionMock.mockReset();
    invokeMock.mockReset();
    openMock.mockReset();
    checkMock.mockReset();
    checkMock.mockResolvedValue(null);
    getVersionMock.mockResolvedValue('0.1.0');
    toast.dismiss();
    mockIpc();
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('壳态渲染 SidebarProvider / AppSidebar / SidebarInset 的 DOM 标记，宽度撑满inset容器', async () => {
    await restored();

    expect(document.querySelector('[data-slot="sidebar-wrapper"]') !== null).toBe(true);
    const sidebar = document.querySelector('[data-slot="sidebar"]');
    expect(sidebar?.getAttribute('data-side')).toBe('left');
    expect(sidebar?.getAttribute('data-state')).toBe('expanded');
    expect(document.querySelector('[data-sidebar="sidebar"]') !== null).toBe(true);
    const inset = document.querySelector('main[data-slot="sidebar-inset"]');
    expect(inset !== null).toBe(true);

    // 宽度撑满inset容器
    const container = Array.from(inset?.children ?? []).find((el) =>
      el.className.includes('w-full'),
    );
    expect(container?.tagName).toBe('DIV');
    expect(container?.contains(screen.getByText('add-feature'))).toBe(true);
    expect(inset?.querySelectorAll('main')).toHaveLength(0);
  });

  it('页面无 combobox：queryByRole("combobox") 为 null（AC-1 硬断言；取代原「下拉保持可操作」废弃用例）', async () => {
    await restored();

    expect(screen.queryByRole('combobox')).toBeNull();
    expect(screen.queryAllByRole('option')).toHaveLength(0);
  });

  it('header 终态：折叠钮（SidebarTrigger）+ 标题「Desktop Terminal」+ 版本/更新指示在场；无「刷新」「移除」按钮', async () => {
    await restored();

    const header = document.querySelector('header');
    expect(header !== null).toBe(true);
    const scope = within(header!);
    expect(header?.querySelector('[data-sidebar="trigger"]') !== null).toBe(true);
    expect(scope.getByText('Dev Team') !== null).toBe(true);
    expect(scope.getByText('v0.1.0') !== null).toBe(true);
    // 刷新入口迁清单页头部、移除入口迁右键菜单：header 内不再有这两枚按钮
    expect(scope.queryByRole('button', { name: '刷新' })).toBeNull();
    expect(scope.queryByText('移除')).toBeNull();
  });

  it('root=null（空清单启动）：无 SidebarProvider / AppSidebar DOM，WelcomeView 全屏文案在场', async () => {
    remaining = [];
    render(<App />);

    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));

    expect(document.querySelector('[data-slot="sidebar-wrapper"]')).toBeNull();
    expect(document.querySelector('[data-slot="sidebar"]')).toBeNull();
    expect(document.querySelector('header')).toBeNull();
    expect(screen.getByText(/还没有记录/) !== null).toBe(true);
    expect(screen.getByText(/选择一个项目根目录/) !== null).toBe(true);
  });

  it('Toaster 与条件渲染同级置于 App 根：欢迎态与壳态各验一次 sonner 容器在场', async () => {
    remaining = [];
    const welcome = render(<App />);
    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));
    expect(document.querySelector('section[aria-label^="Notifications"]') !== null).toBe(true);
    welcome.unmount();

    remaining = [FIRST, SECOND];
    await restored();
    expect(document.querySelector('section[aria-label^="Notifications"]') !== null).toBe(true);
  });

  it('collapsible="icon"：触发折叠后 sidebar 容器呈 icon 折叠标记（data-collapsible / data-state），清单项 Tooltip 在折叠态可显（D4-②）', async () => {
    await restored();

    const sidebar = document.querySelector('[data-slot="sidebar"]')!;
    expect(sidebar.getAttribute('data-collapsible')).toBe('');
    fireEvent.keyDown(window, { key: 'b', ctrlKey: true });
    expect(sidebar.getAttribute('data-state')).toBe('collapsed');
    expect(sidebar.getAttribute('data-collapsible')).toBe('icon');

    // 折叠态 Tooltip 仍可显：完整 root 即显（Popup 无 role="tooltip"，以 data-slot 收敛）
    fireEvent.focus(itemByRoot(FIRST.root));
    const tooltip = await screen.findByText(FIRST.root);
    expect(tooltip.closest('[data-slot="tooltip-content"]') !== null).toBe(true);
  });

  it('Ctrl/Cmd+B：对 window 派发 keyDown（ctrlKey 与 metaKey 两形态）在 collapsed/expanded 间切换（D1 会话内 state）', async () => {
    await restored();

    const sidebar = document.querySelector('[data-slot="sidebar"]')!;
    fireEvent.keyDown(window, { key: 'b', ctrlKey: true });
    expect(sidebar.getAttribute('data-state')).toBe('collapsed');
    fireEvent.keyDown(window, { key: 'b', metaKey: true });
    expect(sidebar.getAttribute('data-state')).toBe('expanded');
    expect(window.localStorage.length).toBe(0);
  });

  it('折叠态不持久化（D1）：卸载重挂回到展开态（新 sidebar provider 会写 sidebar_state cookie 但从不回读，折叠态不被恢复）', async () => {
    await restored();
    const sidebar = document.querySelector('[data-slot="sidebar"]')!;
    fireEvent.keyDown(window, { key: 'b', ctrlKey: true });
    expect(sidebar.getAttribute('data-state')).toBe('collapsed');

    cleanup();
    await restored();
    const sidebarAgain = document.querySelector('[data-slot="sidebar"]')!;
    expect(sidebarAgain.getAttribute('data-state')).toBe('expanded');
    expect(window.localStorage.length).toBe(0);
  });
});

// ---------------------------------------------------------------------------
// 关系四：侧栏 NavLink 页面导航组 → HashRouter 路由表 → 顶层页面切换
// （选中重置语义沿 D10 路由化保留：切回后 /changes 无 :name 段 → 清单呈现、
// get_change_detail 不以旧选中重发）。AgentDebugView 经真实 hooks 挂载
// （挂载不取数），既有清单命令 mock 承载；hash 落点在此核对，完整路由级
// 矩阵归 route_pages.test.tsx。
// ---------------------------------------------------------------------------

describe('App：路由化顶层页面切换（changes | agent | db）', () => {
  beforeEach(() => {
    // 路由化后 App 自含 HashRouter（design D6）：jsdom location 跨用例存活，
    // 上一用例残留的 hash 会改变下一用例启动路由初态，先重置
    window.location.hash = '';
    getVersionMock.mockReset();
    invokeMock.mockReset();
    openMock.mockReset();
    checkMock.mockReset();
    checkMock.mockResolvedValue(null);
    getVersionMock.mockResolvedValue('0.1.0');
    toast.dismiss();
    mockIpc();
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('启动（hash 空）经 / 重定向落 #/changes：清单内容在场、Agent 调试页不在场、nav-changes 激活', async () => {
    await restored();

    expect(window.location.hash).toBe('#/changes');
    expect(screen.getByText('add-feature') !== null).toBe(true);
    expect(screen.queryByTestId('agent-run-form')).toBeNull();
    expect(screen.getByTestId('nav-changes').hasAttribute('data-active')).toBe(true);
    expect(screen.getByTestId('nav-agent').hasAttribute('data-active')).toBe(false);
  });

  it('侧栏点击「Agent 调试」→ hash 落 #/agent、AgentDebugView 呈现、变更页内容卸载；点击「变更」→ hash 回 #/changes 切回清单', async () => {
    await restored();

    fireEvent.click(screen.getByTestId('nav-agent'));
    await waitFor(() => expect(screen.getByTestId('agent-run-form') !== null).toBe(true));
    expect(window.location.hash).toBe('#/agent');
    expect(screen.queryByText('add-feature')).toBeNull();
    expect(screen.getByTestId('nav-agent').hasAttribute('data-active')).toBe(true);

    fireEvent.click(screen.getByTestId('nav-changes'));
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
    expect(window.location.hash).toBe('#/changes');
    expect(screen.queryByTestId('agent-run-form')).toBeNull();
  });

  it('侧栏点击「数据库」→ DbInspectorView 呈现且 db_models 取数发起、其余两分支不挂载（db 分支渲染）', async () => {
    await restored();

    fireEvent.click(screen.getByTestId('nav-db'));
    await waitFor(() => expect(screen.getByTestId('db-model-list') !== null).toBe(true));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('db_models', {
        scope: 'workspace',
        root: FIRST.root,
      }),
    );
    expect(screen.queryByText('add-feature')).toBeNull();
    expect(screen.queryByTestId('agent-run-form')).toBeNull();
    expect(screen.getByTestId('nav-db').hasAttribute('data-active')).toBe(true);
    expect(screen.getByTestId('nav-changes').hasAttribute('data-active')).toBe(false);
    expect(screen.getByTestId('nav-agent').hasAttribute('data-active')).toBe(false);
  });

  it('切至 db 页再切回 changes：清单页重挂重发 list_changes 恰一次、list_workspaces 不增长（清单取数入清单页）', async () => {
    await restored();

    const listChangesBefore = countOf('list_changes');
    const listWorkspacesBefore = countOf('list_workspaces');

    fireEvent.click(screen.getByTestId('nav-db'));
    await waitFor(() => expect(screen.getByTestId('db-model-list') !== null).toBe(true));
    fireEvent.click(screen.getByTestId('nav-changes'));
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));

    // 清单页卸载重挂即重取：list_changes 恰新增一次；workspace 清单仍留 App 层不重发
    expect(countOf('list_changes')).toBe(listChangesBefore + 1);
    expect(countOf('list_workspaces')).toBe(listWorkspacesBefore);
  });

  it('侧栏点击「数据库」→ DbInspectorView 呈现且 db_models 取数发起、其余两分支不挂载（db 分支渲染）', async () => {
    await restored();

    fireEvent.click(screen.getByTestId('nav-db'));
    await waitFor(() => expect(screen.getByTestId('db-model-list') !== null).toBe(true));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('db_models', {
        scope: 'workspace',
        root: FIRST.root,
      }),
    );
    expect(screen.queryByText('add-feature')).toBeNull();
    expect(screen.queryByTestId('agent-run-form')).toBeNull();
    expect(screen.getByTestId('nav-db').hasAttribute('data-active')).toBe(true);
    expect(screen.getByTestId('nav-changes').hasAttribute('data-active')).toBe(false);
    expect(screen.getByTestId('nav-agent').hasAttribute('data-active')).toBe(false);
  });

  it('侧栏点击「Agent 调试」→ agent_sessions 挂载取数携当前 workspace root（返回 [] 数组形态不变）', async () => {
    await restored();

    fireEvent.click(screen.getByTestId('nav-agent'));
    await waitFor(() => expect(screen.getByTestId('agent-run-form') !== null).toBe(true));

    expect(invokeMock).toHaveBeenCalledWith('agent_sessions', {
      root: FIRST.root,
      source: 'debug',
      sourceRef: null,
    });
  });

  it('进入 change 详情后切 Agent 页再切回：hash 落 /changes（无 :name 段）、选中重置回清单、get_change_detail 不以旧选中重发（D10 路由化保留）', async () => {
    await restored();

    fireEvent.click(screen.getByText('add-feature'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('get_change_detail', {
        root: FIRST.root,
        change: 'add-feature',
      }),
    );

    fireEvent.click(screen.getByTestId('nav-agent'));
    await waitFor(() => expect(screen.getByTestId('agent-run-form') !== null).toBe(true));
    fireEvent.click(screen.getByTestId('nav-changes'));
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));

    // 选中重置：URL 无 :name 段 + 清单视图（详情标题不在场），get_change_detail 不重发
    expect(window.location.hash).toBe('#/changes');
    expect(screen.queryByRole('heading', { name: 'add-feature' })).toBeNull();
    expect(countOf('get_change_detail')).toBe(1);
  });

  it('change 清单取数入清单页：切页往返 list_changes 随挂载重发、list_workspaces 仍留 App 层不重发', async () => {
    await restored();

    const workspacesBefore = countOf('list_workspaces');
    const listChangesBefore = countOf('list_changes');
    expect(workspacesBefore).toBeGreaterThan(0);

    fireEvent.click(screen.getByTestId('nav-agent'));
    fireEvent.click(screen.getByTestId('nav-changes'));
    fireEvent.click(screen.getByTestId('nav-agent'));
    await waitFor(() => expect(screen.getByTestId('agent-run-form') !== null).toBe(true));

    expect(countOf('list_workspaces')).toBe(workspacesBefore);
    // /changes 重挂一次即重取一次（清单数据随清单页生命周期，不留 App 层）
    expect(countOf('list_changes')).toBe(listChangesBefore + 1);
    // 重挂取数后清单照常可见
    fireEvent.click(screen.getByTestId('nav-changes'));
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
    expect(countOf('list_changes')).toBe(listChangesBefore + 2);
  });

  it('导航点击不触发任何 workspace 命令（两入口语义不串扰）', async () => {
    await restored();

    const addBefore = countOf('add_workspace');
    const removeBefore = countOf('remove_workspace');
    const listBefore = countOf('list_workspaces');
    fireEvent.click(screen.getByTestId('nav-agent'));
    fireEvent.click(screen.getByTestId('nav-changes'));

    expect(countOf('add_workspace')).toBe(addBefore);
    expect(countOf('remove_workspace')).toBe(removeBefore);
    expect(countOf('list_workspaces')).toBe(listBefore);
  });

  it('root=null（无 workspace）时欢迎屏持有、页面导航不在场（含 nav-db）、不崩', async () => {
    remaining = [];
    render(<App />);

    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));

    expect(screen.queryByTestId('nav-agent')).toBeNull();
    expect(screen.queryByTestId('nav-changes')).toBeNull();
    expect(screen.queryByTestId('nav-db')).toBeNull();
    expect(screen.queryByText('系统工具')).toBeNull();
    expect(screen.queryByTestId('db-model-list')).toBeNull();
    expect(screen.getByText(/还没有记录/) !== null).toBe(true);
    // 清单域取数形态不变：agent_sessions / db_models 零调用，root 无从携带
    expect(countOf('agent_sessions')).toBe(0);
    expect(countOf('db_models')).toBe(0);
  });
});

// ---------------------------------------------------------------------------
// 壳层滚动框架（回溯修订）：壳态路由内容包裹层增 flex min-h-0 flex-col 立高度
// 链起点；其余路由页为内容自适应 flex item，呈现不变。jsdom 以 className 拓扑
// 为代理断言（真实观感归人工验收）。
// ---------------------------------------------------------------------------

describe('App：壳层滚动框架', () => {
  beforeEach(() => {
    // 路由化后 App 自含 HashRouter（design D6）：jsdom location 跨用例存活，
    // 上一用例残留的 hash 会改变下一用例启动路由初态，先重置
    window.location.hash = '';
    getVersionMock.mockReset();
    invokeMock.mockReset();
    openMock.mockReset();
    checkMock.mockReset();
    checkMock.mockResolvedValue(null);
    getVersionMock.mockResolvedValue('0.1.0');
    toast.dismiss();
    mockIpc();
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  /** 壳态路由内容包裹层（SidebarInset 下的 w-full 容器）。 */
  function shellContentWrapper(): Element {
    const inset = document.querySelector('main[data-slot="sidebar-inset"]');
    const wrapper = Array.from(inset?.children ?? []).find(
      (el) => el.tagName === 'DIV' && el.className.includes('w-full'),
    );
    if (!wrapper) throw new Error('壳态路由内容包裹层不存在');
    return wrapper;
  }

  it('壳态路由内容包裹层增 flex min-h-0 flex-col 类（高度链起点），既有留白类不回归', async () => {
    await restored();

    const wrapper = shellContentWrapper();
    for (const className of [
      'mx-auto',
      'flex',
      'min-h-0',
      'w-full',
      'flex-1',
      'flex-col',
      'px-4',
      'py-4',
    ]) {
      expect(wrapper.className).toContain(className);
    }
    // 高度链起点在壳层语义内：包裹层即路由页 flex 容器（AppRoutes 直接子级）
    expect(wrapper.contains(screen.getByText('add-feature'))).toBe(true);
  });

  it('切页往返（changes → db → changes）包裹层框架类稳定，清单 / 数据库页呈现不回归', async () => {
    await restored();

    fireEvent.click(screen.getByTestId('nav-db'));
    await waitFor(() => expect(screen.getByTestId('db-model-list') !== null).toBe(true));
    for (const className of ['flex', 'min-h-0', 'flex-1', 'flex-col']) {
      expect(shellContentWrapper().className).toContain(className);
    }

    fireEvent.click(screen.getByTestId('nav-changes'));
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
    for (const className of ['flex', 'min-h-0', 'flex-1', 'flex-col']) {
      expect(shellContentWrapper().className).toContain(className);
    }
    // 非滚动场景页为内容自适应 flex item：切页往返后清单照常呈现
    expect(screen.getByTestId('nav-changes').hasAttribute('data-active')).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// HashRouter 自含挂载与路由初态（design D2/D6/D7）：render(<App />) 不包任何
// Router 即得路由（D2 收敛点）；启动前预置 hash 验证深链直达与未知路径兜底
// （D7）。深链用例 beforeEach 已重置 hash，此处用例内显式预置。
// ---------------------------------------------------------------------------

describe('App：HashRouter 自含挂载与路由初态（D2/D6/D7）', () => {
  beforeEach(() => {
    // 路由化后 App 自含 HashRouter（design D6）：jsdom location 跨用例存活，
    // 上一用例残留的 hash 会改变下一用例启动路由初态，先重置
    window.location.hash = '';
    getVersionMock.mockReset();
    invokeMock.mockReset();
    openMock.mockReset();
    checkMock.mockReset();
    checkMock.mockResolvedValue(null);
    getVersionMock.mockResolvedValue('0.1.0');
    toast.dismiss();
    mockIpc();
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('壳态启动不包任何 Router 包裹：hash 落 #/changes、清单渲染、sidebar-wrapper / header 在场（D2 自含 HashRouter）', async () => {
    render(<App />);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: FIRST.root }),
    );
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));

    expect(window.location.hash).toBe('#/changes');
    expect(document.querySelector('[data-slot="sidebar-wrapper"]') !== null).toBe(true);
    expect(document.querySelector('header') !== null).toBe(true);
  });

  it('启动前 hash 已为 #/changes/add-feature：直出详情视图并以 URL 参数取数（深链直达 D7，无导航点击）', async () => {
    window.location.hash = '#/changes/add-feature';
    render(<App />);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('get_change_detail', {
        root: FIRST.root,
        change: 'add-feature',
      }),
    );

    await waitFor(() =>
      expect(screen.getByRole('heading', { name: 'add-feature' }) !== null).toBe(true),
    );
    expect(countOf('get_change_detail')).toBe(1);
    // 深链直达不产生行点击之外的清单动作命令
    expect(countOf('add_workspace')).toBe(0);
    expect(countOf('remove_workspace')).toBe(0);
  });

  it('启动前 hash 已为 #/agent：直出 AgentDebugView（agent-run-form 在场）且 nav-agent 呈激活态', async () => {
    window.location.hash = '#/agent';
    render(<App />);

    await waitFor(() => expect(screen.getByTestId('agent-run-form') !== null).toBe(true));

    expect(window.location.hash).toBe('#/agent');
    expect(screen.getByTestId('nav-agent').hasAttribute('data-active')).toBe(true);
    expect(screen.getByTestId('nav-changes').hasAttribute('data-active')).toBe(false);
    // Agent 页不挂变更页：无 detail 取数
    expect(countOf('get_change_detail')).toBe(0);
  });

  it('启动前 hash 为未知路径（#/bogus）：兜底落 #/changes 渲染清单，不空白不崩', async () => {
    window.location.hash = '#/bogus';
    render(<App />);

    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));

    expect(window.location.hash).toBe('#/changes');
    // 落清单而非详情：无详情标题
    expect(screen.queryByRole('heading', { name: 'add-feature' })).toBeNull();
    expect(countOf('get_change_detail')).toBe(0);
  });
});

// ---------------------------------------------------------------------------
// 关系五：/info 基础信息路由（AC-1）——侧栏 nav-info → HashRouter /info 路由项
// → InfoView 真实挂载（AppRoutes / InfoView / useCodeStats 不 mock，进程边界
// 收敛于 invoke 的 code_stats 分支）。路由表为组合声明文件、无独立测试路径，
// 按链路入口挂靠规则以本套件组合用例承载（/info 路由项 + * 兜底回归 +
// 欢迎态隔离）。
// ---------------------------------------------------------------------------

describe('App：/info 基础信息路由可达与欢迎态隔离（AC-1）', () => {
  beforeEach(() => {
    // 路由化后 App 自含 HashRouter（design D6）：jsdom location 跨用例存活，
    // 上一用例残留的 hash 会改变下一用例启动路由初态，先重置
    window.location.hash = '';
    getVersionMock.mockReset();
    invokeMock.mockReset();
    openMock.mockReset();
    checkMock.mockReset();
    checkMock.mockResolvedValue(null);
    getVersionMock.mockResolvedValue('0.1.0');
    toast.dismiss();
    mockIpc();
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('壳态点击 nav-info：hash 落 #/info、基础信息页呈现（info-summary 在场）、变更页内容卸载、nav-info 激活', async () => {
    await restored();

    fireEvent.click(screen.getByTestId('nav-info'));

    await waitFor(() => expect(window.location.hash).toBe('#/info'));
    await waitFor(() => expect(screen.getByTestId('info-summary') !== null).toBe(true));
    expect(screen.queryByText('add-feature')).toBeNull();
    expect(screen.getByTestId('nav-info').hasAttribute('data-active')).toBe(true);
    expect(screen.getByTestId('nav-changes').hasAttribute('data-active')).toBe(false);
    // InfoView 挂载即以 (root, depth=5) 发起解析
    expect(invokeMock).toHaveBeenCalledWith('code_stats', { root: FIRST.root, depth: 5 });
  });

  it('启动前 hash 已为 #/info：深链直出基础信息页且 nav-info 激活（对齐既有深链用例形态）', async () => {
    window.location.hash = '#/info';
    render(<App />);

    await waitFor(() => expect(screen.getByTestId('info-summary') !== null).toBe(true));

    expect(window.location.hash).toBe('#/info');
    expect(screen.getByTestId('nav-info').hasAttribute('data-active')).toBe(true);
    expect(screen.getByTestId('nav-changes').hasAttribute('data-active')).toBe(false);
    expect(countOf('code_stats')).toBe(1);
    // 变更页未挂载：无 change 内容、无详情取数
    expect(screen.queryByText('add-feature')).toBeNull();
    expect(countOf('get_change_detail')).toBe(0);
  });

  it('未知路径兜底回归：#/bogus 仍兜底落 #/changes（/info 插入不破 * 兜底语义）、不误触 code_stats', async () => {
    window.location.hash = '#/bogus';
    render(<App />);

    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));

    expect(window.location.hash).toBe('#/changes');
    expect(screen.queryByTestId('info-view')).toBeNull();
    expect(countOf('code_stats')).toBe(0);
  });

  it('#/info/xyz（路由表无 /info 子段）：兜底落 #/changes，不空白不崩', async () => {
    window.location.hash = '#/info/xyz';
    render(<App />);

    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));

    expect(window.location.hash).toBe('#/changes');
    expect(screen.queryByTestId('info-view')).toBeNull();
    expect(countOf('code_stats')).toBe(0);
  });

  it('欢迎态（root=null）：无壳无 nav-info，#/info 不渲染基础信息页、仅欢迎屏（无 Router 挂壳的隔离）', async () => {
    window.location.hash = '#/info';
    remaining = [];
    render(<App />);

    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));

    expect(document.querySelector('[data-slot="sidebar-wrapper"]')).toBeNull();
    expect(screen.queryByTestId('nav-info')).toBeNull();
    expect(screen.queryByTestId('info-view')).toBeNull();
    expect(screen.getByText(/还没有记录/) !== null).toBe(true);
    expect(countOf('code_stats')).toBe(0);
  });
});

// ---------------------------------------------------------------------------
// 关系六：/config 配置路由（AC-4 / AC-6 / AC-7 / AC-8）——侧栏 nav-config →
// HashRouter /config 路由项 → ConfigView 真实挂载（AppRoutes / ConfigView /
// useWorkspaceConfig 不 mock，进程边界收敛于 invoke 的 workspace_config 分支，
// /info 路由套件先例同型）。路由表为组合声明文件、无独立测试路径，按链路
// 入口挂靠规则以本套件组合用例承载。
// ---------------------------------------------------------------------------

describe('App：/config 配置路由可达与欢迎态隔离（AC-4/AC-6/AC-7）', () => {
  beforeEach(() => {
    // 路由化后 App 自含 HashRouter（design D6）：jsdom location 跨用例存活，
    // 上一用例残留的 hash 会改变下一用例启动路由初态，先重置
    window.location.hash = '';
    getVersionMock.mockReset();
    invokeMock.mockReset();
    openMock.mockReset();
    checkMock.mockReset();
    checkMock.mockResolvedValue(null);
    getVersionMock.mockResolvedValue('0.1.0');
    toast.dismiss();
    mockIpc();
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('壳态点击 nav-config：hash 落 #/config、config-view 在场、nav-config 激活、变更页内容卸载、workspace_config 以 { root: 当前根 } 发起', async () => {
    await restored();

    fireEvent.click(screen.getByTestId('nav-config'));

    await waitFor(() => expect(window.location.hash).toBe('#/config'));
    // config-view 根节点 loading 期即在场：以数据到达后的分区在场为准
    await waitFor(() => expect(screen.getByTestId('config-basic') !== null).toBe(true));
    expect(screen.queryByText('add-feature')).toBeNull();
    expect(screen.getByTestId('nav-config').hasAttribute('data-active')).toBe(true);
    expect(screen.getByTestId('nav-changes').hasAttribute('data-active')).toBe(false);
    expect(invokeMock).toHaveBeenCalledWith('workspace_config', { root: FIRST.root });
  });

  it('启动前 hash 已为 #/config：深链直出配置页且 nav-config 激活，恰发起一次解析（对齐 /info 深链用例形态）', async () => {
    window.location.hash = '#/config';
    render(<App />);

    await waitFor(() => expect(screen.getByTestId('config-basic') !== null).toBe(true));

    expect(window.location.hash).toBe('#/config');
    expect(screen.getByTestId('nav-config').hasAttribute('data-active')).toBe(true);
    expect(countOf('workspace_config')).toBe(1);
    // 变更页未挂载：无 change 内容、无详情取数
    expect(screen.queryByText('add-feature')).toBeNull();
    expect(countOf('get_change_detail')).toBe(0);
  });

  it('未知路径兜底回归：#/bogus 仍兜底落 #/changes（/config 插入不破 * 兜底语义）、不误触 workspace_config', async () => {
    window.location.hash = '#/bogus';
    render(<App />);

    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));

    expect(window.location.hash).toBe('#/changes');
    expect(screen.queryByTestId('config-view')).toBeNull();
    expect(countOf('workspace_config')).toBe(0);
  });

  it('#/config/xyz（路由表无子段）：兜底落 #/changes、不空白不崩', async () => {
    window.location.hash = '#/config/xyz';
    render(<App />);

    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));

    expect(window.location.hash).toBe('#/changes');
    expect(screen.queryByTestId('config-view')).toBeNull();
    expect(countOf('workspace_config')).toBe(0);
  });

  it('欢迎态（root=null）：无壳无 nav-config，#/config 不渲染配置页仅欢迎屏、解析零发起（AC-4 后半）', async () => {
    window.location.hash = '#/config';
    remaining = [];
    render(<App />);

    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));

    expect(document.querySelector('[data-slot="sidebar-wrapper"]')).toBeNull();
    expect(screen.queryByTestId('nav-config')).toBeNull();
    expect(screen.queryByTestId('config-view')).toBeNull();
    expect(screen.getByText(/还没有记录/) !== null).toBe(true);
    expect(countOf('workspace_config')).toBe(0);
  });

  it('workspace_config reject（含无效 root Err reject）：config-error inline 呈现、无 toast 顶替、壳不崩（AC-7 全链核对）', async () => {
    await restored();

    configReject = 'root 无效（C:\\demo\\alpha）：不存在';
    fireEvent.click(screen.getByTestId('nav-config'));

    await waitFor(() => expect(screen.getByTestId('config-error') !== null).toBe(true));
    expect(screen.getByTestId('config-error').textContent).toContain('解析失败');
    expect(screen.getByTestId('config-error').textContent).toContain('root 无效');
    // inline 持久、无 toast 顶替
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    expect(screen.getByTestId('config-error') !== null).toBe(true);
    expect(document.querySelector('[data-sonner-toast]')).toBeNull();

    // 壳不崩：可切回变更页
    fireEvent.click(screen.getByTestId('nav-changes'));
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
  });

  it('切换 workspace（sidebar 清单项）后停留 /config：以新根重发 workspace_config、旧根配置不呈现（AC-6 切换重取的壳层核对）', async () => {
    await restored();

    fireEvent.click(screen.getByTestId('nav-config'));
    await waitFor(() =>
      expect(screen.getByTestId('config-view').textContent).toContain(FIRST.root),
    );

    fireEvent.click(itemByRoot(SECOND.root));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('workspace_config', { root: SECOND.root }),
    );
    await waitFor(() =>
      expect(screen.getByTestId('config-view').textContent).toContain(SECOND.root),
    );
    // 旧根配置不呈现、恰以新旧根各重取一次
    expect(screen.getByTestId('config-view').textContent).not.toContain(FIRST.root);
    const configRoots = invokeMock.mock.calls
      .filter(([name]) => name === 'workspace_config')
      .map(([, params]) => (params as { root: string }).root);
    expect(configRoots).toEqual([FIRST.root, SECOND.root]);
  });
});
