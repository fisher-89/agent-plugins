import { FilePlusCornerIcon, PlusIcon } from 'lucide-react';
import { useCallback } from 'react';
import { useNavigate } from 'react-router';

import { TextTooltip } from '@/components/standard/tooltip';
import { Button } from '@/components/ui/button';

import { groupExploreRecords } from '../../lib/explore-groups';
import type { ExploreRecord } from '../../types/dto';
import { ExploreCreateDialog } from './components/explore-create-dialog';
import { ExploreOpenFile } from './components/explore-open-file';
import { useExploreRouteContext } from './explore-layout';
import type { ExploreListState } from './hooks/use-explore-list';

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

/** 新建 / 导入两个入口的图标触发器（base-ui 触发 props 由消费方原样落地） */
function ExploreActions({
  root,
  onCreated,
}: {
  root: string;
  onCreated: (name: string, autoPrompt: string | null) => void;
}): React.JSX.Element {
  return (
    <div className="flex justify-end">
      <ExploreCreateDialog
        root={root}
        onCreated={onCreated}
        trigger={({ onClick }) => (
          <TextTooltip text="创建探索">
            <Button
              data-testid="explore-create-open"
              variant="outline"
              size="icon"
              aria-label="创建探索"
              onClick={onClick}
            >
              <PlusIcon />
            </Button>
          </TextTooltip>
        )}
      />
      <ExploreOpenFile
        root={root}
        onCreated={onCreated}
        trigger={(triggerProps) => (
          <TextTooltip text="导入探索文档">
            <Button
              {...triggerProps}
              data-testid="explore-import-open"
              variant="outline"
              size="icon"
              aria-label="导入探索文档"
            >
              <FilePlusCornerIcon />
            </Button>
          </TextTooltip>
        )}
      />
    </div>
  );
}

/** 清单数据区：分组派生 + 加载 / 错误 / 空态三态（渲染期纯派生，零 useMemo） */
function ListBody({
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
  const groups = groupExploreRecords(list.records);
  return (
    <div data-testid="explore-list">
      <ExploreActions onCreated={onCreated} root={root} />
      <section className="mt-2 rounded-lg border border-border bg-card px-4 py-3.5">
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
 * 探索清单页（路由 `/explores`，清单状态取自路由布局）：清单按当前 workspace
 * 过滤（非实时扫目录），新建成功后携 `state.autoPrompt` 导航进详情页。
 */
export function ExploreListView({ root }: { root: string }): React.JSX.Element {
  const { list } = useExploreRouteContext();
  const navigate = useNavigate();
  const openExplore = useCallback((target: string) => navigate(`/explores/${target}`), [navigate]);
  const onCreated = useCallback(
    (created: string, autoPrompt: string | null) => {
      list.refresh(); // 对话框直连建档，清单经此显式刷新（布局持有，导航后即见）
      void navigate(
        `/explores/${created}`,
        autoPrompt === null ? undefined : { state: { autoPrompt } },
      );
    },
    [list.refresh, navigate],
  );
  return <ListBody list={list} onCreated={onCreated} onOpen={openExplore} root={root} />;
}
