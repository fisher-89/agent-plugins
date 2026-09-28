import { Navigate, Route, Routes } from 'react-router';

import { type ChangeListState } from './hooks/use-change-list';
import { AgentDebugView } from './views/agent/agent-debug-view';
import { ChangeView } from './views/changes/change-view';
import { DbInspectorView } from './views/db/db-inspector-view';
import { ExploreView } from './views/explores/explore-view';

/** 壳态路由表：/ 与未知路径 replace 重定向 /changes，顶层页面与 change / explore 选中均由 URL 承载 */
export function AppRoutes({ root, list }: { root: string; list: ChangeListState }) {
  return (
    <Routes>
      <Route path="/" element={<Navigate replace to="/changes" />} />
      <Route path="/changes" element={<ChangeView list={list} root={root} />} />
      <Route path="/changes/:name" element={<ChangeView list={list} root={root} />} />
      <Route path="/agent" element={<AgentDebugView root={root} />} />
      <Route path="/explores" element={<ExploreView root={root} />} />
      <Route path="/explores/:name" element={<ExploreView root={root} />} />
      <Route path="/db" element={<DbInspectorView />} />
      <Route path="*" element={<Navigate replace to="/changes" />} />
    </Routes>
  );
}
