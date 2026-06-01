use std::collections::VecDeque;

/// Append/drain buffer of pending domain events held by an aggregate.
#[derive(Clone, Debug)]
pub struct EventCollection<E> {
    events: VecDeque<E>,
}

impl<E> EventCollection<E> {
    pub fn new() -> Self {
        Self {
            events: VecDeque::new(),
        }
    }

    pub fn record(&mut self, event: E) {
        self.events.push_back(event);
    }

    pub fn pull(&mut self) -> Vec<E> {
        self.events.drain(..).collect()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl<E> Default for EventCollection<E> {
    fn default() -> Self {
        Self::new()
    }
}
