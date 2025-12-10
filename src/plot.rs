use gnuplot::{AutoOption, Axes2D, AxesCommon, ColorType, PlotOption};
use kernel_density_estimation::prelude::*;
use ordered_float::OrderedFloat;

use crate::counter::Counter;
use crate::stats::OnlineStats;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Distribution {
    Cosine,
    Epanechnikov,
    Logistic,
    #[default]
    Normal,
    Quartic,
    Silverman,
    Triangular,
    Tricube,
    Triweight,
    Uniform,
}

impl std::fmt::Display for Distribution {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            // important: Should match clap::ValueEnum format
            Self::Cosine => write!(f, "cosine"),
            Self::Epanechnikov => write!(f, "epanechnikov"),
            Self::Logistic => write!(f, "logistic"),
            Self::Normal => write!(f, "normal"),
            Self::Quartic => write!(f, "quartic"),
            Self::Silverman => write!(f, "silverman"),
            Self::Triangular => write!(f, "triangular"),
            Self::Tricube => write!(f, "tricube"),
            Self::Triweight => write!(f, "triweight"),
            Self::Uniform => write!(f, "uniform"),
        }
    }
}

impl<F: KDEFloat> Kernel<F> for Distribution {
    fn pdf(&self, x: F) -> F {
        match self {
            Distribution::Cosine => Cosine.pdf(x),
            Distribution::Epanechnikov => Epanechnikov.pdf(x),
            Distribution::Logistic => Logistic.pdf(x),
            Distribution::Normal => Normal.pdf(x),
            Distribution::Quartic => Quartic.pdf(x),
            Distribution::Silverman => SilvermanKernel.pdf(x),
            Distribution::Triangular => Triangular.pdf(x),
            Distribution::Tricube => Tricube.pdf(x),
            Distribution::Triweight => Triweight.pdf(x),
            Distribution::Uniform => Uniform.pdf(x),
        }
    }
}

pub trait Axes2DExt {
    fn histplot_discrete(
        &mut self,
        x: Vec<f64>,
        stats: &OnlineStats,
        min: Option<f64>,
        max: Option<f64>,
        num_bins: Option<usize>,
        dist: Distribution,
    ) -> &mut Self;

    fn histplot_continuous(
        &mut self,
        x: Vec<f64>,
        stats: &OnlineStats,
        min: Option<f64>,
        max: Option<f64>,
        num_bins: Option<usize>,
        dist: Distribution,
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
        dist: Distribution,
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

        let mut items: Vec<_> = counter.into_iter().filter(|(x, _)| !x.is_nan()).collect();
        items.sort_unstable_by_key(|(x, _count)| *x);
        let (x, counts): (Vec<_>, Vec<_>) = items.into_iter().unzip();
        let x = unsafe { std::mem::transmute::<Vec<OrderedFloat<f64>>, Vec<f64>>(x) };
        let widths = std::iter::repeat_n(bin_width, x.len()).collect();

        let kde = KernelDensityEstimator::new(x.as_slice(), Silverman, dist);
        // TODO: This scaling needs tuning I think. It makes the assumption that the median is
        // close to the most common value, which is not the case. it would maybe be better if it
        // were scaled up to the count at the median, but the median isn't guaranteed to be a key
        // in the Counter.
        let median_pdf = kde.pdf(&[stats.median.unwrap_or(stats.mean)])[0];
        let sample_points: Vec<_> = itertools_num::linspace(min, max, num_bins * 2).collect();
        let pdf_samples = kde
            .pdf(&sample_points)
            .into_iter()
            .map(|s| s * 0.7 * max_count / median_pdf);

        self.set_y_range(AutoOption::Fix(0.0), AutoOption::Fix(max_count + 0.4));
        self.set_x_range(
            AutoOption::Fix(min - 0.1 * stats.stddev()),
            AutoOption::Fix(max + 0.1 * stats.stddev()),
        );
        self.set_y_label("Count", &[]);
        self.boxes(
            x.clone(),
            counts,
            &[
                PlotOption::BorderColor(ColorType::RGBString("black")),
                PlotOption::BoxWidth(widths),
            ],
        )
        .lines(sample_points, pdf_samples, &[PlotOption::LineWidth(2.0)])
    }

    fn histplot_continuous(
        &mut self,
        x: Vec<f64>,
        stats: &OnlineStats,
        min: Option<f64>,
        max: Option<f64>,
        num_bins: Option<usize>,
        dist: Distribution,
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

        let mut max_count: usize = 0;
        for value in x.iter() {
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
        let max_count = max_count as f64;

        let x: Vec<_> = x.into_iter().filter(|x| !x.is_nan()).collect();
        let kde = KernelDensityEstimator::new(x, Silverman, dist);
        let median_pdf = kde.pdf(&[stats.median.unwrap_or(stats.mean)])[0];
        let sample_points: Vec<_> = itertools_num::linspace(min, max, num_bins * 2).collect();
        let pdf_samples = kde
            .pdf(&sample_points)
            .into_iter()
            .map(|s| s * 0.7 * max_count / median_pdf);

        let widths = std::iter::repeat_n(bin_width, bin_centers.len()).collect();

        self.set_y_range(AutoOption::Fix(0.0), AutoOption::Fix(max_count + 0.4));
        self.set_x_range(
            AutoOption::Fix(min - 0.1 * stats.stddev()),
            AutoOption::Fix(max + 0.1 * stats.stddev()),
        );
        self.set_y_label("Count", &[]);
        self.boxes(
            bin_centers,
            counts,
            &[
                PlotOption::BorderColor(ColorType::RGBString("black")),
                PlotOption::BoxWidth(widths),
            ],
        )
        .lines(sample_points, pdf_samples, &[PlotOption::LineWidth(2.0)])
    }
}
