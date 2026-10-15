# mobile-primitives

Objective: stable typing, deliberate gestures and cohesive motion on iOS. Shared commands, style, structure and boundaries: [contract](README.md).

Acceptance:
- A focused input stays registered across frames; no repeated egui request_focus/IME interruption. Explicit transfer and dismissal work.
- A stationary hold opens one context action sheet; movement/scroll/cancel cancels it; releasing a consumed hold never plays the row. Preserve desktop context menus.
- Reusable sheet supports scrim, safe area, visible close affordance, drag dismissal and focus restoration. Transitions stay interruptible and honor reduced-motion settings where available.
- Regression tests cover a repeated focus frame, input transfer, hold threshold, motion/cancel and duplicate activation. Visual tests cover open/closed sheets at phone widths.

Ownership: generic focus/gestures/sheets/motion in small limn PRs; Surco chooses action content and navigation.
