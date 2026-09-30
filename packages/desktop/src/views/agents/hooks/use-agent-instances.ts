import { useCallback, useEffect, useState } from 'react';
import { toast } from 'sonner';

import {
  commands,
  type AgentEngineKind,
  type AgentInstanceRecord,
} from '../../../types/generated/bindings';

/** 动作失败 toast 固定前缀（文案供断言；remove 的 store miss 幂等非错误不 toast） */
const ACTION_ERROR_PREFIX = {
  save: '保存 agent 失败：',
  remove: '删除 agent 失败：',
  setDefault: '设置默认 agent 失败：',
} as const;

/** save 动作入参（id null 新建 / number 更新；sdk 未选 provider 由后端校验 reject） */
interface AgentInstanceSaveInput {
  id: number | null;
  name: string;
  engine: AgentEngineKind;
  providerId: number | null;
}

export interface AgentInstancesState {
  instances: AgentInstanceRecord[];
  loading: boolean;
  /**
   * 仅承载 list_agent_instances 加载失败（双轨语义：save/remove/setDefault
   * 动作失败在 hook 内直调 toast.error 呈现，不置本字段）。
   */
  error: string | null;
  save: (input: AgentInstanceSaveInput) => Promise<AgentInstanceRecord | null>;
  remove: (id: number) => Promise<boolean>;
  /** 标记即切换（旧默认自动清除），成功刷新清单 */
  setDefault: (id: number) => Promise<AgentInstanceRecord | null>;
}

/**
 * agent 清单取数收口 hook：挂载自动 invoke("list_agent_instances") 一次，
 * 此后由动作轨道刷新（显式动作刷新，无轮询）。错误呈现双轨（沿
 * use-agent-providers 同口径）：查询轨道加载失败置 error 态 inline 持久；
 * 动作轨道失败 hook 内直调 toast.error（固定前缀 + String(err)），不置
 * error 态。remove 返回 false（store miss）为幂等非错误。
 */
export function useAgentInstances(): AgentInstancesState {
  const [instances, setInstances] = useState<AgentInstanceRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  const refresh = useCallback(() => setTick((t) => t + 1), []);
  const { save, remove, setDefault } = useAgentInstancesActions(refresh);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    commands
      .listAgentInstances()
      .then((result) => {
        if (cancelled) return;
        setInstances(result);
        setLoading(false);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setError(String(err));
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [tick]);

  return { instances, loading, error, save, remove, setDefault };
}

/** 动作轨道：失败直调 toast.error（不置 error 态），返回值仅供流程控制 */
function useAgentInstancesActions(refresh: () => void) {
  const save = useCallback(
    async (input: AgentInstanceSaveInput): Promise<AgentInstanceRecord | null> => {
      try {
        const record = await commands.saveAgentInstance(
          input.id,
          input.name,
          input.engine,
          input.providerId,
        );
        refresh();
        return record;
      } catch (err: unknown) {
        toast.error(`${ACTION_ERROR_PREFIX.save}${String(err)}`);
        return null;
      }
    },
    [refresh],
  );

  const remove = useCallback(
    async (id: number): Promise<boolean> => {
      try {
        const hit = await commands.deleteAgentInstance(id);
        refresh();
        return hit;
      } catch (err: unknown) {
        toast.error(`${ACTION_ERROR_PREFIX.remove}${String(err)}`);
        return false;
      }
    },
    [refresh],
  );

  const setDefault = useCallback(
    async (id: number): Promise<AgentInstanceRecord | null> => {
      try {
        const record = await commands.setDefaultAgentInstance(id);
        refresh();
        return record;
      } catch (err: unknown) {
        toast.error(`${ACTION_ERROR_PREFIX.setDefault}${String(err)}`);
        return null;
      }
    },
    [refresh],
  );

  return { save, remove, setDefault };
}
