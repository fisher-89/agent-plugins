import { useEffect, useRef } from 'react';

import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable';

import type { ExploreRecord } from '../../types/dto';
import { ExploreComposer } from './components/ExploreComposer';
import { ExploreConversation } from './components/ExploreConversation';
import { ExplorePreview } from './components/ExplorePreview';
import { useExploreDoc } from './hooks/useExploreDoc';
import { useExploreSession } from './hooks/useExploreSession';

export interface ExploreDetailViewProps {
  /** 当前 workspace root（cwd 与读面入参） */
  root: string;
  /** 当前记录（父层按 URL name 从清单派生；清单未就绪时为 null） */
  record: ExploreRecord | null;
}

/**
 * 探索详情页（双栏）：左对话区（对话 + composer）、右文档预览，resizable
 * 可拖分界。文档取数与 watch 订阅收口 useExploreDoc（订阅生命周期即页面
 * 生命周期，卸载即退订）；会话链还原 / 续话收口 useExploreSession；run 终态
 * （running true→false）定点重读文档，预览即随落盘更新。
 */
export function ExploreDetailView({ root, record }: ExploreDetailViewProps): React.JSX.Element {
  const name = record?.name ?? null;
  const doc = useExploreDoc(root, name);
  const session = useExploreSession(root, record);
  const wasRunning = useRef(false);

  // run 终态定点重读文档（含本轮 agent 落盘 / 更新笔记的情形）；挂载不触发
  useEffect(() => {
    if (wasRunning.current && !session.running) doc.refresh();
    wasRunning.current = session.running;
  }, [session.running, doc.refresh]);

  if (record === null) {
    return (
      <div className="text-muted-foreground" data-testid="explore-detail-missing">
        记录不存在或清单尚未就绪。
      </div>
    );
  }

  return (
    <div className="flex min-h-0 flex-col" data-testid="explore-detail">
      <ResizablePanelGroup className="min-h-0 flex-1" orientation="horizontal">
        <ResizablePanel defaultSize="55" minSize="25">
          <div className="flex h-full min-h-0 flex-col gap-3 pr-1.5">
            <ExploreConversation
              events={session.events}
              loading={session.loading}
              running={session.running}
            />
            <ExploreComposer disabled={session.running} onSend={session.send} />
          </div>
        </ResizablePanel>
        <ResizableHandle withHandle />
        <ResizablePanel defaultSize="45" minSize="20">
          <div className="h-full min-h-0 pl-1.5">
            <ExplorePreview doc={doc.doc} loading={doc.loading} onRefresh={doc.refresh} />
          </div>
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  );
}
