import { BaseEdge, EdgeLabelRenderer, getBezierPath, type EdgeProps } from '@xyflow/react';
import { MessageSquareMoreIcon } from 'lucide-react';

import { HoverCard, HoverCardContent, HoverCardTrigger } from '@/components/ui/hover-card';

/** 图标开关延后（默认 600ms 过长——图上标记点密集时容易误触）。 */
const OPEN_DELAY_MS = 120;

/**
 * 带标签边的渲染形态（nodeTypes 外的 edgeTypes 键 `labeled`）：边线上只落一个
 * 标记点，全量 label 经 HoverCard 悬停 / 聚焦呈现。
 *
 * 判别只看 label（`toChartEdges` 按 label 有无选 type）：内置边的标签走
 * `BaseEdge → EdgeText` 落 SVG `<text>`，单行无宽度约束——上千字的回跳记因会
 * 撑成一条长线并被画布 overflow 裁掉，故带标签边改走本组件。
 */
export function LabeledEdge({
  label,
  style,
  markerEnd,
  ...pathProps
}: EdgeProps): React.JSX.Element {
  const [path, labelX, labelY] = getBezierPath(pathProps);
  const accent = style?.stroke;
  // EdgeProps 的 label 是 ReactNode：本组件只承接文本标签（模型层 label 恒 string），
  // 非文本形态按无标签处理，不落标记点
  const text = typeof label === 'string' && label.trim() !== '' ? label : null;
  return (
    <>
      {/* 不向 BaseEdge 传 label：标签不落 SVG text，改经 HTML overlay 呈现 */}
      <BaseEdge path={path} markerEnd={markerEnd} style={style} />
      {text !== null && (
        <EdgeLabelRenderer>
          <HoverCard>
            <HoverCardTrigger
              delay={OPEN_DELAY_MS}
              render={
                <button
                  type="button"
                  // overlay 容器默认 pointer-events: none（不拦画布平移）——标记点
                  // 自带 pointer-events-auto 才可悬停；nodrag / nopan 防拖拽与滚轮平移
                  className="nodrag nopan pointer-events-auto absolute flex size-4 -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-full border bg-background"
                  style={{ left: labelX, top: labelY, color: accent, borderColor: accent }}
                  aria-label={`边标签：${text}`}
                  data-testid="edge-label-marker"
                >
                  <MessageSquareMoreIcon className="size-2.5" aria-hidden="true" />
                </button>
              }
            />
            <HoverCardContent className="max-w-[320px] whitespace-pre-wrap wrap-break-word">
              {text}
            </HoverCardContent>
          </HoverCard>
        </EdgeLabelRenderer>
      )}
    </>
  );
}
