use std::{collections::VecDeque, slice::Iter};

pub trait Selectable {
    fn selected(&self) -> bool;
    fn toggle_selected(&mut self);
}

pub struct StatefulList<T>
{
    list: Vec<T>,
    state: scrollview::StatefulPosition,
    /// list of elements currently selected
    selections: VecDeque<usize>,
}

impl<T> StatefulList<T>
{
    pub fn new() -> Self {
        Self {
            list: Vec::new(),
            state: scrollview::StatefulPosition::default(),
            selections: VecDeque::default(),
        }
    }

    pub fn reset(&mut self) {
        self.list = Vec::new();
        // TODO: offset should be part of API one day
        self.state.reset(5, 0);
        self.selections.clear();
    }

    pub fn push(&mut self, mut data: Vec<T>) {
        self.list.append(&mut data);
        self.state.length_extended(self.list.len());
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn scroll_next(&mut self, count: usize) {
        self.state.next(count)
    }

    pub fn scroll_prev(&mut self, count: usize) {
        self.state.prev(count)
    }

    pub fn scroll_start(&mut self) {
        self.state.start()
    }

    pub fn scroll_end(&mut self) {
        self.state.end()
    }

    pub fn scroll_to_position(&mut self, pos: usize) {
        self.state.select(pos);
    }

    pub fn has_selected(&self) -> bool {
        !self.selections.is_empty()
    }

    pub fn selected0<'a: 'b, 'b>(&'a self) -> Option<&'b T> {
        if !self.selections.is_empty() {
            let idx0 = self.selections[0];
            return Some(&self.list[idx0]);
        }
        None
    }

    pub fn set_view_height(&mut self, height: u16) {
        self.state.set_height(height as usize)
    }

    // Returns a position and view as iterator to slice of data.
    // Note: Position is returned with view to avoid using out-of-date position with this view.
    pub fn iter_view(&self) -> (usize, impl Iterator<Item = &T>) {
        let view = self.state.get_view();
        let iter = self
            .list
            .iter()
            // take slice from iter [start..end]
            .take(view.end)
            .skip(view.start);
        (view.pos, iter)
    }

    pub fn iter_all(&self) -> Iter<'_, T> {
        self.list.iter()
    }

    pub fn current(&self) -> Option<&T> {
        let selected = self.state.position();
        self.list.get(selected)
    }

    pub fn current_position(&self) -> usize {
        self.state.position()
    }

    fn current_mut(&mut self) -> Option<&mut T> {
        let selected = self.state.position();
        self.list.get_mut(selected)
    }

    pub fn toggle_select_for_current(&mut self) -> Option<()>
    where
        T: Selectable
    {
        let pos = self.state.position();

        self.current_mut()?.toggle_selected();

        if self.current()?.selected() {
            if !self.selections.is_empty() {
                let x = self.selections.pop_front().unwrap();
                self.list[x].toggle_selected()
            }
            self.selections.push_back(pos);
        } else {
            self.selections.retain(|e| *e != pos);
        }

        Some(())
    }

    pub fn center(&mut self) {
        self.state.center()
    }
}
