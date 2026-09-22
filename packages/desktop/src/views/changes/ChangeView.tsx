import { useCallback, useState } from 'react';

import { type ChangeListState } from '../../hooks/useChangeList';
import { ChangeDetailView } from './ChangeDetailView';
import { ChangeListView } from './ChangeListView';
import { useChangeDetail } from './hooks/useChangeDetail';

export function ChangeView({ root, list }: { root: string | null; list: ChangeListState }) {
  const [prevRoot, setPrevRoot] = useState(root);
  const [selectedChange, setSelectedChange] = useState<string | null>(null);
  const detail = useChangeDetail(root, selectedChange);

  if (prevRoot !== root) {
    setPrevRoot(root);
    setSelectedChange(null);
  }

  const openChange = useCallback((name: string) => setSelectedChange(name), []);
  const backToList = useCallback(() => setSelectedChange(null), []);

  return selectedChange === null ? (
    <ChangeListView state={list} onSelect={openChange} />
  ) : (
    <ChangeDetailView state={detail} onBack={backToList} />
  );
}
