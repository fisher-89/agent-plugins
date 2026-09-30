import { useEffect, useMemo, useState } from 'react';

import { commands, type AgentInstanceRecord } from '../../../types/generated/bindings';

interface AgentOptionsState {
  /** agent 实例清单（id 升序，选择器选项数据面） */
  instances: AgentInstanceRecord[];
  loading: boolean;
  /** 默认 agent id（`is_default` 记录；无默认回 null——表单保持缺省语义） */
  defaultId: number | null;
}

/**
 * 调试页 agent 选择器取数 hook：挂载 invoke("list_agent_instances") 一次
 * （无轮询、无显式刷新——管理页增删改后重进调试页即得新清单）。清单加载
 * 失败静默为空清单：发起面保持可用，缺省发起由后端解析路径显式报错（错误
 * 横幅呈现），本 hook 不做二次呈现。
 */
export function useAgentOptions(): AgentOptionsState {
  const [instances, setInstances] = useState<AgentInstanceRecord[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    commands
      .listAgentInstances()
      .then((result) => {
        if (cancelled) return;
        setInstances(result);
        setLoading(false);
      })
      .catch(() => {
        if (cancelled) return;
        setInstances([]);
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const defaultId = useMemo(
    () => instances.find((record) => record.isDefault)?.id ?? null,
    [instances],
  );
  return { instances, loading, defaultId };
}
