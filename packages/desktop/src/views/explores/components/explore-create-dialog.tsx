import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useState } from 'react';

import type { ExploreScanEntry } from '../../../types/dto';

export interface ExploreCreateDialogProps {
  /** 当前 workspace root（扫描与建档入参） */
  root: string;
  /** 成功建档后回调（父层 refresh 清单并导航进详情） */
  onCreated: (name: string) => void;
}

type TabKey = 'import' | 'topic';

/** 单条导入项：stem + 修改时间，点击即建档绑定 */
function ImportItem({
  entry,
  root,
  onCreated,
  onError,
}: {
  entry: ExploreScanEntry;
  root: string;
  onCreated: (name: string) => void;
  onError: (message: string) => void;
}): React.JSX.Element {
  return (
    <button
      type="button"
      className="block w-full cursor-pointer border-0 border-b border-b-border bg-transparent px-0 py-1.5 text-left last:border-b-0 hover:text-primary"
      data-testid="explore-import-item"
      onClick={() => {
        invoke<unknown>('create_explore_record', { root, name: entry.name })
          .then(() => onCreated(entry.name))
          .catch((err: unknown) => onError(String(err)));
      }}
    >
      {entry.name}
      {entry.modifiedAt !== null && (
        <span className="ml-2 text-xs text-muted-foreground">
          {new Date(entry.modifiedAt).toLocaleString()}
        </span>
      )}
    </button>
  );
}

/** 扫描结果列表：未绑定 `*.md`（已绑定已在命令层求差滤除），点击即建档 */
function ImportList({
  root,
  onCreated,
  onError,
}: ExploreCreateDialogProps & { onError: (message: string) => void }): React.JSX.Element {
  const [entries, setEntries] = useState<ExploreScanEntry[] | null>(null);

  useEffect(() => {
    let cancelled = false;
    invoke<ExploreScanEntry[]>('scan_explores', { root })
      .then((result) => {
        if (!cancelled) setEntries(result);
      })
      .catch((err: unknown) => {
        if (!cancelled) onError(String(err));
      });
    return () => {
      cancelled = true;
    };
  }, [root, onError]);

  if (entries === null) {
    return <div className="text-muted-foreground">扫描中…</div>;
  }
  if (entries.length === 0) {
    return (
      <div className="text-muted-foreground" data-testid="explore-import-empty">
        没有可导入的未绑定文档。
      </div>
    );
  }
  return (
    <>
      {entries.map((entry) => (
        <ImportItem
          entry={entry}
          key={entry.name}
          onCreated={onCreated}
          onError={onError}
          root={root}
        />
      ))}
    </>
  );
}

/** 「从已有文档创建」入口：导入扫描 + 选中建档（展示名 = stem） */
function ImportEntry({ root, onCreated }: ExploreCreateDialogProps): React.JSX.Element {
  const [error, setError] = useState<string | null>(null);
  const onError = useCallback((message: string) => setError(message), []);
  return (
    <div data-testid="explore-import-entry">
      <div className="mb-1.5 text-xs text-muted-foreground">
        选择笔记目录中尚未绑定的文档，建档后可在页面内继续探索：
      </div>
      {error !== null && (
        <div
          className="mb-2 break-all rounded-md bg-fail-bg px-2 py-1.5 text-xs text-fail"
          data-testid="explore-create-error"
        >
          {error}
        </div>
      )}
      <ImportList onError={onError} onCreated={onCreated} root={root} />
    </div>
  );
}

/** 「新话题」入口：仅建档不落盘文件（内容唯一真源在磁盘，agent 会话流程懒创建） */
function TopicEntry({ root, onCreated }: ExploreCreateDialogProps): React.JSX.Element {
  const [topic, setTopic] = useState('');
  const [error, setError] = useState<string | null>(null);

  const create = () => {
    setError(null);
    invoke<unknown>('create_explore_record', { root, name: topic.trim() })
      .then(() => onCreated(topic.trim()))
      .catch((err: unknown) => setError(String(err)));
  };

  return (
    <div data-testid="explore-topic-entry">
      <label className="mb-1 block text-xs text-muted-foreground" htmlFor="explore-topic-name">
        主题名（kebab-case；仅建档，笔记文件由 agent 会话按需落盘）
      </label>
      <input
        id="explore-topic-name"
        className="mb-2 block w-full rounded-md border border-border bg-transparent px-2.5 py-1.5 text-sm outline-none focus:ring-1 focus:ring-ring"
        data-testid="explore-topic-name"
        value={topic}
        onChange={(e) => setTopic(e.target.value)}
        placeholder="例如 api-retry-strategy"
      />
      {error !== null && (
        <div
          className="mb-2 break-all rounded-md bg-fail-bg px-2 py-1.5 text-xs text-fail"
          data-testid="explore-create-error"
        >
          {error}
        </div>
      )}
      <button
        type="button"
        className="cursor-pointer rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground outline-none transition-colors hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50"
        data-testid="explore-topic-create"
        disabled={topic.trim().length === 0}
        onClick={create}
      >
        建档
      </button>
    </div>
  );
}

/** 入口切换条：新话题 / 从已有文档创建 */
function EntryTabs({
  tab,
  onSelect,
}: {
  tab: TabKey;
  onSelect: (key: TabKey) => void;
}): React.JSX.Element {
  return (
    <div className="mt-2 mb-3 flex gap-2" data-testid="explore-create-tabs">
      <button
        type="button"
        className={`cursor-pointer rounded-md border border-border px-2.5 py-1 text-sm ${tab === 'topic' ? 'bg-muted font-medium' : 'bg-transparent'}`}
        data-testid="explore-tab-topic"
        onClick={() => onSelect('topic')}
      >
        新话题
      </button>
      <button
        type="button"
        className={`cursor-pointer rounded-md border border-border px-2.5 py-1 text-sm ${tab === 'import' ? 'bg-muted font-medium' : 'bg-transparent'}`}
        data-testid="explore-tab-import"
        onClick={() => onSelect('import')}
      >
        从已有文档创建
      </button>
    </div>
  );
}

/**
 * 新建对话框（两入口）：「从已有文档创建」经 scan_explores 列未绑定文件
 * （已绑定以 store 清单求差滤除），选中即建档；「新话题」输入主题名建档
 * （不落盘文件）。成功后 refresh 清单并导航进详情由父层 onCreated 承担。
 */
export function ExploreCreateDialog({
  root,
  onCreated,
}: ExploreCreateDialogProps): React.JSX.Element {
  const [open, setOpen] = useState(false);
  const [tab, setTab] = useState<TabKey>('topic');
  const toggle = useCallback(() => setOpen((prev) => !prev), []);
  const selectTab = useCallback((key: TabKey) => setTab(key), []);

  return (
    <section
      className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5"
      data-testid="explore-create-dialog"
    >
      <div className="flex items-center justify-between gap-2">
        <h2 className="m-0 text-[15px]">新建探索</h2>
        <button
          type="button"
          className="cursor-pointer rounded-md border border-border bg-transparent px-3 py-1.5 text-sm hover:bg-muted"
          data-testid="explore-create-toggle"
          onClick={toggle}
        >
          {open ? '收起' : '新建'}
        </button>
      </div>
      {open && (
        <>
          <EntryTabs onSelect={selectTab} tab={tab} />
          {tab === 'topic' ? (
            <TopicEntry onCreated={onCreated} root={root} />
          ) : (
            <ImportEntry onCreated={onCreated} root={root} />
          )}
        </>
      )}
    </section>
  );
}
