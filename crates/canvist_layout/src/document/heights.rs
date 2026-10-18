//! Row extents in a Fenwick tree, so a height change and a position lookup
//! both cost `O(log rows)`.

/// Prefix sums over row extents (leading space plus height) in fixed-point
/// units.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Heights {
	values: Vec<i64>,
	/// One-based Fenwick tree over `values`.
	tree: Vec<i64>,
}

impl Heights {
	/// Build from row extents in `O(n)`.
	pub(crate) fn new(values: Vec<i64>) -> Self {
		let mut tree = vec![0; values.len() + 1];

		for (index, value) in values.iter().enumerate() {
			let node = index + 1;
			tree[node] += value;
			let parent = node + lowest_bit(node);

			if parent < tree.len() {
				tree[parent] += tree[node];
			}
		}

		Self { values, tree }
	}

	/// The extent of row `index`.
	pub(crate) fn get(&self, index: usize) -> i64 {
		self.values.get(index).copied().unwrap_or(0)
	}

	/// Replace the extent of row `index`.
	pub(crate) fn set(&mut self, index: usize, value: i64) {
		let Some(slot) = self.values.get_mut(index) else {
			return;
		};
		let delta = value - *slot;
		*slot = value;

		if delta == 0 {
			return;
		}

		let mut node = index + 1;

		while node < self.tree.len() {
			self.tree[node] += delta;
			node += lowest_bit(node);
		}
	}

	/// The sum of the extents of rows `0..index`.
	pub(crate) fn prefix(&self, index: usize) -> i64 {
		let mut node = index.min(self.values.len());
		let mut sum = 0;

		while node > 0 {
			sum += self.tree[node];
			node -= lowest_bit(node);
		}

		sum
	}

	/// The sum of every extent.
	pub(crate) fn total(&self) -> i64 {
		self.prefix(self.values.len())
	}

	/// The row whose extent contains `y`: the last row starting at or before
	/// `y`, clamped to the first and last rows.
	pub(crate) fn find(&self, y: i64) -> usize {
		let mut position = 0;
		let mut remaining = y;
		let mut step = self.tree.len().next_power_of_two() / 2;

		while step > 0 {
			let next = position + step;

			if next < self.tree.len() && self.tree[next] <= remaining {
				position = next;
				remaining -= self.tree[next];
			}

			step /= 2;
		}

		position.min(self.values.len().saturating_sub(1))
	}
}

fn lowest_bit(node: usize) -> usize {
	node & node.wrapping_neg()
}

#[cfg(test)]
#[allow(
	clippy::float_cmp,
	reason = "the values compared are exactly representable"
)]
mod tests {
	use super::*;

	#[test]
	fn prefix_sums_track_updates() {
		let mut heights = Heights::new(vec![10, 20, 30, 40]);

		assert_eq!(heights.prefix(0), 0);
		assert_eq!(heights.prefix(3), 60);
		assert_eq!(heights.total(), 100);

		let shrunk = heights.get(0) / 2;
		heights.set(1, shrunk);
		assert_eq!(heights.prefix(2), 15);
		assert_eq!(heights.total(), 85);
	}

	#[test]
	fn find_returns_the_containing_row() {
		let heights = Heights::new(vec![10, 20, 30, 40]);

		assert_eq!(heights.find(-5), 0);
		assert_eq!(heights.find(0), 0);
		assert_eq!(heights.find(9), 0);
		assert_eq!(heights.find(10), 1);
		assert_eq!(heights.find(59), 2);
		assert_eq!(heights.find(60), 3);
		assert_eq!(heights.find(1_000), 3);
	}

	#[test]
	fn zero_height_rows_are_skipped_by_find() {
		let heights = Heights::new(vec![10, 0, 10]);

		assert_eq!(heights.find(10), 2);
	}

	#[test]
	fn matches_naive_sums() {
		let values: Vec<i64> = (0..37).map(|i| (i * 7) % 11).collect();
		let heights = Heights::new(values.clone());

		for index in 0..=values.len() {
			assert_eq!(heights.prefix(index), values[..index].iter().sum::<i64>());
		}
	}
}
