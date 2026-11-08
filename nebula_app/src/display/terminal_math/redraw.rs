//! Reanchor verified formula sources when a TUI scrolls by repainting its grid.

use super::*;

/// A content rectangle proven by fixed edge rows in two redraw snapshots.
/// Keep its fingerprint so a later UI/layout change cannot silently hide edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct TuiClip {
    pub(super) top: i64,
    pub(super) bottom: i64,
    fingerprint: u64,
}

impl TuiClip {
    pub(super) fn contains(self, row: i64) -> bool {
        (self.top..self.bottom).contains(&row)
    }

    fn edge_fingerprint(self, grid: &TextGrid) -> u64 {
        let mut hasher = DefaultHasher::new();
        grid.absolute_top.hash(&mut hasher);
        grid.rows.len().hash(&mut hasher);
        for (row, cells) in grid.rows.iter().enumerate() {
            if !self.contains(grid.absolute_top as i64 + row as i64) {
                row.hash(&mut hasher);
                cells.hash(&mut hasher);
                grid.wrapped[row].hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    pub(super) fn matches(self, grid: &TextGrid) -> bool {
        self.fingerprint == self.edge_fingerprint(grid)
    }

    fn stationary_edges(previous: &TextGrid, grid: &TextGrid) -> Option<Self> {
        if previous.rows.len() != grid.rows.len() {
            return None;
        }
        let unchanged = |row: usize| {
            previous.rows[row] == grid.rows[row] && previous.wrapped[row] == grid.wrapped[row]
        };
        let informative = |range: std::ops::Range<usize>| {
            grid.rows[range].iter().flatten().any(|cell| cell.is_some_and(char::is_alphanumeric))
        };
        let mut top = 0;
        while top < grid.rows.len() && unchanged(top) {
            top += 1;
        }
        if !informative(0..top) {
            top = 0;
        }
        let mut bottom = grid.rows.len();
        while bottom > top && unchanged(bottom - 1) {
            bottom -= 1;
        }
        if !informative(bottom..grid.rows.len()) {
            bottom = grid.rows.len();
        }
        if top == bottom || top == 0 && bottom == grid.rows.len() {
            return None;
        }
        let mut clip = Self {
            top: grid.absolute_top as i64 + top as i64,
            bottom: grid.absolute_top as i64 + bottom as i64,
            fingerprint: 0,
        };
        clip.fingerprint = clip.edge_fingerprint(grid);
        Some(clip)
    }
}

impl TerminalMathState {
    pub(super) fn relocate_tui_formulas(&mut self, grid: &mut TextGrid) {
        let Some((previous, _)) = &self.last_scan else { return };
        grid.tui_clip = previous.tui_clip.filter(|clip| clip.matches(grid));
        if previous == grid
            || previous.columns != grid.columns
            || previous.absolute_top != grid.absolute_top
            || grid.rows.is_empty()
        {
            return;
        }
        let top = previous.absolute_top as i64;
        let bottom = top + previous.rows.len() as i64 - 1;
        let lower = FormulaAnchor { row: top - self.max_formula_rows as i64, column: 0 };
        let upper = FormulaAnchor { row: bottom, column: usize::MAX };
        let candidates: Vec<_> = self
            .formulas
            .range(lower..=upper)
            .filter(|(_, formula)| {
                formula.intersects(top, bottom)
                    && !formula.matches_visible_rows(grid)
                    && formula.matches_visible_rows(previous)
            })
            .collect();
        if candidates.is_empty() {
            return;
        }

        // Hash each physical row once, independent of the number of formulas.
        // Repeated rows cannot identify motion; blank/delimiter-only rows cannot
        // prove that the surrounding document is still the same document.
        let before = unique_content_rows(previous);
        let after = unique_content_rows(grid);
        let clip = TuiClip::stationary_edges(previous, grid).or(grid.tui_clip);
        let moves: Vec<_> = candidates
            .into_iter()
            .filter_map(|(&anchor, formula)| {
                let mut matched = None;
                for (&fingerprint, &row) in &before {
                    let (Some(row), Some(Some(next))) = (row, after.get(&fingerprint)) else {
                        continue;
                    };
                    let absolute_row = top + row as i64;
                    if absolute_row < formula.anchor.row - 1
                        || absolute_row > formula.last_row() + 1
                    {
                        continue;
                    }
                    let delta = *next as i64 - row as i64;
                    if delta == 0
                        || matched == Some(delta)
                        || !formula.matches_shifted_rows(grid, delta, clip)
                    {
                        continue;
                    }
                    if matched.is_some_and(|previous_delta| previous_delta != delta) {
                        return None;
                    }
                    matched = Some(delta);
                }
                let delta = matched?;
                let mut moved = formula.clone();
                moved.anchor.row += delta;
                moved.clip = clip;
                for span in &mut moved.spans {
                    span.row += delta;
                }
                Some((anchor, moved))
            })
            .collect();
        if moves.is_empty() {
            return;
        }

        grid.tui_clip = clip;
        // Move as a batch so one destination cannot erase another formula
        // before its source rows have been checked against the previous frame.
        for (anchor, _) in &moves {
            self.remove(*anchor);
        }
        for (_, formula) in moves {
            let replaced: Vec<_> = self
                .formulas
                .iter()
                .filter_map(|(&anchor, existing)| existing.overlaps(&formula).then_some(anchor))
                .collect();
            for anchor in replaced {
                self.remove(anchor);
            }
            self.persisted_bytes += formula.charge;
            self.max_formula_rows = self.max_formula_rows.max(formula.row_count());
            self.formulas.insert(formula.anchor, formula);
        }
        self.pending_display = None;
    }
}

fn unique_content_rows(grid: &TextGrid) -> BTreeMap<u64, Option<usize>> {
    let mut rows = BTreeMap::new();
    for (row, cells) in grid.rows.iter().enumerate() {
        if !cells.iter().any(|cell| cell.is_some_and(char::is_alphanumeric)) {
            continue;
        }
        let mut hasher = DefaultHasher::new();
        cells.hash(&mut hasher);
        grid.wrapped[row].hash(&mut hasher);
        rows.entry(hasher.finish()).and_modify(|position| *position = None).or_insert(Some(row));
    }
    rows
}

impl PersistedFormula {
    fn matches_shifted_rows(&self, grid: &TextGrid, delta: i64, clip: Option<TuiClip>) -> bool {
        let mut compared = false;
        for span in &self.spans {
            let Ok(row) = usize::try_from(span.row + delta - grid.absolute_top as i64) else {
                continue;
            };
            if row >= grid.rows.len() || clip.is_some_and(|clip| !clip.contains(span.row + delta)) {
                continue;
            }
            // Unlike the stationary redraw tolerance, relocation needs every
            // visible source span to match, including its delimiters/wrap flags.
            if grid.span_fingerprint(row, span.start, span.end, span.include_wrap)
                != Some(span.fingerprint)
            {
                return false;
            }
            compared = true;
        }
        compared
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(rows: &[&str]) -> TextGrid {
        let mut grid = TextGrid::from_rows(rows);
        grid.columns = 64;
        for row in &mut grid.rows {
            row.resize(grid.columns, Some(' '));
        }
        grid
    }

    fn scan(state: &mut TerminalMathState, grid: &TextGrid) -> Vec<FormulaOverlay> {
        let mut grid = grid.clone();
        state.synchronize_grid(&grid);
        state.relocate_tui_formulas(&mut grid);
        state.scan_visible_grid(&grid, None);
        state.visible_overlays(&grid)
    }

    #[test]
    fn redraw_can_move_a_formula_below_the_bottom_without_a_closer() {
        let initial = grid(&[r"\[", "a^2", "c+d", r"\]", "after"]);
        let mut state = TerminalMathState::default();
        let source = scan(&mut state, &initial)[0].source_arc();
        let moved = grid(&["heading", "more", "context", r"\[", "a^2"]);
        let formulas = scan(&mut state, &moved);
        assert_eq!(formulas.len(), 1);
        assert_eq!(formulas[0].source, source);
        assert_eq!((formulas[0].spans[0].row, formulas[0].spans.last().unwrap().row), (3, 6));
        let returned = scan(&mut state, &initial);
        assert_eq!(returned.len(), 1);
        assert_eq!(returned[0].source, source);
    }

    #[test]
    fn changed_visible_source_never_reuses_the_hidden_formula_prefix() {
        let mut state = TerminalMathState::default();
        let initial = grid(&[r"\[", "a^2", "c+d", r"\]", "after"]);
        assert_eq!(scan(&mut state, &initial).len(), 1);
        let changed = grid(&["a^2", "c+e", r"\]", "after", "new"]);
        assert!(scan(&mut state, &changed).is_empty());
    }

    #[test]
    fn repeated_fragments_and_lone_delimiters_do_not_identify_motion() {
        for moved in [
            grid(&["x^2", r"\]", "x^2", r"\]", "other"]),
            grid(&[r"\]", "unrelated", "new", "", ""]),
        ] {
            let mut state = TerminalMathState::default();
            assert_eq!(scan(&mut state, &grid(&[r"\[", "x^2", r"\]", "after", ""])).len(), 1);
            assert!(scan(&mut state, &moved).is_empty());
        }
    }

    #[test]
    fn fixed_edge_clip_does_not_hide_source_edits_or_ui_changes() {
        let initial = grid(&["QUESTION", r"\[", "a^2", "c+d", r"\]", "after", "", "BACK", "INPUT"]);
        let moved =
            grid(&["QUESTION", "earlier", "more", "context", r"\[", "a^2", "c+d", "BACK", "INPUT"]);
        for changed in [
            grid(&["QUESTION", "earlier", "more", "context", r"\[", "a^2", "c+e", "BACK", "INPUT"]),
            grid(&[
                "QUESTION", "earlier", "more", "context", r"\[", "a^2", "c+d", "EDITED", "INPUT",
            ]),
        ] {
            let mut state = TerminalMathState::default();
            let original = scan(&mut state, &initial)[0].source_arc();
            let formulas = scan(&mut state, &moved);
            assert_eq!(formulas.len(), 1);
            assert_eq!(formulas[0].source, original);
            assert_eq!(formulas[0].clip_rows, Some((1, 7)));
            assert!(scan(&mut state, &changed).is_empty());
        }
    }

    #[test]
    fn unchanged_redraw_does_not_move_cache_or_increase_its_charge() {
        let mut state = TerminalMathState::default();
        let initial = grid(&[r"\[", "a^2", "c+d", r"\]", "after"]);
        assert_eq!(scan(&mut state, &initial).len(), 1);
        let moved = grid(&["a^2", "c+d", r"\]", "after", "new"]);
        assert_eq!(scan(&mut state, &moved).len(), 1);
        let charge = state.persisted_bytes;
        for _ in 0..3 {
            let formulas = scan(&mut state, &moved);
            assert_eq!(formulas.len(), 1);
            assert_eq!(formulas[0].spans[0].row, -1);
            assert_eq!(state.persisted_bytes, charge);
            assert_eq!(state.formulas.len(), 1);
        }
    }
}
