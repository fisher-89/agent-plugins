import { useCallback, useEffect, useState } from 'react';
import { toast } from 'sonner';

import {
  commands,
  type AgentModelTiers,
  type AgentProviderRecord,
} from '../../../types/generated/bindings';

/** 动作失败 toast 固定前缀（文案供断言；remove 的 store miss 幂等非错误不 toast） */
const ACTION_ERROR_PREFIX = {
  save: '保存 provider 失败：',
  remove: '删除 provider 失败：',
} as const;

/** save 动作入参（id null 新建 / number 更新；apiKey 留空 = 保持原值，由后端回填；
 * contextLength 留空 = 未配置存 null，MUST NOT 落 0 / 128000 缺省字面） */
interface AgentProviderSaveInput {
  id: number | null;
  name: string;
  baseUrl: string;
  apiKey: string;
  models: AgentModelTiers;
  contextLength: number | null;
}

export interface AgentProvidersState {
  providers: AgentProviderRecord[];
  loading: boolean;
  /**
   * 仅承载 list_agent_providers 加载失败（双轨语义：save/remove 动作失败在
   * hook 内直调 toast.error 呈现，不置本字段）。
   */
  error: string | null;
  save: (input: AgentProviderSaveInput) => Promise<AgentProviderRecord | null>;
  remove: (id: number) => Promise<boolean>;
}

/**
 * provider 清单取数收口 hook：挂载自动 invoke("list_agent_providers") 一次，
 * 此后由动作轨道刷新（显式动作刷新，无轮询）。错误呈现双轨（沿 use-workspaces
 * 口径）：查询轨道加载失败置 error 态 inline 持久；动作轨道 save/remove 失败
 * hook 内直调 toast.error（固定前缀 + String(err)），不置 error 态，返回值
 * （null / false）仅供流程控制。remove 返回 false（store miss）为幂等非错误：
 * 无 error、无 toast。
 */
export function useAgentProviders(): AgentProvidersState {
  const [providers, setProviders] = useState<AgentProviderRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  const refresh = useCallback(() => setTick((t) => t + 1), []);
  const { save, remove } = useAgentProvidersActions(refresh);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    commands
      .listAgentProviders()
      .then((result) => {
        if (cancelled) return;
        setProviders(result);
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

  return { providers, loading, error, save, remove };
}

/** 动作轨道：失败直调 toast.error（不置 error 态），返回值仅供流程控制 */
function useAgentProvidersActions(refresh: () => void) {
  const save = useCallback(
    async (input: AgentProviderSaveInput): Promise<AgentProviderRecord | null> => {
      try {
        const record = await commands.saveAgentProvider(
          input.id,
          input.name,
          input.baseUrl,
          input.apiKey,
          input.models,
          input.contextLength,
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
        const hit = await commands.deleteAgentProvider(id);
        refresh();
        return hit;
      } catch (err: unknown) {
        toast.error(`${ACTION_ERROR_PREFIX.remove}${String(err)}`);
        return false;
      }
    },
    [refresh],
  );

  return { save, remove };
}
