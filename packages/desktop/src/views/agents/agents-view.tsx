import { AgentPanel } from './components/agent-panel';
import { ProviderPanel } from './components/provider-panel';
import { useAgentInstances } from './hooks/use-agent-instances';
import { useAgentProviders } from './hooks/use-agent-providers';
import { useModelCatalog } from './hooks/use-model-catalog';

/**
 * Agent 管理页（/agents，全局语义）：单页上下两段——上段 Providers
 * （连接档案 CRUD）+ 下段 Agents（引擎实例 CRUD + 默认标记切换）。不依赖 workspace
 * root、不触发任何 workspace 命令（数据面为全局库 user 维度）；取数收在
 * 三取数 hook（清单挂载 invoke 一次 + 动作轨道刷新，模型目录挂载一次，均无
 * 轮询）。目录取数收口在视图根、以 `catalog` 下传面板——面板保持无取数职责。
 */
export function AgentsView(): React.JSX.Element {
  const providersState = useAgentProviders();
  const instancesState = useAgentInstances();
  const catalogState = useModelCatalog();
  return (
    <div className="flex flex-col gap-3" data-testid="agents-view">
      <h2 className="m-0 text-[15px]">Agent 管理</h2>
      <div className="flex flex-col gap-3">
        <ProviderPanel catalog={catalogState.catalog} state={providersState} />
        <AgentPanel providers={providersState.providers} state={instancesState} />
      </div>
    </div>
  );
}
