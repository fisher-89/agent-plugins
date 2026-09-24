// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ChangeListState } from '../../hooks/useChangeList';
import type { ChangeDetail, ChangeList } from '../../types/dto';
import { ChangeView } from './ChangeView';

// ---------------------------------------------------------------------------
// ChangeView 组件级单测：选中态路由参数承载（useParams 派生，无本地 state
// 双轨）、行点击 / 返回显式导航（D4：不用 navigate(-1)）、根切换过渡抑制 +
// 落 /changes（D4 单点收口）。
//
// Mock策略（test-design）：useChangeDetail mock 为受控 vi.fn——过渡轮
// (新root, null) 抑制断言需直接观察 hook 入参，mock 为唯一可行观测点（hook
// 本体契约零改动，不在组件级重复其内部行为测试）；MemoryRouter + Routes 为
// 测试装置（非 mock），提供 useParams / useNavigate 上下文并以 initialEntries
// 控制路由参数初态；ChangeListView / ChangeDetailView 不 mock（呈现契约零
// 改动），经 props 回调（onSelect / onBack）触发导航断言、复用既有 DOM 定位。
// ---------------------------------------------------------------------------

const { useChangeDetailMock } = vi.hoisted(() => ({ useChangeDetailMock: vi.fn() }));

vi.mock('./hooks/useChangeDetail', () => ({ useChangeDetail: useChangeDetailMock }));

// ---------------------------------------------------------------------------
// fixture 与装置
// ---------------------------------------------------------------------------

const FIRST = 'C:\\demo\\alpha';
const SECOND = 'C:\\demo\\beta';

const fakeList: ChangeList = {
  active: [
    { name: 'add-feature', source: 'active', inventory: 'v2', created: null, unparsable: false },
    { name: 'beta-fix', source: 'active', inventory: 'v1', created: null, unparsable: false },
  ],
  archiveGroups: [],
};

function detailDto(name: string): ChangeDetail {
  return {
    name,
    source: 'active',
    inventory: 'v2',
    created: null,
    unparsable: false,
    pipeline: [],
    activePhase: null,
    interrupted: [],
    fileLog: [],
    artifacts: [],
  };
}

function listState(): ChangeListState {
  return { data: fakeList, loading: false, error: null, refresh: () => {} };
}

/** 受控 detail 态：已知 change 返回同名 detail；null（清单态/抑制轮）或未知名（ghost 等，对齐后端未命中）返回 detail=null 态 */
function detailStateFor(change: string | null) {
  const known = change !== null && (change === 'add-feature' || change === 'beta-fix');
  return {
    detail: known ? detailDto(change) : null,
    artifacts: [],
    loading: false,
    error: null,
    refresh: () => {},
  };
}

/** URL 探针：把 MemoryRouter 当前 pathname 投影到 DOM 供断言 */
function LocationProbe() {
  const { pathname } = useLocation();
  return <span data-testid="location-probe">{pathname}</span>;
}

function changeViewTree(root: string) {
  return (
    <>
      <Routes>
        <Route path="/changes" element={<ChangeView list={listState()} root={root} />} />
        <Route path="/changes/:name" element={<ChangeView list={listState()} root={root} />} />
      </Routes>
      <LocationProbe />
    </>
  );
}

function renderChangeView(root: string, initialEntry: string) {
  return render(
    <MemoryRouter initialEntries={[initialEntry]}>{changeViewTree(root)}</MemoryRouter>,
  );
}

/** 已挂载视图换根重渲染（根切换场景）：Router 实例保持、location 不重置 */
function rerenderWith(
  view: ReturnType<typeof renderChangeView>,
  root: string,
  initialEntry: string,
) {
  view.rerender(
    <MemoryRouter initialEntries={[initialEntry]}>{changeViewTree(root)}</MemoryRouter>,
  );
}

function probePathname(): string {
  return screen.getByTestId('location-probe').textContent ?? '';
}

function mockUseChangeDetail() {
  useChangeDetailMock.mockReset();
  useChangeDetailMock.mockImplementation((_root: string | null, change: string | null) =>
    detailStateFor(change),
  );
}

describe('ChangeView：路由参数选中态', () => {
  beforeEach(mockUseChangeDetail);

  it('清单路由 /changes：ChangeListView 呈现、useChangeDetail 以 (root, null) 调用恰一次', () => {
    renderChangeView(FIRST, '/changes');

    expect(probePathname()).toBe('/changes');
    expect(screen.getByRole('button', { name: '刷新列表' }) !== null).toBe(true);
    expect(screen.getAllByTestId('change-row')).toHaveLength(2);
    expect(useChangeDetailMock).toHaveBeenCalledTimes(1);
    expect(useChangeDetailMock).toHaveBeenCalledWith(FIRST, null);
  });

  it('详情路由 /changes/:name：ChangeDetailView 呈现、useChangeDetail 以 (root, name) 调用', () => {
    renderChangeView(FIRST, '/changes/add-feature');

    expect(screen.getByRole('heading', { name: 'add-feature' }) !== null).toBe(true);
    expect(screen.getByTestId('detail-header') !== null).toBe(true);
    expect(useChangeDetailMock).toHaveBeenCalledTimes(1);
    expect(useChangeDetailMock).toHaveBeenCalledWith(FIRST, 'add-feature');
  });

  it('未知 change 深链透传（/changes/ghost）：选中态不做校验、以 (root, ghost) 调用，呈现交由 detail 态（降级页）且不崩', () => {
    renderChangeView(FIRST, '/changes/ghost');

    expect(useChangeDetailMock).toHaveBeenCalledTimes(1);
    expect(useChangeDetailMock).toHaveBeenCalledWith(FIRST, 'ghost');
    // detail=null → 既有「未找到该 change。」降级兜底，返回入口在场
    expect(screen.getByText('未找到该 change。') !== null).toBe(true);
    expect(screen.getByRole('button', { name: '← 返回列表' }) !== null).toBe(true);
  });
});

describe('ChangeView：行点击与返回的显式导航', () => {
  beforeEach(mockUseChangeDetail);

  it('清单态点击行 onSelect(beta-fix) → navigate 落 /changes/beta-fix，useChangeDetail 以 URL 参数实时取数', () => {
    renderChangeView(FIRST, '/changes');

    fireEvent.click(screen.getByText('beta-fix'));

    expect(probePathname()).toBe('/changes/beta-fix');
    expect(screen.getByRole('heading', { name: 'beta-fix' }) !== null).toBe(true);
    expect(useChangeDetailMock).toHaveBeenLastCalledWith(FIRST, 'beta-fix');
  });

  it('详情态点击「← 返回列表」→ 显式 navigate 落 /changes 呈现清单（显式导航而非历史回退，D4）', () => {
    renderChangeView(FIRST, '/changes/add-feature');
    expect(screen.getByRole('heading', { name: 'add-feature' }) !== null).toBe(true);

    fireEvent.click(screen.getByRole('button', { name: '← 返回列表' }));

    expect(probePathname()).toBe('/changes');
    expect(screen.getByRole('button', { name: '刷新列表' }) !== null).toBe(true);
    expect(useChangeDetailMock).toHaveBeenLastCalledWith(FIRST, null);
  });

  it('先后选中 A、返回、选中 B：两次 useChangeDetail 参数分别为 A 与 B（URL 参数实时透传，无旧参残留）', () => {
    renderChangeView(FIRST, '/changes');

    fireEvent.click(screen.getByText('add-feature'));
    expect(useChangeDetailMock).toHaveBeenLastCalledWith(FIRST, 'add-feature');

    fireEvent.click(screen.getByRole('button', { name: '← 返回列表' }));
    expect(useChangeDetailMock).toHaveBeenLastCalledWith(FIRST, null);

    fireEvent.click(screen.getByText('beta-fix'));
    expect(probePathname()).toBe('/changes/beta-fix');
    expect(useChangeDetailMock).toHaveBeenLastCalledWith(FIRST, 'beta-fix');
  });
});

describe('ChangeView：根切换过渡抑制与落清单', () => {
  beforeEach(mockUseChangeDetail);

  it('详情态以新 root 重渲染：过渡轮 useChangeDetail 以 (新root, null) 调用（旧名抑制），随后 replace 导航落 /changes 且抑制位清空（后续选择恢复正常取数）', async () => {
    const view = renderChangeView(FIRST, '/changes/add-feature');
    expect(useChangeDetailMock).toHaveBeenCalledWith(FIRST, 'add-feature');

    rerenderWith(view, SECOND, '/changes/add-feature');

    // 导航落点：/changes（URL 无 :name 段）
    await waitFor(() => expect(probePathname()).toBe('/changes'));
    expect(screen.getByRole('button', { name: '刷新列表' }) !== null).toBe(true);

    // 抑制不变量：一旦过渡轮 (新root, null) 出现，落点达成前不再重现
    // (新root, 旧名) 入参（渲染期弃用渲染的入参不提交 effect、不构成取数；
    // 端到端「无新根 + 旧名误发」由 route_pages.test.tsx 的 invoke 记录承接）
    const callsAtLanding = useChangeDetailMock.mock.calls.slice();
    const calls = callsAtLanding as Array<[string, string | null]>;
    const firstSuppressed = calls.findIndex(([root, change]) => root === SECOND && change === null);
    expect(firstSuppressed).toBeGreaterThan(-1);
    expect(
      calls.slice(firstSuppressed).some(([root, change]) => root === SECOND && change !== null),
    ).toBe(false);

    // 抑制位已清空：落清单后正常选详情恢复 (root, selected) 取数
    fireEvent.click(screen.getByText('beta-fix'));
    expect(probePathname()).toBe('/changes/beta-fix');
    expect(useChangeDetailMock).toHaveBeenLastCalledWith(SECOND, 'beta-fix');
  });

  it('清单态（无 :name 段）变更 root：pathname 不变、无导航发生、不产生 (root, 非null) 调用（resetPending 不置位）', async () => {
    const view = renderChangeView(FIRST, '/changes');

    rerenderWith(view, SECOND, '/changes');
    await waitFor(() => expect(useChangeDetailMock).toHaveBeenCalledWith(SECOND, null));

    expect(probePathname()).toBe('/changes');
    expect(screen.getAllByTestId('change-row')).toHaveLength(2);
    expect(useChangeDetailMock.mock.calls.filter(([, change]) => change !== null)).toHaveLength(0);
  });
});
