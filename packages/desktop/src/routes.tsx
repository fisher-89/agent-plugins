import { Navigate, Route, Routes } from 'react-router';

import { AgentDebugView } from './views/agent/agent-debug-view';
import { AgentsView } from './views/agents/agents-view';
import { ChangeDetailView } from './views/changes/change-detail-view';
import { ChangeListView } from './views/changes/change-list-view';
import { ConfigView } from './views/config/config-view';
import { DbInspectorView } from './views/db/db-inspector-view';
import { ExploreView } from './views/explores/explore-view';
import { InfoView } from './views/info/info-view';

/** 壳态路由表：/ 与未知路径 replace 重定向 /changes，顶层页面与 change / explore 选中均由 URL 承载 */
export function AppRoutes({ root }: { root: string }) {
  return (
    <Routes>
      <Route path="/" element={<Navigate replace to="/changes" />} />
      <Route path="/changes" element={<ChangeListView root={root} />} />
      <Route path="/changes/:name" element={<ChangeDetailView root={root} />} />
      <Route path="/info" element={<InfoView root={root} />} />
      <Route path="/config" element={<ConfigView root={root} />} />
      <Route path="/agents" element={<AgentsView />} />
      <Route path="/agent" element={<AgentDebugView root={root} />} />
      <Route path="/explores" element={<ExploreView root={root} />} />
      <Route path="/explores/:name" element={<ExploreView root={root} />} />
      <Route path="/db" element={<DbInspectorView root={root} />} />
      <Route path="*" element={<Navigate replace to="/changes" />} />
    </Routes>
  );
}
