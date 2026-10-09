import { useCallback, useState } from 'react';

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

type StandardDialogProps = {
  title: string;
  trigger?: React.JSX.Element;
  content: React.JSX.Element;
  onSubmit?: (close: () => void) => void;
};

export function StandardDialog({ title, trigger, content, onSubmit }: StandardDialogProps) {
  const [open, setOpen] = useState(false);
  const hasSubmit = typeof onSubmit === 'function';
  const close = useCallback(() => setOpen(false), []);

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger render={trigger} />
      <DialogContent>
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
