import { useEffect, useState } from 'react';

import { ipc } from '../../../lib/ipc';
import { type ModelCatalog } from '../../../types/generated/bindings';

interface ModelCatalogState {
  catalog: ModelCatalog | null;
  loading: boolean;
}

/**
 * 模型目录取数收口 hook：挂载 invoke("list_model_catalog") 一次（目录是编译期
 * 静态表，重取永远同值——无轮询、无刷新入口）。
 *
 * 目录是**建议面**：失败的正确表现是「没有建议」，不是「页面报错」——故失败只
 * 置 `catalog = null`（不置 error 态、不弹提示、不重试），表单照常可用，五个
 * 组合框退化为纯输入框。
 */
export function useModelCatalog(): ModelCatalogState {
  const [catalog, setCatalog] = useState<ModelCatalog | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    ipc
      .listModelCatalog()
      .then((result) => {
        if (cancelled) return;
        setCatalog(result);
        setLoading(false);
      })
      .catch(() => {
        if (cancelled) return;
        setCatalog(null);
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return { catalog, loading };
}
