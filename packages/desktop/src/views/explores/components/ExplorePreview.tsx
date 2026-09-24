import type { ArtifactEnvelope, ExploreDoc } from '../../../types/dto';
import { MarkdownDocRenderer } from '../../changes/renderers/MarkdownDocRenderer';

export interface ExplorePreviewProps {
  /** 当前笔记内容；null 即未落盘 / 已删除（空态，不报错） */
  doc: ExploreDoc | null;
  /** 磁盘重读进行中 */
  loading: boolean;
  /** watch 防抖刷新 / 显式刷新入口 */
  onRefresh: () => void;
}

/** 复用 change 域 markdown-doc renderer：应用层组装同构信封（payload { markdown }） */
function toEnvelope(doc: ExploreDoc): ArtifactEnvelope {
  return {
    kind: 'markdown-doc',
    version: 1,
    title: doc.name,
    payload: { markdown: doc.content },
    fallbackText: null,
  };
}

/**
 * 预览（详情页右栏）：read_explore 文本经既有 MarkdownDocRenderer 渲染；
 * 未落盘 / 已删除呈空态（记录保留、不报错），文件（重）出现后经 watch 信号
 * 恢复内容。watch 信号为失效信号（无内容），刷新一律显式拉取。
 */
export function ExplorePreview({
  doc,
  loading,
  onRefresh,
}: ExplorePreviewProps): React.JSX.Element {
  return (
    <section
      className="flex min-h-0 flex-col overflow-hidden rounded-lg border border-border bg-card"
      data-testid="explore-preview"
    >
      <div className="flex items-center justify-between gap-2 border-b border-border px-3 py-2">
        <h2 className="m-0 text-[15px]">笔记预览</h2>
        <span className="flex items-center gap-2">
          {loading && (
            <span className="text-xs text-muted-foreground" data-testid="preview-loading">
              刷新中…
            </span>
          )}
          <button
            type="button"
            className="cursor-pointer border-0 bg-transparent p-0 text-xs text-muted-foreground underline hover:text-foreground"
            data-testid="preview-refresh"
            onClick={onRefresh}
          >
            刷新
          </button>
        </span>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-3">
        {doc === null ? (
          <div className="text-muted-foreground" data-testid="preview-empty">
            笔记尚未落盘或已被删除。探索会话中 agent 会按主题落盘同名文件；文件出现后预览自动恢复。
          </div>
        ) : (
          <div data-testid="preview-doc">
            <MarkdownDocRenderer envelope={toEnvelope(doc)} />
          </div>
        )}
      </div>
    </section>
  );
}
