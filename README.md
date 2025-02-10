# csvizmo

![lint workflow](https://github.com/Notgnoshi/csvizmo/actions/workflows/lint.yml/badge.svg?event=push)
![code coverage](https://img.shields.io/endpoint?url=https://gist.githubusercontent.com/Notgnoshi/5c7197add87b1263923e0cbfb85477a8/raw/csvizmo-coverage.json)

Gizmos for working with CSVs

# Philosophy

Rather than to build an infinitely flexible, highly optimized, does-everything-and-more toolkit (see
<https://github.com/dathere/qsv> for that) these gizmos are targeted tools to solve specific
problems I frequently encounter.

All tools operate on stdin/stdout in addition to files, and are designed to be chained together with
pipes. Any ancillary output is emitted on stderr.

# Gizmos

* [x] `can2csv` - format a `can-utils` candump into a CSV file
* [ ] `csvbits` - parse bitfields out of a CSV column
* [ ] `csvjitter` - add some random noise to a CSV column
* [ ] `csvcomm` - find where two CSV files overlap
* [ ] `csvdelta` - inter-row deltas
* [ ] `csvstats` - calculate 5-number summary statistics
* [ ] `csvoutlier` - outlier detection and filtering
* [ ] `csvplot` - line, scatter, and time series plots
  * [ ] plot histogram, with pde kernel estimation
