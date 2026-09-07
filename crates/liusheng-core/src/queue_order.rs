//! Queue-index ordering stays separate from the visible queue and audio decoding.
#[derive(Debug)]
pub struct PlaybackOrder {
    order: Vec<usize>,
    seed: u64,
}
impl Default for PlaybackOrder {
    fn default() -> Self {
        Self {
            order: Vec::new(),
            seed: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(1)
                | 1,
        }
    }
}
impl PlaybackOrder {
    pub fn rebuild(&mut self, len: usize, current: usize, shuffle: bool) {
        self.order = (0..len).collect();
        if shuffle {
            for i in (1..len).rev() {
                self.seed ^= self.seed << 13;
                self.seed ^= self.seed >> 7;
                self.seed ^= self.seed << 17;
                self.order.swap(i, (self.seed as usize) % (i + 1));
            }
            if let Some(i) = self.order.iter().position(|x| *x == current) {
                self.order.swap(0, i);
            }
        }
    }
    pub fn next(&self, current: usize, repeat: u8, manual: bool) -> Option<usize> {
        if !manual && repeat == 1 {
            return (!self.order.is_empty()).then_some(current);
        }
        let i = self.order.iter().position(|x| *x == current)?;
        self.order
            .get(i + 1)
            .copied()
            .or_else(|| (repeat == 2).then(|| self.order.first().copied()).flatten())
    }
    pub fn previous(&self, current: usize, repeat: u8) -> Option<usize> {
        let i = self.order.iter().position(|x| *x == current)?;
        if i > 0 {
            self.order.get(i - 1).copied()
        } else if repeat == 2 {
            self.order.last().copied()
        } else {
            Some(current)
        }
    }
    pub fn promote_next(&mut self, current: usize, next: usize) {
        let Some(from) = self.order.iter().position(|i| *i == next) else {
            return;
        };
        self.order.remove(from);
        let to = self
            .order
            .iter()
            .position(|i| *i == current)
            .map(|i| i + 1)
            .unwrap_or(0);
        self.order.insert(to, next);
    }
}
pub fn moved_index(index: usize, from: usize, to: usize) -> usize {
    if index == from {
        to
    } else if from < index && index <= to {
        index - 1
    } else if to <= index && index < from {
        index + 1
    } else {
        index
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeat_and_manual_next_have_distinct_semantics() {
        let mut o = PlaybackOrder::default();
        o.rebuild(3, 0, false);
        assert_eq!(o.next(0, 1, false), Some(0));
        assert_eq!(o.next(0, 1, true), Some(1));
        assert_eq!(o.next(2, 2, false), Some(0));
        assert_eq!(o.next(2, 0, false), None);
    }
    #[test]
    fn shuffle_visits_each_index_once() {
        let mut o = PlaybackOrder::default();
        o.rebuild(100, 37, true);
        let mut seen = vec![37];
        let mut current = 37;
        while let Some(i) = o.next(current, 0, true) {
            seen.push(i);
            current = i;
        }
        seen.sort();
        assert_eq!(seen, (0..100).collect::<Vec<_>>());
    }
    #[test]
    fn index_moves_follow_the_same_track() {
        assert_eq!(moved_index(2, 0, 3), 1);
        assert_eq!(moved_index(0, 0, 3), 3);
        assert_eq!(moved_index(1, 3, 0), 2);
    }
}
