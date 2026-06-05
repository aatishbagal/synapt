/** Pure index-stepping helpers for keyboard navigation of the result list. */

/**
 * Next highlighted index when moving the selection down. Caps at the last item
 * and never exceeds it. Returns -1 when there are no results.
 */
export function stepDown(current: number, count: number): number {
  if (count <= 0) return -1;
  return Math.min(current + 1, count - 1);
}

/**
 * Next highlighted index when moving the selection up. Returns -1 when moving
 * above the first item, which returns focus to the search input.
 */
export function stepUp(current: number): number {
  return current <= 0 ? -1 : current - 1;
}
