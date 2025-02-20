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

# How to use

You can install the gizmos with

```sh
# Will install to ~/.cargo/bin/
cargo install --path .
```

You can experiment with them by

```sh
cargo run --bin can2csv -- ...
```

# Gizmos

* [x] `can2csv` - format a `can-utils` candump into a CSV file
* [x] `canspam` - generate lots of random CAN messages for testing
* [ ] `can2k` - a NMEA2000 candump parser
* [ ] `csvbits` - parse bitfields out of a CSV column
* [ ] `csvjitter` - add some random noise to a CSV column
* [ ] `csvcomm` - find where two CSV files overlap
* [x] `csvdelta` - inter-row deltas and value centering
* [ ] `csvstats` - calculate 5-number summary statistics
* [ ] `csvoutlier` - outlier detection and filtering
* [ ] `csvplot` - line, scatter, and time series plots
  * [ ] plot histogram, with pde kernel estimation

## can2csv

Faster than `sed`, and also parses the canid. Useful in conjunction with `csvdelta` to understand
message timing.

```sh
$ head -n 3 data/candump-random-data.log | can2csv
timestamp,interface,canid,dlc,priority,src,dst,pgn,data
1739229594.465994,can0,0xE9790B5,8,3,0xB5,0x90,0x29700,CA3F871A5A6EE75F
1739229594.467052,can0,0xD15F192,8,3,0x92,0xF1,0x11500,500B3766CB2DED7C
```

## csvdelta

Useful for understanding the time between events. Also supports mean-centering a column, or
centering it around a specific value.

```sh
$ csvdelta --column foo <<EOF
foo,bar
0,a
1,b
3,c
5,d
EOF

foo,bar,foo-deltas
0,a,
1,b,1
3,c,2
5,d,2
```
