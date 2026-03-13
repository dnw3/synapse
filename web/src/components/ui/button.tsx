import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { forwardRef } from "react";
import { cn } from "../../lib/cn";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap font-medium transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent)] focus-visible:ring-offset-0 focus-visible:shadow-[0_0_0_3px_var(--accent-glow)] disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 cursor-pointer active:scale-[0.97] active:duration-100",
  {
    variants: {
      variant: {
        default:
          "bg-[var(--accent)] text-white hover:brightness-110 [text-shadow:0_1px_1px_rgba(0,0,0,0.2)]",
        secondary:
          "bg-[var(--bg-grouped)] text-[var(--text-primary)] hover:bg-[var(--bg-elevated)]",
        ghost:
          "bg-transparent text-[var(--accent)] hover:bg-[var(--bg-hover)]",
        destructive:
          "bg-[var(--error)] text-white hover:brightness-110 [text-shadow:0_1px_1px_rgba(0,0,0,0.2)]",
        outline:
          "border border-[var(--border-subtle)] bg-transparent text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]",
        link:
          "text-[var(--accent)] underline-offset-4 hover:underline bg-transparent",
      },
      size: {
        sm: "h-7 px-3 text-[11px] rounded-[var(--radius-sm)] [&_svg]:size-3",
        default: "h-8 px-4 text-[13px] rounded-[var(--radius-sm)] [&_svg]:size-4",
        lg: "h-9 px-5 text-[14px] rounded-[var(--radius-md)] [&_svg]:size-4",
        icon: "h-8 w-8 rounded-[var(--radius-md)] [&_svg]:size-4",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
);

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean;
}

const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, asChild = false, ...props }, ref) => {
    const Comp = asChild ? Slot : "button";
    return (
      <Comp
        className={cn(buttonVariants({ variant, size, className }))}
        ref={ref}
        {...props}
      />
    );
  }
);
Button.displayName = "Button";

export { Button, buttonVariants };
