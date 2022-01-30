#[derive(Debug, Default, Clone, Copy, PartialEq, PartialOrd)]
pub struct View {
    pub pos: usize,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct StatefulPosition {
    // currently visible slice
    view: View,

    // value set by user
    user_offset: usize,

    // constants
    height: usize,
    length: usize,
    offset: usize,
}

impl StatefulPosition {
    pub fn reset(&mut self, offset: usize, length: usize) {
        self.view.start = 0;
        self.view.end = self.height;
        self.view.pos = 0;
        // self.height = height; # do not update it
        self.length = length;
        self.user_offset = offset;
        self.update_offset();
    }

    pub fn length_extended(&mut self, length: usize) {
        self.length = length;
    }

    pub fn position(&self) -> usize {
        self.view.pos + self.view.start
    }

    pub fn next(&mut self, count: usize) {
        if self.length == 0 {
            return;
        }
        self.view.pos = std::cmp::min(self.view.pos + count, self.length - 1 - self.view.start);

        if let Some(count_to_scroll) = self.view.pos.checked_sub(self.height - self.offset - 1) {
            let old_end = std::cmp::min(self.length, self.view.end);
            self.view.end = std::cmp::min(self.length, self.view.end + count_to_scroll);
            let count_to_scroll = self.view.end - old_end;
            self.view.pos -= count_to_scroll;
        }
        self.view.start = self.view.end.saturating_sub(self.height);
    }

    pub fn prev(&mut self, count: usize) {
        if self.length == 0 {
            return;
        }
        let old_pos = self.view.pos;
        self.view.pos = self.view.pos.saturating_sub(count);
        self.view.start = self
            .view
            .start
            .saturating_sub(count - (old_pos - self.view.pos));

        if self.view.pos < self.offset {
            let offset_missing = self.offset - 1 - self.view.pos;
            let old_start = self.view.start;
            self.view.start = self.view.start.saturating_sub(offset_missing);
            self.view.pos += old_start - self.view.start;
        }

        self.view.end = self.view.start + self.height;
    }

    pub fn end(&mut self) {
        self.view.pos = std::cmp::min(self.length, self.height).saturating_sub(1);
        self.view.end = self.length;
        self.view.start = self.length.saturating_sub(self.height);
    }

    pub fn start(&mut self) {
        self.view.pos = 0;
        self.view.start = 0;
        self.view.end = self.height;
    }

    // Modifies view by changing height of it
    pub fn set_height(&mut self, height: usize) {
        if self.height != height {
            if let Some(count) = self.height.checked_sub(height) {
                self.view.end -= count;
                // and scroll into view
                if let Some(count) = self.view.pos.checked_sub(self.view.end - 1) {
                    self.view.start += count;
                    self.view.end += count;
                    self.view.pos -= count;
                }
            } else {
                let count = height - self.height;
                self.view.end += count;
            }
            self.height = height;
            self.update_offset();
        }
    }

    // returns view which is constrained by current height
    pub fn get_view(self) -> View {
        let mut v = self.view;
        v.end = std::cmp::min(self.length, self.view.end);
        v
    }

    pub fn select(&mut self, position: usize) {
        if let Some(count) = self.position().checked_sub(position) {
            self.prev(count);
        } else {
            self.next(position - self.position());
        }
    }

    fn update_offset(&mut self) {
        self.offset = if 2 * self.user_offset > self.height {
            self.height.checked_div(2).unwrap_or(0)
        } else {
            self.user_offset
        }
    }

    pub fn center(&mut self) {
        if self.height == 0 {
            return;
        }
        let middle = self.height / 2;
        if let Some(count) = self.view.pos.checked_sub(middle) {
            self.view.pos = middle;
            self.view.start += count;
            self.view.end += count;
        } else {
            let count = middle - self.view.pos;

            let old_start = self.view.start;
            self.view.start = self.view.start.saturating_sub(count);
            let real_count = old_start - self.view.start;
            self.view.pos += real_count;
            self.view.end -= real_count;
        }
    }
    /// Returns position as seen in current view or None if not in view.
    pub fn view_position(&self, position: usize) -> Option<usize> {
        let pos = position.checked_sub(position)?;
        if pos < self.height {
            Some(pos)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod test {
    use super::{StatefulPosition, View};

    macro_rules! assert_pos {
        ($current:ident, $slice_pos:expr, $slice_start:expr) => {
            assert_eq!($current.position(), $slice_pos + $slice_start);
            let slice_end = std::cmp::min($current.length, $slice_start + $current.height);
            $current.set_height($current.height);
            assert_eq!(
                $current.get_view(),
                View {
                    pos: $slice_pos,
                    start: $slice_start,
                    end: slice_end
                }
            );
        };
    }

    #[test]
    fn test_next() {
        let mut current = StatefulPosition::default();

        macro_rules! next {
            ($count:expr, $slice_pos:expr, $slice_start:expr) => {
                current.next($count);
                assert_pos!(current, $slice_pos, $slice_start);
            };
        }

        current.reset(5, 40);
        current.set_height(20);
        assert_pos!(current, 0, 0);

        next!(1, 1, 0);
        next!(1, 2, 0);
        next!(1, 3, 0);
        next!(1, 4, 0);
        next!(1, 5, 0);
        // no real change in next 9 items
        next!(9, 14, 0);
        // first scroll where we hit offset
        next!(1, 14, 1);
        next!(1, 14, 2);
        next!(1, 14, 3);
        next!(1, 14, 4);
        // here we'll hit end of list
        next!(15, 14, 19);
        next!(1, 14, 20);
        // and here we have no more elements to offset
        next!(1, 15, 20);
        next!(1, 16, 20);
        next!(1, 17, 20);
        next!(1, 18, 20);
        next!(1, 19, 20);
        // end of list
        next!(1, 19, 20);
        next!(10, 19, 20);
    }

    #[test]
    fn test_prev() {
        let mut current = StatefulPosition::default();

        macro_rules! prev {
            ($count:expr, $slice_pos:expr, $slice_start:expr) => {
                current.prev($count);
                assert_pos!(current, $slice_pos, $slice_start);
            };
        }

        current.reset(5, 40);
        current.set_height(20);
        current.end();
        assert_pos!(current, 19, 20);
        prev!(1, 18, 20);
        prev!(1, 17, 20);
        prev!(1, 16, 20);
        prev!(12, 4, 20);
        // here we hit offset
        prev!(1, 4, 19);
        prev!(1, 4, 18);
        prev!(18, 4, 0);
        // here we cannot offset anymore
        prev!(1, 3, 0);
        prev!(1, 2, 0);
        prev!(1, 1, 0);
        prev!(1, 0, 0);
        // no scroll past it
        prev!(1, 0, 0);
        prev!(10, 0, 0);
    }

    #[test]
    fn test_up_down() {
        let mut current = StatefulPosition::default();
        current.reset(5, 40);
        current.set_height(20);
        assert_pos!(current, 0, 0);

        current.next(10);
        assert_pos!(current, 10, 0);
        current.next(10);
        assert_pos!(current, 14, 6);
        current.prev(5);
        assert_pos!(current, 9, 6);
        current.prev(10);
        assert_pos!(current, 4, 1);

        current.start();
        assert_pos!(current, 0, 0);
        current.end();
        assert_pos!(current, 19, 20);
    }

    #[test]
    fn test_small_list() {
        let mut current = StatefulPosition::default();
        current.reset(5, 15);
        current.set_height(20);
        assert_pos!(current, 0, 0);
        for i in 1..14 {
            current.next(1);
            assert_pos!(current, i, 0);
        }
        current.next(1);
        assert_pos!(current, 14, 0);
        for i in (0..14).rev() {
            current.prev(1);
            assert_pos!(current, i, 0);
        }
        current.prev(1);
        assert_pos!(current, 0, 0);

        current.end();
        assert_pos!(current, 14, 0);
        current.start();
        assert_pos!(current, 0, 0);
    }

    #[test]
    fn zero_len_list() {
        let mut current = StatefulPosition::default();
        current.reset(5, 0);
        current.set_height(20);
        assert_pos!(current, 0, 0);
        current.next(1);
        assert_pos!(current, 0, 0);
        current.prev(1);
        assert_pos!(current, 0, 0);
        current.end();
        assert_pos!(current, 0, 0);
        current.start();
        assert_pos!(current, 0, 0);
    }

    #[test]
    fn center() {
        let mut current = StatefulPosition::default();
        current.reset(5, 40);
        current.set_height(20);
        assert_pos!(current, 0, 0);
        current.select(25);
        assert_pos!(current, 14, 11);
        current.center();
        assert_pos!(current, 10, 15);
        current.select(35);
        assert_pos!(current, 15, 20);
        current.prev(10);
        assert_pos!(current, 5, 20);
        current.center();
        assert_pos!(current, 10, 15);
    }
}
