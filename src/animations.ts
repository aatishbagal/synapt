export const SPRING = {
  type: 'spring' as const,
  stiffness: 300,
  damping: 30,
  mass: 1,
};

export const SPRING_GENTLE = {
  type: 'spring' as const,
  stiffness: 200,
  damping: 28,
  mass: 1,
};

export const EASE_STANDARD = [0.25, 0.1, 0.25, 1.0] as const;

export const DURATION = {
  micro:  0.1,
  short:  0.15,
  medium: 0.2,
};

// Overlay entrance - slides down from slightly above, fades in
export const OVERLAY_ENTER = {
  initial:   { opacity: 0, y: -8, scale: 0.98 },
  animate:   { opacity: 1, y: 0,  scale: 1 },
  exit:      { opacity: 0, y: -4, scale: 0.99 },
  transition: { ...SPRING, stiffness: 350, damping: 35 },
};

// Result list item - staggers in from left
export const RESULT_ITEM = (index: number) => ({
  initial:    { opacity: 0, x: -6 },
  animate:    { opacity: 1, x: 0 },
  transition: { ...SPRING_GENTLE, delay: index * 0.03 },
});

// Panel entrance for SynaptClip
export const PANEL_ENTER = {
  initial:    { opacity: 0, scale: 0.97 },
  animate:    { opacity: 1, scale: 1 },
  exit:       { opacity: 0, scale: 0.98 },
  transition: { ...SPRING },
};

// Section/content fade
export const FADE_IN = {
  initial:    { opacity: 0 },
  animate:    { opacity: 1 },
  transition: { duration: DURATION.short, ease: EASE_STANDARD },
};

// Inline confirmation slide down
export const INLINE_CONFIRM = {
  initial:    { opacity: 0, height: 0 },
  animate:    { opacity: 1, height: 'auto' },
  exit:       { opacity: 0, height: 0 },
  transition: { duration: DURATION.short, ease: EASE_STANDARD },
};

// Removed list item - quick fade in place, no movement
export const ITEM_EXIT = {
  opacity: 0,
  transition: { duration: DURATION.micro, ease: EASE_STANDARD },
};
