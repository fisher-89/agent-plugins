import { useCallback, useEffect, useRef, useState } from 'react';
import { useNavigate, useParams } from 'react-router';

import { groupExploreRecords } from '../../lib/explore-groups';
import type { ExploreRecord } from '../../types/dto';
import { ExploreCreateDialog } from './components/explore-create-dialog';
import { ExploreDetailView } from './explore-detail-view';
import { useExploreList, type ExploreListState } from './hooks/use-explore-list';

/** 清单条目：标题（title，恒非空）+ 建档时间 + 删除入口（删记录不动磁盘文件） */
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
        <span className="block truncate text-sm" data-testid="explore-item-title">
          {record.title}
        </span>
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

/** 清单分组：组标题（组名 + 计数，层级较外层「探索清单」`<h2>` 降一级）+ 条目列表；
 * 组内为空呈现占位文案（不以组缺席表达空集） */
function ExploreGroup({
  groupTestId,
  label,
  records,
  emptyText,
  onOpen,
  onRemove,
}: {
  groupTestId: string;
  label: string;
  records: ExploreRecord[];
  emptyText: string;
  onOpen: (name: string) => void;
  onRemove: (name: string) => void;
}): React.JSX.Element {
  return (
    <section className="mb-3 last:mb-0" data-testid={groupTestId}>
      <h3 className="m-0 mb-2 text-[13px]">
        {label} <span className="text-muted-foreground">({records.length})</span>
      </h3>
      {records.length === 0 ? (
        <div className="text-muted-foreground">{emptyText}</div>
      ) : (
        <ul className="m-0 list-none p-0">
          {records.map((record) => (
            <ExploreListItem key={record.id} onOpen={onOpen} onRemove={onRemove} record={record} />
          ))}
        </ul>
      )}
    </section>
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
  onCreated: (name: string, autoPrompt: string | null) => void;
}): React.JSX.Element {
  // 渲染期纯派生：分组 + 组内按建档时间倒序（零 useMemo / 不进 hook）
  const groups = groupExploreRecords(list.records);
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
        {list.records.length > 0 && (
          <div data-testid="explore-items">
            <ExploreGroup
              emptyText="无进行中的探索。"
              groupTestId="explore-group-active"
              label="进行中"
              onOpen={onOpen}
              onRemove={list.remove}
              records={groups.active}
            />
            <ExploreGroup
              emptyText="暂无已转变更。"
              groupTestId="explore-group-promoted"
              label="已转变更"
              onOpen={onOpen}
              onRemove={list.remove}
              records={groups.promoted}
            />
          </div>
        )}
      </section>
    </div>
  );
}

/**
 * 探索视图：/explores（清单）与 /explores/:name（详情）共用；选中记录由路由
 * 参数承载（useParams 派生，无本地 state 双轨），记录对象从清单三级派生——
 * URL 名 find → miss 时以 `lastRecordRef` 快照 id 在清单重寻（id 桥：in-place
 * 改名保 id，旧名寻址不再命中）→ 仍 miss 以快照本体兜底（MUST NOT 闪现
 * MissingRecord）。workspace 根切换（select / 移除当前根 / 添加新根）时若带
 * 旧选中，照既有 resetPending 流程落清单。
 */
export function ExploreView({ root }: { root: string }): React.JSX.Element {
  const { name } = useParams<'name'>();
  const navigate = useNavigate();
  const selected = name ?? null;
  const [prevRoot, setPrevRoot] = useState(root);
  const [resetPending, setResetPending] = useState(false);
  const list = useExploreList(root);
  // id 桥快照：最近一次非空派生记录（渲染期缓存维护）——记录 in-place 改名后
  // 按旧名寻址 miss 时，以快照 id 在清单重寻；清单未就绪 / 仍 miss 以快照本体
  // 兜底（页面不闪 MissingRecord）
  const lastRecordRef = useRef<ExploreRecord | null>(null);

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
    (created: string, autoPrompt: string | null) => {
      list.refresh(); // 对话框直连建档，清单经此显式刷新
      void navigate(
        `/explores/${created}`,
        autoPrompt === null ? undefined : { state: { autoPrompt } },
      );
    },
    [list.refresh, navigate],
  );

  let record: ExploreRecord | null = null;
  if (selected !== null && !resetPending) {
    const byName = list.records.find((item) => item.name === selected);
    if (byName !== undefined) {
      record = byName;
    } else if (lastRecordRef.current !== null) {
      const snapshot = lastRecordRef.current;
      record = list.records.find((item) => item.id === snapshot.id) ?? snapshot;
    }
  }
  if (record !== null) {
    lastRecordRef.current = record; // 派生非空时渲染期刷新快照
  }

  return selected === null || resetPending ? (
    <ExploreListView list={list} onCreated={onCreated} onOpen={openExplore} root={root} />
  ) : (
    <ExploreDetailView list={list} record={record} root={root} />
  );
}
