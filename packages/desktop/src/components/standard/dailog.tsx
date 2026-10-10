import { useCallback } from 'react';

import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog';

import { useOpen } from './hooks/use-open';

type StandardDialogProps = {
  title: string;
  content: React.JSX.Element;
  trigger?: React.JSX.Element;
  onSubmit?: (close: () => void) => void;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
};

export function StandardDialog({
  title,
  trigger,
  content,
  onSubmit,
  open: outerOpen,
  onOpenChange: onOuterOpenChange,
  ...rest
}: StandardDialogProps) {
  const hasTrigger = trigger !== undefined;
  const hasSubmit = typeof onSubmit === 'function';

  const [open, setOpen] = useOpen(outerOpen, onOuterOpenChange);
  const close = useCallback(() => setOpen(false), []);

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      {hasTrigger && <DialogTrigger render={trigger} />}
      <DialogContent {...rest}>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
        </DialogHeader>
        {content}
        <DialogFooter>
          <DialogClose render={<Button variant="outline" />}>
            {hasSubmit ? '取消' : '关闭'}
          </DialogClose>
          {hasSubmit && (
            <Button variant="default" onClick={() => onSubmit?.(close)}>
              提交
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
