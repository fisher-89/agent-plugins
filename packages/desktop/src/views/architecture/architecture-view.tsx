import { RefreshCwIcon } from 'lucide-react';
import { useState } from 'react';

import { Button } from '@/components/ui/button';
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable';

import { ArchiChatPanel } from './agent/archi-chat-panel';
import { ArchitectureCanvas } from './components/architecture-canvas';

export function ArchitectureView({ root }: { root: string | null }): React.JSX.Element {
  const [refreshTick, setRefreshTick] = useState(0);
  return (
    <div className="flex h-full min-h-0 flex-col gap-2">
      <div className="flex flex-wrap items-center gap-2">
        <h2 className="m-0 text-[15px]">架构</h2>
        <div className="flex items-center gap-2">
          <Button
            variant="ghost"
            size="icon"
            data-testid="archi-refresh"
            title="刷新"
            onClick={() => setRefreshTick((tick) => tick + 1)}
          >
            <RefreshCwIcon />
          </Button>
        </div>
      </div>
      <ResizablePanelGroup className="min-h-0 flex-1" orientation="horizontal">
        <ArchiChatPanel root={root} />
        <ResizableHandle withHandle />
        <ResizablePanel minSize="40">
          <div className="h-full min-h-0 pl-1.5">
            <ArchitectureCanvas key={refreshTick} root={root} mode="full" />
          </div>
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  );
}
