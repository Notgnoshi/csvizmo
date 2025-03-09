use gnuplot::{AutoOption, Axes2D, AxesCommon, PlotOption};
use ordered_float::OrderedFloat;

use crate::counter::Counter;
use crate::stats::OnlineStats;

pub trait Axes2DExt {
    fn histplot_discrete(
        &mut self,
        x: Vec<f64>,
        stats: &OnlineStats,
        min: Option<f64>,
        max: Option<f64>,
        num_bins: Option<usize>,
    ) -> &mut Self;

    fn histplot_continuous(
        &mut self,
        x: Vec<f64>,
        stats: &OnlineStats,
        min: Option<f64>,
        max: Option<f64>,
        num_bins: Option<usize>,
    ) -> &mut Self;
}

impl Axes2DExt for Axes2D {
    fn histplot_discrete(
        &mut self,
        x: Vec<f64>,
        stats: &OnlineStats,
        min: Option<f64>,
        max: Option<f64>,
        num_bins: Option<usize>,
    ) -> &mut Self {
        let min = if let Some(m) = min { m } else { stats.min };
        let max = if let Some(m) = max { m } else { stats.max };

        let x = unsafe { std::mem::transmute::<Vec<f64>, Vec<OrderedFloat<f64>>>(x) };
        let counter = Counter::new(x);

        let num_bins = num_bins.unwrap_or(counter.len());
        let bin_width = (max - min) / num_bins as f64;

        let max_count = if let Some((_, max_count)) = counter.single_most_common() {
            *max_count as f64
        } else {
            0.0
        };

        let (x, counts): (Vec<_>, Vec<_>) = counter.into_iter().unzip();
        let x = unsafe { std::mem::transmute::<Vec<OrderedFloat<f64>>, Vec<f64>>(x) };
        let widths = std::iter::repeat(bin_width);

        self.set_y_range(AutoOption::Fix(0.0), AutoOption::Fix(max_count + 0.4));
        self.set_x_range(
            AutoOption::Fix(min - 0.1 * stats.stddev()),
            AutoOption::Fix(max + 0.1 * stats.stddev()),
        );
        self.set_y_label("Count", &[]);
        self.boxes_set_width(x, counts, widths, &[PlotOption::BorderColor("black")])
    }

    fn histplot_continuous(
        &mut self,
        x: Vec<f64>,
        stats: &OnlineStats,
        min: Option<f64>,
        max: Option<f64>,
        num_bins: Option<usize>,
    ) -> &mut Self {
        let min = if let Some(m) = min { m } else { stats.min };
        let max = if let Some(m) = max { m } else { stats.max };

        // If number of bins is given, then linspace the range [min..max]. Otherwise use the
        // Freedman-Diaconis rule to calculate the binwidth.
        let (bin_width, num_bins) = if let Some(num_bins) = num_bins {
            let bin_width = (max - min) / (num_bins as f64);
            (bin_width, num_bins)
        } else {
            // https://en.wikipedia.org/wiki/Freedman%E2%80%93Diaconis_rule
            let iqr = stats.q3.unwrap() - stats.q1.unwrap();
            let bin_width = 2.0 * iqr / (stats.num as f64).cbrt();

            let num_bins = (max - min) / bin_width;
            let num_bins = num_bins.ceil() as usize;

            // Not much of a histogram with <=3 bins
            if num_bins < 4 {
                let num_bins = 4;
                let bin_width = (max - min) / (num_bins as f64);
                (bin_width, num_bins)
            } else {
                (bin_width, num_bins)
            }
        };
        tracing::info!("Using {num_bins} bins with width {bin_width:.4}");

        let mut counts = vec![0; num_bins];

        let mut bin_centers = Vec::with_capacity(num_bins);
        let first_bin_center = min + 0.5 * bin_width;
        for i in 0..num_bins {
            let center = first_bin_center + i as f64 * bin_width;
            bin_centers.push(center);
        }

        let mut max_count = 0;
        for value in x {
            // This calculation uses an inclusive LHS and exclusive RHS. That's what we want
            // everywhere except the far RHS endpoint.
            let bin_index = (value - min) / bin_width;
            let mut bin_index = bin_index.floor() as usize;
            assert!(bin_index <= num_bins);
            if bin_index == num_bins {
                bin_index = num_bins - 1;
            }

            counts[bin_index] += 1;
            if counts[bin_index] > max_count {
                max_count = counts[bin_index];
            }
        }

        let widths = std::iter::repeat(bin_width);

        self.set_y_range(
            AutoOption::Fix(0.0),
            AutoOption::Fix(max_count as f64 + 0.4),
        );
        self.set_x_range(
            AutoOption::Fix(min - 0.1 * stats.stddev()),
            AutoOption::Fix(max + 0.1 * stats.stddev()),
        );
        self.set_y_label("Count", &[]);
        self.boxes_set_width(
            bin_centers,
            counts,
            widths,
            &[PlotOption::BorderColor("black")],
        )
    }
}
