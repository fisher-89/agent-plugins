import { useCallback, useEffect, useState } from 'react';
import { useNavigate, useParams } from 'react-router';

import type { ExploreRecord } from '../../types/dto';
import { ExploreCreateDialog } from './components/ExploreCreateDialog';
import { ExploreDetailView } from './ExploreDetailView';
import { useExploreList, type ExploreListState } from './hooks/useExploreList';

/** 清单条目：名称 + 建档时间 + 删除入口（删记录不动磁盘文件） */
function ExploreListItem({
  record,
  onOpen,
  onRemove,
}: {
  record: ExploreRecord;
  onOpen: (name: string) => void;
  onRemove: (name: string) => void;
}): React.JSX.Element {
  return (
    <li className="flex items-center gap-2 border-b border-b-border py-2 last:border-b-0">
      <button
        type="button"
        className="min-w-0 flex-1 cursor-pointer border-0 bg-transparent p-0 text-left hover:text-primary"
        data-testid="explore-item"
        data-name={record.name}
        onClick={() => onOpen(record.name)}
      >
        <span className="block truncate text-sm">{record.name}</span>
        <span className="block text-xs text-muted-foreground">
          {new Date(record.createdAt).toLocaleString()}
        </span>
      </button>
      <button
        type="button"
        className="cursor-pointer border-0 bg-transparent p-0 text-xs text-muted-foreground underline hover:text-fail"
        data-testid="explore-item-delete"
        onClick={() => onRemove(record.name)}
      >
        删除
      </button>
    </li>
  );
}

/** 清单页：store 清单（按当前 workspace 过滤）+ 新建入口，非实时扫目录 */
function ExploreListView({
  list,
  root,
  onOpen,
  onCreated,
}: {
  list: ExploreListState;
  root: string;
  onOpen: (name: string) => void;
  onCreated: (name: string) => void;
}): React.JSX.Element {
  return (
    <div data-testid="explore-list">
      <ExploreCreateDialog root={root} onCreated={onCreated} />
      <section className="rounded-lg border border-border bg-card px-4 py-3.5">
        <h2 className="m-0 mb-2 text-[15px]">探索清单</h2>
        {list.error !== null && (
          <div
            className="mb-3 break-all rounded-md bg-fail-bg px-3 py-2 text-fail"
            data-testid="explore-list-error"
          >
            清单加载失败：{list.error}
          </div>
        )}
        {list.loading && (
          <div className="text-muted-foreground" data-testid="explore-list-loading">
            加载中…
          </div>
        )}
        {!list.loading && list.records.length === 0 && list.error === null && (
          <div className="text-muted-foreground" data-testid="explore-list-empty">
            当前 workspace 暂无探索记录。新建一个话题，或从已有文档导入。
          </div>
        )}
        <ul className="m-0 list-none p-0" data-testid="explore-items">
          {list.records.map((record) => (
            <ExploreListItem
              key={record.id}
              onOpen={onOpen}
              onRemove={list.remove}
              record={record}
            />
          ))}
        </ul>
      </section>
    </div>
  );
}

/**
 * 探索视图：/explores（清单）与 /explores/:name（详情）共用；选中记录由路由
 * 参数承载（useParams 派生，无本地 state 双轨），记录对象从清单按名派生。
 * workspace 根切换（select / 移除当前根 / 添加新根）时若带旧选中，照
 * ChangeView 模式：过渡轮抑制误发查询 → replace 导航回 /explores → 清抑制位。
 */
export function ExploreView({ root }: { root: string }): React.JSX.Element {
  const { name } = useParams<'name'>();
  const navigate = useNavigate();
  const selected = name ?? null;
  const [prevRoot, setPrevRoot] = useState(root);
  const [resetPending, setResetPending] = useState(false);
  const list = useExploreList(root);

  // 渲染期调整（照 ChangeView）：根切换且带旧选中 → 置待导航标记；清位不可
  // 早于 navigate 过渡提交，否则留出「新根 + 旧名」中间提交重新武装查询
  if (prevRoot !== root) {
    setPrevRoot(root);
    if (selected !== null) setResetPending(true);
  }
  if (resetPending && selected === null) {
    setResetPending(false);
  }

  useEffect(() => {
    if (resetPending) {
      void navigate('/explores', { replace: true }); // workspace 切换落清单
    }
  }, [resetPending, navigate]);

  const openExplore = useCallback((target: string) => navigate(`/explores/${target}`), [navigate]);
  const onCreated = useCallback(
    (created: string) => {
      list.refresh(); // 对话框直连建档，清单经此显式刷新
      void openExplore(created);
    },
    [list.refresh, openExplore],
  );

  const record =
    selected !== null && !resetPending
      ? (list.records.find((item) => item.name === selected) ?? null)
      : null;

  return selected === null || resetPending ? (
    <ExploreListView list={list} onCreated={onCreated} onOpen={openExplore} root={root} />
  ) : (
    <ExploreDetailView record={record} root={root} />
  );
}
