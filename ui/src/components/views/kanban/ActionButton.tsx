const styles = {
  primary:
    "border-accent/40 text-accent bg-accent-subtle/20 hover:bg-accent-subtle/40 hover:text-accent",
  secondary:
    "border-border text-text-muted hover:text-text-primary hover:bg-surface-secondary",
  danger:
    "border-status-conflict/30 text-status-conflict/70 hover:text-status-conflict hover:bg-status-conflict/10",
};

export function ActionButton({
  label,
  title,
  disabled,
  onClick,
  variant = "secondary",
}: {
  label: string;
  title: string;
  disabled: boolean;
  onClick: () => void;
  variant?: "primary" | "secondary" | "danger";
}) {
  return (
    <button
      type="button"
      title={title}
      disabled={disabled}
      onClick={onClick}
      className={`
        text-[10px] font-mono px-2.5 py-1.5 min-h-[36px] inline-flex items-center rounded border cursor-pointer
        transition-colors disabled:opacity-40 disabled:cursor-not-allowed
        ${styles[variant]}
      `}
    >
      {label}
    </button>
  );
}
