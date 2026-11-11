import { type MenuTriggerState, type UseRenderRenderProp } from '@base-ui/react';
import { useEffect, useState } from 'react';
import { toast } from 'sonner';

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { ipc } from '@/lib/ipc';
import { type ExploreScanEntry } from '@/types/generated/bindings';

export interface ExploreOpenFileProps {
  trigger: UseRenderRenderProp<MenuTriggerState>;
  root: string;
  onCreated: (name: string, autoPrompt: string | null) => void;
}

export function ExploreOpenFile({
  trigger,
  root,
  onCreated,
}: ExploreOpenFileProps): React.JSX.Element {
  const [entries, setEntries] = useState<ExploreScanEntry[] | null>(null);

  useEffect(() => {
    let cancelled = false;
    ipc
      .scanExplores(root)
      .then((result) => {
        if (!cancelled) setEntries(result);
      })
      .catch((err: unknown) => {
        if (!cancelled) toast.error(String(err));
      });
    return () => {
      cancelled = true;
    };
  }, [root]);
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={trigger} />
      <DropdownMenuContent className="w-80">
        {entries === null && (
          <DropdownMenuGroup>
            <DropdownMenuLabel data-testid="explore-import-loading">扫描中…</DropdownMenuLabel>
          </DropdownMenuGroup>
        )}
        {entries?.length === 0 && (
          <DropdownMenuGroup>
            <DropdownMenuLabel data-testid="explore-import-empty">
              没有可导入的文档
            </DropdownMenuLabel>
          </DropdownMenuGroup>
        )}
        {entries?.map((entry) => (
          <DropdownMenuItem
            key={entry.name}
            className="flex-col items-start p-2"
            data-testid="explore-import-item"
            onClick={() => {
              ipc
                .createExploreRecord(root, entry.name)
                .then(() => onCreated(entry.name, null))
                .catch((err: unknown) => toast.error(String(err)));
            }}
          >
            <div>{entry.name}</div>
            {entry.modifiedAt !== null && (
              <div className="text-muted-foreground">
                {new Date(entry.modifiedAt).toLocaleString()}
              </div>
            )}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
