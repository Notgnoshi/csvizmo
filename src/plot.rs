use gnuplot::{Axes2D, PlotOption};

use crate::counter::Counter;

pub trait Axes2DExt {
    fn histplot_discrete(
        &mut self,
        // It sucks to specialize this, but I can't impl DataType for OrderedFloat. This is a
        // pragmatic workaround, because I'm the only user of this, and my only use-case is
        // OrderedFloat<f64>.
        counter: &Counter<ordered_float::OrderedFloat<f64>>,
        bin_width: f64,
        options: &[PlotOption<&str>],
    ) -> &mut Self;
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
        let widths = std::iter::repeat(bin_width);
        tracing::warn!("values: {values:?}");
        tracing::warn!("counts: {counts:?}");
        self.boxes_set_width(values, counts, widths, options)
    }
}
