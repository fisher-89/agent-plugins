import { useCallback, useEffect, useState } from 'react';

import { commands, type ExploreRecord } from '../../../types/generated/bindings';

export interface ExploreListState {
  /** 探索记录清单（store 按当前 root 过滤，id 升序；root 未就绪/切换过渡轮为空） */
  records: ExploreRecord[];
  loading: boolean;
  error: string | null;
  /** 显式刷新清单 */
  refresh: () => void;
  /** 新话题建档 / 导入绑定共用：invoke 后 refresh */
  create: (name: string) => void;
  /** in-place 改名（保主键保链）：invoke 后 refresh */
  rename: (name: string, newName: string) => void;
  /** 删除记录（不动磁盘文件）：invoke 后 refresh */
  remove: (name: string) => void;
  /** 标题回填（title 回填唯一写口）：invoke 后 refresh；失败向外 rethrow 供行内呈现 */
  updateTitle: (name: string, title: string) => Promise<void>;
  /** 启动变更（promote，move 语义）：invoke 后 refresh；失败向外 rethrow 供行内呈现 */
  promote: (name: string) => Promise<void>;
}

/** 动作面汇总（清单三动作 + 详情页两动作；`...actions` 逐字段即 [`ExploreListState`] 的动作段） */
type ExploreActions = ExploreRecordActions & ExploreDetailActions;

interface ExploreRecordActions {
  create: (name: string) => void;
  rename: (name: string, newName: string) => void;
  remove: (name: string) => void;
}

/** 详情页动作面（title 回填 / promote）：失败向外 rethrow 供详情页行内捕获 */
interface ExploreDetailActions {
  updateTitle: (name: string, title: string) => Promise<void>;
  promote: (name: string) => Promise<void>;
}

/** 清单动作薄封装：invoke 后 refresh；失败落 error 态（不中断页面） */
function useExploreRecordActions(
  root: string | null,
  refresh: () => void,
  onError: (message: string) => void,
): ExploreRecordActions {
  const create = useCallback(
    (name: string) => {
      if (!root) return;
      commands
        .createExploreRecord(root, name)
        .then(refresh)
        .catch((err: unknown) => onError(String(err)));
    },
    [root, refresh, onError],
  );
  const rename = useCallback(
    (name: string, newName: string) => {
      if (!root) return;
      commands
        .renameExploreRecord(root, name, newName)
        .then(refresh)
        .catch((err: unknown) => onError(String(err)));
    },
    [root, refresh, onError],
  );
  const remove = useCallback(
    (name: string) => {
      if (!root) return;
      commands
        .deleteExploreRecord(root, name)
        .then(refresh)
        .catch((err: unknown) => onError(String(err)));
    },
    [root, refresh, onError],
  );
  return { create, rename, remove };
}

/**
 * 详情页动作薄封装：失败既落清单 error 态也向外 rethrow（详情页行内错误块
 * 捕获后端 Err 原文）；成功 refresh 清单（清单与详情头 title / promoted 态
 * 一致）。
 */
function useExploreDetailActions(
  root: string | null,
  refresh: () => void,
  onError: (message: string) => void,
): ExploreDetailActions {
  const updateTitle = useCallback(
    async (name: string, title: string) => {
      if (!root) return;
      try {
        await commands.updateExploreTitle(root, name, title);
        refresh();
      } catch (err: unknown) {
        onError(String(err));
        throw err;
      }
    },
    [root, refresh, onError],
  );
  const promote = useCallback(
    async (name: string) => {
      if (!root) return;
      try {
        await commands.promoteExplore(root, name);
        refresh();
      } catch (err: unknown) {
        onError(String(err));
        throw err;
      }
    },
    [root, refresh, onError],
  );
  return { updateTitle, promote };
}

/** 动作面组合：清单三动作 + 详情页两动作（返回面即 [`ExploreActions`]） */
function useExploreActions(
  root: string | null,
  refresh: () => void,
  onError: (message: string) => void,
): ExploreActions {
  return {
    ...useExploreRecordActions(root, refresh, onError),
    ...useExploreDetailActions(root, refresh, onError),
  };
}

/**
 * 探索清单 hook：root 变更与显式动作触发取数（invoke("list_explore_records")），
 * 无轮询。清单数据带归属 root 标记——root 切换的过渡轮不呈现旧根记录（抑制
 * 动作封装为薄封装（invoke 后 refresh）；组件不直接 invoke。
 */
export function useExploreList(root: string | null): ExploreListState {
  const [records, setRecords] = useState<ExploreRecord[]>([]);
  const [recordsRoot, setRecordsRoot] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  const refresh = useCallback(() => setTick((t) => t + 1), []);
  const onError = useCallback((message: string) => setError(message), []);
  const actions = useExploreActions(root, refresh, onError);

  useEffect(() => {
    if (!root) {
      setRecords([]);
      setRecordsRoot(null);
      setError(null);
      setLoading(false);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    commands
      .listExploreRecords(root)
      .then((result) => {
        if (cancelled) return;
        setRecords(result);
        setRecordsRoot(root);
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
  }, [root, tick]);

  return {
    records: recordsRoot === root ? records : [],
    loading,
    error,
    refresh,
    ...actions,
  };
}
