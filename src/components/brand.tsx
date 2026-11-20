import { cn } from "@/lib/utils"

export function BrandMark({ className }: { className?: string }) {
  return (
    <span
      className={cn(
        "grid size-8 shrink-0 place-items-center rounded-[10px] bg-foreground text-background shadow-sm",
        className
      )}
      aria-hidden="true"
    >
      <svg
        width="18"
        height="18"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        <path d="M14 6h2a2 2 0 0 1 2 2v5" />
        <path d="M14.21 15.202 18 17.4V22" />
        <path d="M14.796 14.611A1.6 1.6 0 0 0 14 15.995v3.013a1.6 1.6 0 0 0 .797 1.384l2.4 1.392a1.6 1.6 0 0 0 1.606 0l2.4-1.392A1.6 1.6 0 0 0 22 19.008v-3.013a1.6 1.6 0 0 0-.796-1.384l-2.4-1.394a1.6 1.6 0 0 0-1.514-.051z" />
        <path d="m18 17.4 3.79-2.198" />
        <path d="M2.21 4.202 6 6.4V16a2 2 0 0 0 2 2h2" />
        <path d="M2.796 3.611A1.6 1.6 0 0 0 2 4.995v3.013a1.6 1.6 0 0 0 .797 1.384l2.4 1.392a1.6 1.6 0 0 0 1.606 0l2.4-1.392A1.6 1.6 0 0 0 10 8.008V4.995a1.6 1.6 0 0 0-.796-1.384l-2.4-1.394a1.6 1.6 0 0 0-1.514-.052z" />
        <path d="m6 6.4 3.79-2.198" />
      </svg>
    </span>
  )
}

export function BrandLockup({ className }: { className?: string }) {
  return (
    <div className={cn("flex items-center gap-2.5", className)}>
      <BrandMark />
      <div className="flex flex-col">
        <span className="text-[14px] leading-none font-semibold tracking-[-0.02em]">
          Localink
        </span>
        <span className="mt-1 text-[9.5px] leading-none tracking-[0.14em] text-muted-foreground uppercase">
          Local transfer
        </span>
      </div>
    </div>
  )
}
