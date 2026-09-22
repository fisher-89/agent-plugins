import { cva, type VariantProps } from 'class-variance-authority';

import { cn } from '@/lib/utils';

/* shadcn Badge vendored 内部化：变体集收敛为本 app 实际用量——
   代际 inv0/inv1/inv2 与 kind 直接取 Tailwind 调色板（原 hex 本即调色板值，等值零漂移），
   pass/fail/active 走 CSS 入口 token（design D10/D11）。 */
const badgeVariants = cva(
  'inline-block rounded px-[7px] py-px text-xs font-semibold leading-[18px]',
  {
    variants: {
      variant: {
        inv0: 'bg-gray-200 text-gray-600',
        inv1: 'bg-yellow-100 text-yellow-800',
        inv2: 'bg-blue-100 text-blue-700',
        pass: 'bg-pass-bg text-pass',
        fail: 'bg-fail-bg text-fail',
        kind: 'bg-violet-100 text-violet-700',
        active: 'bg-primary text-primary-foreground',
      },
    },
    defaultVariants: {
      variant: 'inv0',
    },
  },
);

function Badge({
  className,
  variant,
  ...props
}: React.ComponentProps<'span'> & VariantProps<typeof badgeVariants>): React.JSX.Element {
  return (
    <span data-slot="badge" className={cn(badgeVariants({ variant }), className)} {...props} />
  );
}

export { Badge };
