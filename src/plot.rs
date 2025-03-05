use gnuplot::{AutoOption, Axes2D, AxesCommon, DataType, PlotOption};

use crate::counter::Counter;

pub trait Axes2DExt {
    fn histplot_discrete(
        &mut self,
        // It sucks to specialize the generic type, but I can't impl DataType for OrderedFloat due
        // to the orphan rule, so it can't be Counter<X: DataType>. This is a pragmatic workaround,
        // because I'm the only user of this, and my only use-case is OrderedFloat<f64>.
        counter: &Counter<ordered_float::OrderedFloat<f64>>,
        bin_width: f64,
        options: &[PlotOption<&str>],
    ) -> &mut Self;

    fn histplot_continuous<X, Tx>(
        &mut self,
        x: X,
        min: f64,
        num_bins: usize,
        bin_width: f64,
        options: &[PlotOption<&str>],
    ) -> &mut Self
    where
        X: IntoIterator<Item = Tx>,
        Tx: DataType;
}

impl Axes2DExt for Axes2D {
    fn histplot_discrete(
        &mut self,
        counter: &Counter<ordered_float::OrderedFloat<f64>>,
        bin_width: f64,
        options: &[PlotOption<&str>],
    ) -> &mut Self {
        let values: Vec<_> = counter.keys().collect();
        let values = unsafe {
            std::mem::transmute::<Vec<&ordered_float::OrderedFloat<f64>>, Vec<&f64>>(values)
        };
        let counts: Vec<_> = counter.values().collect();
        let (_x_val, max_count) = counter
            .single_most_common()
            .unwrap_or((&ordered_float::OrderedFloat::<f64>::from(0.0), &0));
        let widths = std::iter::repeat(bin_width);

        self.set_y_range(
            AutoOption::Fix(0.0),
            AutoOption::Fix(*max_count as f64 + 0.5),
        );
        self.set_y_label("Count", &[]);
        self.boxes_set_width(values, counts, widths, options)
    }

    fn histplot_continuous<X, Tx>(
        &mut self,
        xs: X,
        // TODO: Find a way to not need to pass the min in.
        //
        // 1. Break with gnuplot's iterator API and pass in an &mut [f64], so this method can
        //    generate the OnlineStats
        // 2. Pass the OnlineStats in, as well as Option<f64> overrides for num_bins, min, and max
        // 3. Leave it as-is
        min: f64,
        num_bins: usize,
        bin_width: f64,
        options: &[PlotOption<&str>],
    ) -> &mut Self
    where
        X: IntoIterator<Item = Tx>,
        Tx: DataType,
    {
        let mut counts = vec![0; num_bins];

        let mut bin_centers = Vec::with_capacity(num_bins);
        let first_bin_center = min + 0.5 * bin_width;
        for i in 0..num_bins {
            let center = first_bin_center + i as f64 * bin_width;
            bin_centers.push(center);
        }
        let mut max_count = 0;
        for x in xs {
            let x = x.get();

            // This calculation uses an inclusive LHS and exclusive RHS. That's what we want
            // everywhere except the far RHS endpoint.
            let bin_index = (x - min) / bin_width;
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
            AutoOption::Fix(max_count as f64 + 0.5),
        );
        self.set_y_label("Count", &[]);
        self.boxes_set_width(bin_centers, counts, widths, options)
    }
}
