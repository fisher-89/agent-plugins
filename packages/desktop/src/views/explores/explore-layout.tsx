import { Outlet, useOutletContext } from 'react-router';

import { useExploreList, type ExploreListState } from './hooks/use-explore-list';

/** 探索路由上下文：清单由布局单源持有，清单页与详情页共享 */
export interface ExploreRouteContext {
  list: ExploreListState;
}

/** 取布局持有的清单状态（仅可在 `/explores` 路由子树内调用） */
export function useExploreRouteContext(): ExploreRouteContext {
  return useOutletContext<ExploreRouteContext>();
}

/**
 * 探索路由布局（`/explores` 父路由）：仅持有清单状态并下传路由上下文——清单页
 * 与详情页之间切换不重取清单、不闪缺失态（两页同源）；页面本体各自成路由。
 */
export function ExploreLayout({ root }: { root: string }): React.JSX.Element {
  const list = useExploreList(root);
  return <Outlet context={{ list } satisfies ExploreRouteContext} />;
}
