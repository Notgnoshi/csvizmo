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
    if data.len().is_multiple_of(2) {
        average(data[mid - 1], data[mid])
    } else {
        data[mid]
    }
}

/// Calculate Q1, Q2 (median), and Q3 quartiles of the given data
///
/// Requires the data be sorted in ascending order. Any NaNs sorted to the top with
/// [nan_safe_sort()] will be excluded.
pub fn quartiles(data: &[f64]) -> Option<(f64, f64, f64)> {
    let mut num_nans: usize = 0;
    for maybe_nan in data.iter().rev() {
        if maybe_nan.is_nan() {
            num_nans += 1;
        } else {
            break;
        }
    }

    if num_nans >= data.len() {
        return None;
    }

    let upper = data.len() - num_nans;
    quartiles_impl(&data[..upper])
}

fn quartiles_impl(data: &[f64]) -> Option<(f64, f64, f64)> {
    if data.len() < 3 {
        return None;
    }

    let mid = data.len() / 2;
    if data.len().is_multiple_of(2) {
        Some((median(&data[0..mid]), median(data), median(&data[mid..])))
    } else {
        Some((
            median(&data[0..mid]),
            median(data),
            median(&data[mid + 1..]),
        ))
    }
}

#[derive(Debug, Clone)]
pub struct OnlineStats {
    pub filename: String,
    pub colname: String,
    pub num: usize,
    pub num_filtered: usize,
    pub mean: f64,
    m2: f64,
    pub sum: f64,
    pub min: f64,
    pub min_index: usize,
    pub max: f64,
    pub max_index: usize,
    pub q1: Option<f64>,
    pub median: Option<f64>,
    pub q3: Option<f64>,
}

impl std::fmt::Display for OnlineStats {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        // quartiles are computed by from_unsorted(), which isn't purely online, so they're not
        // guaranteed to be present.
        let q1 = if let Some(q1) = &self.q1 {
            format!("{q1}")
        } else {
            String::new()
        };
        let median = if let Some(median) = &self.median {
            format!("{median}")
        } else {
            String::new()
        };
        let q3 = if let Some(q3) = &self.q3 {
            format!("{q3}")
        } else {
            String::new()
        };

        write!(
            f,
            "\"{}\",\"{}\",{},{},{},{},{},{},{},{},{},{},{}",
            self.filename,
            self.colname,
            self.num,
            self.num_filtered,
            self.min,
            self.min_index,
            self.max,
            self.max_index,
            self.mean,
            self.stddev(),
            q1,
            median,
            q3,
        )
    }
}

impl OnlineStats {
    pub fn get_csv_header() -> &'static str {
        "filename,colname,count,filtered,min,min-index,max,max-index,mean,stddev,Q1,median,Q3"
    }
    pub fn new(filename: impl Into<String>, colname: impl Into<String>) -> Self {
        Self {
            filename: filename.into(),
            colname: colname.into(),
            num: 0,
            num_filtered: 0,
            mean: 0.0,
            m2: 0.0,
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
    /// skipped by this method. If you want quartile measurement, use [Self::from_unsorted_mut] or
    /// [Self::from_sorted].
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
    }

    #[inline]
    #[must_use]
    pub fn variance(&self) -> f64 {
        // Delta degrees of freedom.
        // Some stddev calculations use ddof=0 (numpy) and some use ddof=1 (scipy, R).
        const DDOF: f64 = 1.0;
        self.m2 / (self.num as f64 - DDOF)
    }

    #[inline]
    #[must_use]
    pub fn stddev(&self) -> f64 {
        self.variance().sqrt()
    }

    /// Will sort the data in-place and calculate summary stats and quartiles
    pub fn from_unsorted_mut(
        filename: impl Into<String>,
        colname: impl Into<String>,
        data: &mut [f64],
        min: Option<f64>,
        max: Option<f64>,
    ) -> Self {
        let mut this = Self::from_unsorted_iter(filename, colname, data.iter(), min, max);

        // sound, because OrderedFloat is a repr(transparent) newtype
        let data = unsafe {
            std::mem::transmute::<&mut [f64], &mut [ordered_float::OrderedFloat<f64>]>(data)
        };
        data.sort_unstable();
        let data = unsafe {
            std::mem::transmute::<&mut [ordered_float::OrderedFloat<f64>], &mut [f64]>(data)
        };

        // TODO: Investigate online quartile estimation algorithms. There doesn't seem to be a
        // "here's the answer" algorithm, and there seems to be lots of possible ones to pick from.
        // t-digest seems promising? https://github.com/tdunning/t-digest although it looks like I
        // may need to write my own online implementation.
        //
        // I think the public API of this OnlineStats tool could use some polishing. If I can get
        // an online t-digest, then it should use an online-only API, and throw out the &[f64] APIs
        // entirely.
        //
        // Since csvstats reads the CSV data into memory anyways, sorting and doing the "real"
        // quartile calculation is probably the best choice for it, but other tools might benefit
        // from a real online version.
        if let Some((q1, q2, q3)) = quartiles(data) {
            this.q1 = Some(q1);
            this.median = Some(q2);
            this.q3 = Some(q3);
        }
        this
    }

    /// Given sorted data, will calculate both summary stats and quartiles
    pub fn from_sorted(
        filename: impl Into<String>,
        colname: impl Into<String>,
        data: &[f64],
        min: Option<f64>,
        max: Option<f64>,
    ) -> Self {
        let mut this = Self::from_unsorted_iter(filename, colname, data.iter(), min, max);
        if let Some((q1, q2, q3)) = quartiles(data) {
            this.q1 = Some(q1);
            this.median = Some(q2);
            this.q3 = Some(q3);
        }
        this
    }

    /// Given unsorted data, will calculate summary stats, skipping quartile calculations
    pub fn from_unsorted_iter<'v, V>(
        filename: impl Into<String>,
        colname: impl Into<String>,
        data: V,
        min: Option<f64>,
        max: Option<f64>,
    ) -> Self
    where
        V: Iterator<Item = &'v f64>,
    {
        let mut stats = Self::new(filename, colname);
        for sample in data {
            if let Some(min) = min
                && *sample < min
            {
                stats.num_filtered += 1;
                continue;
            }
            if let Some(max) = max
                && *sample > max
            {
                stats.num_filtered += 1;
                continue;
            }
            stats.update(*sample);
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
}
