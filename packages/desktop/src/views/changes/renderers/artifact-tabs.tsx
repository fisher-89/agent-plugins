import { useState } from 'react';

import { ScrollArea, ScrollBar } from '@/components/ui/scroll-area';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';

import type { ArtifactEnvelope } from '../../../types/dto';
import { ArtifactView } from './artifact-view';

export function ArtifactTabs({ artifacts }: { artifacts: ArtifactEnvelope[] }): React.JSX.Element {
  const [active, setActive] = useState(0);
  const current = Math.min(active, artifacts.length - 1);
  if (artifacts.length === 1) {
    return <ArtifactView envelope={artifacts[0]} />;
  }
  return (
    <Tabs onValueChange={(value) => setActive(Number(value))} value={String(current)}>
      <ScrollArea>
        <TabsList data-testid="artifact-tabs">
          {artifacts.map((envelope, index) => (
            <TabsTrigger
              className="max-w-[200px] truncate"
              data-testid="artifact-tab"
              key={`${envelope.kind}-${envelope.title}-${index}`}
              value={String(index)}
            >
              {envelope.title}
            </TabsTrigger>
          ))}
        </TabsList>
        <ScrollBar orientation="horizontal" />
      </ScrollArea>
      {artifacts.map((envelope, index) => (
        <TabsContent key={`${envelope.kind}-${envelope.title}-${index}`} value={String(index)}>
          <ArtifactView envelope={envelope} />
        </TabsContent>
      ))}
    </Tabs>
  );
}
