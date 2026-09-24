import { Navigate, Route, Routes } from 'react-router';

import { type ChangeListState } from './hooks/useChangeList';
import { AgentDebugView } from './views/agent/AgentDebugView';
import { ChangeView } from './views/changes/ChangeView';

/** 壳态路由表：/ 与未知路径 replace 重定向 /changes，顶层页面与 change 选中均由 URL 承载 */
export function AppRoutes({ root, list }: { root: string; list: ChangeListState }) {
  return (
    <Routes>
      <Route path="/" element={<Navigate replace to="/changes" />} />
      <Route path="/changes" element={<ChangeView list={list} root={root} />} />
      <Route path="/changes/:name" element={<ChangeView list={list} root={root} />} />
      <Route path="/agent" element={<AgentDebugView root={root} />} />
      <Route path="*" element={<Navigate replace to="/changes" />} />
    </Routes>
  );
}
