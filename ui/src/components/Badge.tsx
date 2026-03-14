/** Shared inline badge — status, CI, origin, and environment labels. */
export function Badge({ label, colorClass }: { label: string; colorClass: string }) {
  return (
    <span className={`px-1.5 py-0.5 rounded border text-[10px] font-mono uppercase tracking-wider ${colorClass}`}>
      {label}
    </span>
  );
}
