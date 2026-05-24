import { cn } from "~/lib/utils"

const BRAND_GREEN = "oklch(0.62 0.15 155)"

export function BrandMark({ className }: { className?: string }) {
  return (
    <span
      className={cn(
        "inline-flex items-baseline gap-[0.05em] font-sans leading-none tracking-tight",
        className,
      )}
      aria-label="Conecta SUAS"
    >
      <span className="font-light text-foreground/85">Conecta</span>
      <Connector />
      <span
        className="font-extrabold"
        style={{ color: BRAND_GREEN }}
      >
        SUAS
      </span>
    </span>
  )
}

function Connector() {
  return (
    <svg
      viewBox="0 0 18 18"
      xmlns="http://www.w3.org/2000/svg"
      className="size-[0.6em] -translate-y-[0.35em]"
      aria-hidden="true"
    >
      <path
        d="M3 14 L15 4"
        stroke={BRAND_GREEN}
        strokeWidth="1.6"
        strokeLinecap="round"
        fill="none"
      />
      <circle cx="4" cy="13" r="3" fill={BRAND_GREEN} />
    </svg>
  )
}
