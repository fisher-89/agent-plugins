import { useCallback, useEffect, useState } from 'react';

type OpenPropState = [boolean, (value: boolean) => void];

export function useOpen(
  outerOpen?: boolean,
  onOuterOpenChange?: (value: boolean) => void,
): OpenPropState {
  const [innerOpen, setInnerOpen] = useState(outerOpen ?? false);

  useEffect(() => {
    if (outerOpen === undefined || outerOpen === innerOpen) {
      return;
    }
    setInnerOpen(outerOpen);
  }, [outerOpen]);

  const handleOpenChange = useCallback((value: boolean) => {
    setInnerOpen(value);
    onOuterOpenChange?.(value);
  }, []);

  return [innerOpen, handleOpenChange];
}
