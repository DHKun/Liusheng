//! Queue-index ordering stays separate from the visible queue and audio decoding.
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlaybackOrder {
    order: Arc<Vec<usize>>,
    seed: u64,
}
impl Default for PlaybackOrder {
    fn default() -> Self {
        Self {
            order: Arc::new(Vec::new()),
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
        let mut order: Vec<usize> = (0..len).collect();
        if shuffle {
            for i in (1..len).rev() {
                self.seed ^= self.seed << 13;
                self.seed ^= self.seed >> 7;
                self.seed ^= self.seed << 17;
                order.swap(i, (self.seed as usize) % (i + 1));
            }
            if let Some(i) = order.iter().position(|x| *x == current) {
                order.swap(0, i);
            }
        }
        self.order = Arc::new(order);
    }

    pub fn is_valid_for(&self, len: usize) -> bool {
        if self.order.len() != len {
            return false;
        }
        let mut seen = vec![false; len];
        self.order.iter().all(|&index| {
            if index >= len || seen[index] {
                return false;
            }
            seen[index] = true;
            true
        })
    }

    pub fn sequence(&self) -> &[usize] {
        &self.order
    }

    /// Queue edits remap identities while keeping the established shuffled history.
    pub fn insert(&mut self, index: usize, current: usize, shuffle: bool, next: bool) {
        let len = self.order.len();
        if index > len {
            return;
        }
        if shuffle {
            let order = Arc::make_mut(&mut self.order);
            for entry in order.iter_mut() {
                if *entry >= index {
                    *entry += 1;
                }
            }
            order.push(index);
        } else {
            self.rebuild(len + 1, current, false);
        }
        if next {
            self.promote_next(current, index);
        }
    }

    /// Resolve the surviving current entry before mutating positional IDs.
    pub fn current_after_removal(
        &self,
        current: usize,
        removed: usize,
        repeat: u8,
    ) -> Option<usize> {
        if self.order.len() <= 1 {
            return None;
        }
        let chosen = if current == removed {
            self.next(current, repeat, true)
                .filter(|i| *i != removed)
                .or_else(|| self.previous(current, repeat).filter(|i| *i != removed))?
        } else {
            current
        };
        Some(chosen - usize::from(chosen > removed))
    }

    pub fn remove(&mut self, index: usize, shuffle: bool) {
        if index >= self.order.len() {
            return;
        }
        let order = Arc::make_mut(&mut self.order);
        order.retain(|entry| *entry != index);
        for entry in order.iter_mut() {
            if *entry > index {
                *entry -= 1;
            }
        }
        if !shuffle {
            order.sort_unstable();
        }
    }

    pub fn move_item(&mut self, from: usize, to: usize, shuffle: bool) {
        if from >= self.order.len() || to >= self.order.len() {
            return;
        }
        let order = Arc::make_mut(&mut self.order);
        for entry in order.iter_mut() {
            *entry = moved_index(*entry, from, to);
        }
        if !shuffle {
            order.sort_unstable();
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
        Arc::make_mut(&mut self.order).remove(from);
        let to = self
            .order
            .iter()
            .position(|i| *i == current)
            .map(|i| i + 1)
            .unwrap_or(0);
        Arc::make_mut(&mut self.order).insert(to, next);
    }
}
/// Authoritative navigation projection. It is cheap to share across threads;
/// restored and active playback use these same ordering rules.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NavigationState {
    pub order: PlaybackOrder,
    pub current: usize,
    pub repeat: u8,
    pub shuffle: bool,
}
impl NavigationState {
    pub fn next(&self) -> Option<usize> {
        self.order.next(self.current, self.repeat, true)
    }
    pub fn previous(&self) -> Option<usize> {
        self.order.previous(self.current, self.repeat)
    }
    pub fn can_next(&self) -> bool {
        self.next().is_some_and(|i| i != self.current)
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

#[cfg(test)]
mod persistence_tests {
    use super::*;
    #[test]
    fn shuffled_edits_preserve_history_and_duplicate_entry_positions() {
        let mut order = PlaybackOrder::default();
        order.rebuild(8, 3, true);
        let original = order.sequence().to_vec();
        let copy = order.clone();
        order.insert(8, 3, true, false);
        assert_eq!(&order.sequence()[..8], original.as_slice());
        assert_eq!(copy.sequence(), original.as_slice());
        order.move_item(2, 6, true);
        assert_eq!(
            order.sequence(),
            original
                .iter()
                .copied()
                .chain([8])
                .map(|i| moved_index(i, 2, 6))
                .collect::<Vec<_>>()
        );
        order.remove(6, true);
        assert!(order.is_valid_for(8));
        let roundtrip: PlaybackOrder =
            serde_json::from_str(&serde_json::to_string(&order).unwrap()).unwrap();
        assert_eq!(order, roundtrip);
    }
    #[test]
    fn capabilities_use_actual_shuffle_position_and_manual_repeat_semantics() {
        let mut order = PlaybackOrder::default();
        order.rebuild(5, 4, true);
        let mut nav = NavigationState {
            order,
            current: 4,
            repeat: 0,
            shuffle: true,
        };
        assert!(nav.can_next());
        nav.current = *nav.order.sequence().last().unwrap();
        assert!(!nav.can_next());
        nav.repeat = 2;
        assert!(nav.can_next());
        nav.order.rebuild(1, 0, false);
        nav.current = 0;
        assert!(!nav.can_next());
    }
    #[test]
    fn malformed_persisted_order_is_rejected() {
        for indices in [vec![0, 0], vec![0, 2], vec![], vec![0, 1, 2]] {
            let order = PlaybackOrder {
                order: Arc::new(indices),
                ..Default::default()
            };
            assert!(!order.is_valid_for(2));
        }
    }
}
