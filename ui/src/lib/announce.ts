/**
 * Screen-reader announcement utility.
 * Writes to the #sr-announcements live region in App.tsx.
 */
export function announce(message: string): void {
  const el = document.getElementById("sr-announcements");
  if (!el) return;
  // Clear then set — ensures repeated identical messages are re-announced
  el.textContent = "";
  requestAnimationFrame(() => {
    el.textContent = message;
  });
}
