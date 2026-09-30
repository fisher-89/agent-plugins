import { useState } from 'react';

import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';

import type { ArtifactEnvelope } from '../../../types/dto';
import { ArtifactView } from './artifact-view';

/** 产物 tab 组：详情页产物区与抽屉本站文档节共用的多文档切换。多文档 Tabs
 * （仅当前 TabsContent 挂载，替代纵向堆叠的上下滚动浏览）、单文档不设 tab 条
 * 直接渲染；value 受控 + min 收敛，清单变短（刷新后）不重置回首项。
 * 入参约定非空：空态占位由各调用方以自有文案呈现。 */
export function ArtifactTabs({ artifacts }: { artifacts: ArtifactEnvelope[] }): React.JSX.Element {
  const [active, setActive] = useState(0);
  const current = Math.min(active, artifacts.length - 1);
  if (artifacts.length === 1) {
    return <ArtifactView envelope={artifacts[0]} />;
  }
  return (
    <Tabs onValueChange={(value) => setActive(Number(value))} value={String(current)}>
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
      {artifacts.map((envelope, index) => (
        <TabsContent key={`${envelope.kind}-${envelope.title}-${index}`} value={String(index)}>
          <ArtifactView envelope={envelope} />
        </TabsContent>
      ))}
    </Tabs>
  );
}
