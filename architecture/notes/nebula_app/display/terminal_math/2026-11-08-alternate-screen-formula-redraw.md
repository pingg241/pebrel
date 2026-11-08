# Preserve formula identity across alternate-screen redraw scrolling

## Status

Proposed for review; scoped to already observed complete formulas.

## Context

A formula can disappear from native rendering as its opening rows leave a
Codex transcript. The previous regression only used terminal scrollback:
`Scroll::Delta` changes the viewport origin, so absolute formula rows survive.
An alternate-screen application instead clears/repaints its document at new
physical rows. The terminal viewport origin and history size remain unchanged.
The old fingerprints then compare against the wrong rows and discard the only
complete source as soon as its opening delimiter is no longer visible.

## Evidence

The real terminal stream-parser regression
`tui_redraw_scroll_keeps_a_partially_visible_formula` uses `1049h`, cursor
positioning and clear-screen sequences. On the previous code, the second row
of scroll leaves zero formula plans for a previously rendered two-line
fraction. History remains zero throughout. This reproduces the failure
without a long formula or any formula byte-limit change.

## Decision

Keep ordinary terminal history anchoring unchanged. In alternate screens,
compare the previous content grid with the current grid before validating
persisted formulas. Only unique rows containing letters or numbers can
identify displacement, and only when they belonged to a formula or its
immediate context. Every visible formula source span must match its recorded
fingerprint at the proposed destination. Conflicting displacements and
repeated fragments are insufficient evidence.

Move verified anchors as one batch. Signed row coordinates allow an opener
to move above row zero while the complete source and full geometry survive.
Rendering still clips to the current viewport. Switching screen modes or
changing width invalidates the scan and formula anchors. A wholly blank
alternate-screen frame paints no formulas but retains the last content
snapshot for a subsequent redraw delivered in another PTY read.

## Rejected alternatives

- Treat the TUI redraw as terminal scrollback: neither the history nor viewport
  offset changes, which is precisely why the earlier fix missed this case.
- Match only delimiters or repeat the last bitmap at its old coordinates:
  either can attach a stale formula to unrelated text.
- Reconstruct unseen source from the remaining tail: information is missing;
  a formula that has never been fully observed must remain literal.
- Disable source validation after scrolling: edits and new output would keep
  an obsolete formula on screen.

## Consequences

No terminal cells, cursor, scrollback or copied text are modified. The existing
formula count/source budgets still bound retained data; relocated entries keep
the same source and memory charge. Unchanged frames and frames with no invalid
visible formula avoid row-map allocation. A candidate redraw hashes each row
of the previous/current grid once, then checks candidate offsets against the
persisted spans; a successful offset is checked only once per formula.
These are work/allocation bounds, not a universal latency claim.

This does not infer formulas first seen with missing source, nor does it prove
all custom TUI clipping layouts are recoverable. It also does not recover a
formula after a width-changing reflow invalidated its original source rows.

## Validation

The terminal adapter regression covers scrolling both directions, repeated
frames, a clear-screen delivered separately, unchanged font size/baseline
translation and source coverage. Matching tests cover bottom-edge clipping,
source edits, ambiguous duplicate rows and cache-charge stability. Existing
normal-history, table, selection and image projection regressions also apply.
A runtime connection to the user's current window was unavailable during
investigation; these are automated layout-plan checks, not live GPU evidence.

## Supersedes

Refines the clipped-cache handling in `e3986b8c`; its ordinary-history tests
remain necessary but do not cover alternate-screen document scrolling.

## Revisit when

TUI applications provide stable document/formula identities or a semantic
source protocol that avoids inferring row motion from visible content.
