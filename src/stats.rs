#[inline]
#[must_use]
fn average(a: f64, b: f64) -> f64 {
    (a + b) / 2.0
}

#[inline]
#[must_use]
pub fn median(data: &[f64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }

    let mid = data.len() / 2;
    if data.len() % 2 == 0 {
        average(data[mid - 1], data[mid])
    } else {
        data[mid]
    }
}

/// Calculate Q1, Q2 (median), and Q3 quartiles of the given data
///
/// Requires the data be sorted in ascending order.
pub fn quartiles(data: &[f64]) -> Option<(f64, f64, f64)> {
    if data.len() < 3 {
        return None;
    }

    let mid = data.len() / 2;
    if data.len() % 2 == 0 {
        Some((median(&data[0..mid]), median(data), median(&data[mid..])))
    } else {
        Some((
            median(&data[0..mid]),
            median(data),
            median(&data[mid + 1..]),
        ))
    }
}

pub fn nan_safe_sort(data: &mut [f64]) {
    data.sort_unstable_by(|a, b| {
        // This sorts the NaNs to the upper range
        if a.is_nan() && b.is_nan() {
            std::cmp::Ordering::Equal
        } else if a.is_nan() {
            std::cmp::Ordering::Greater
        } else if b.is_nan() {
            std::cmp::Ordering::Less
        } else {
            a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
        }
    });
}

pub struct OnlineStats {
    ddof: f64,
    pub num: usize,
    pub num_filtered: usize,
    pub mean: f64,
    m2: f64,
    pub variance: f64,
    pub stddev: f64,
    pub sum: f64,
    pub min: f64,
    pub min_index: usize,
    pub max: f64,
    pub max_index: usize,
    pub q1: Option<f64>,
    pub median: Option<f64>,
    pub q3: Option<f64>,
}

impl OnlineStats {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            ddof: 1.0,
            num: 0,
            num_filtered: 0,
            mean: 0.0,
            m2: 0.0,
            variance: 0.0,
            stddev: 0.0,
            sum: 0.0,
            min: f64::MAX,
            min_index: 0,
            max: f64::MIN,
            max_index: 0,
            q1: None,
            median: None,
            q3: None,
        }
    }

    /// An online variance update
    ///
    /// NOTE: the q1, q2 (median), and q3 quartiles cannot be updated in an online manner, and are
    /// skipped by this method. If you want quartile measurement, use [Self::from_sorted] or
    /// [Self::from_unsorted].
    ///
    /// TODO: It appears there are online *estimation* algorithms for quantiles. But since I have
    /// to collect the data stream into a Vec for histogram plotting anyways, an online algorithm
    /// is pointless except for bragging rights.
    pub fn update(&mut self, sample: f64) {
        if sample.is_nan() {
            self.num_filtered += 1;
            return;
        }

        self.sum += sample;
        if sample > self.max {
            self.max = sample;
            self.max_index = self.num + self.num_filtered;
        }
        if sample < self.min {
            self.min = sample;
            self.min_index = self.num + self.num_filtered;
        }
        self.num += 1;
        let delta = sample - self.mean;
        self.mean += delta / self.num as f64;
        self.m2 += delta * delta;

        self.variance = self.m2 / (self.num as f64 - self.ddof);
        self.stddev = self.variance.sqrt();
    }

    pub fn from_sorted(data: &[f64], min: Option<f64>, max: Option<f64>) -> Self {
        let mut stats = Self::new();

        // TODO: Find the min/max value indices, and run the online variance (and quartile)
        // calculations on the filtered range.

        for sample in data {
            if let Some(min) = min {
                if *sample < min {
                    stats.num_filtered += 1;
                    continue;
                }
            }
            if let Some(max) = max {
                if *sample < max {
                    stats.num_filtered += 1;
                    continue;
                }
            }
            stats.update(*sample);
        }

        if let Some((q1, q2, q3)) = quartiles(data) {
            stats.q1 = Some(q1);
            stats.median = Some(q2);
            stats.q3 = Some(q3);
        }

        stats
    }

    pub fn from_unsorted(data: &mut [f64], min: Option<f64>, max: Option<f64>) -> Self {
        nan_safe_sort(data);
        Self::from_sorted(data, min, max)
    }

    // Skips quartiles
    pub fn from_unsorted_iter<'v, V>(values: V) -> Self
    where
        V: Iterator<Item = &'v f64>,
    {
        let mut stats = Self::new();
        for value in values {
            stats.update(*value);
        }
        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_three_quartiles() {
        let data = [0.0, 1.0, 2.0];
        let qs = quartiles(&data).unwrap();
        assert_eq!(qs, (0.0, 1.0, 2.0));
    }

    #[test]
    fn test_even_quartiles() {
        let data = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
        let qs = quartiles(&data).unwrap();
        assert_eq!(qs, (1.0, 2.5, 4.0));
    }

    #[test]
    fn test_odd_quartiles() {
        let data = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let qs = quartiles(&data).unwrap();
        assert_eq!(qs, (1.0, 3.0, 5.0));
    }

    #[test]
    fn test_sort_floats() {
        let mut data = [1.0, 0.0, -0.0, 2.0];
        let expected = [-0.0, 0.0, 1.0, 2.0];
        nan_safe_sort(&mut data);
        assert_eq!(data, expected);
    }

    #[test]
    fn test_sort_floats_with_nans() {
        let mut data = [
            3.0,
            -1.0,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NAN,
            -0.0,
            0.0,
            f64::NAN,
            2.0,
        ];
        let expected = [f64::NEG_INFINITY, -1.0, -0.0, 0.0, 2.0, 3.0, f64::INFINITY];
        nan_safe_sort(&mut data);
        assert_eq!(data[..7], expected);
        // NaNs don't compare equal, so filter them out
        assert!(data[7].is_nan());
        assert!(data[8].is_nan());
    }
}
