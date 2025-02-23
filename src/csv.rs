use std::io::Read;

use eyre::WrapErr;

/// Find the column index of the given column in the CSV header (if it's present)
pub fn column_index<S: AsRef<str>, R: Read>(
    reader: &mut csv::Reader<R>,
    name_or_index: S,
) -> eyre::Result<usize> {
    let has_header = reader.has_headers();
    let header = reader.headers()?;

    // Find the column index of the input column
    let column_index: usize = if has_header {
        // First check for a column named by --column, and then fallback to parsing --column as an
        // index
        if let Some(index) = header.iter().position(|h| h == name_or_index.as_ref()) {
            index
        } else {
            let index: usize = name_or_index
                .as_ref()
                .parse()
                .wrap_err("Failed to parse column as an index")
                .wrap_err("Failed to find column in CSV header")?;
            index
        }
    } else {
        // If there's no header, then --column *must* be an index
        name_or_index.as_ref().parse()?
    };
    // If the CSV doesn't have a header, then 'header' is the first row, which is still enough to
    // bounds check the index.
    if header.get(column_index).is_none() {
        eyre::bail!(
            "Given column {:?} not found in CSV header: {header:?}",
            name_or_index.as_ref()
        );
    }
    tracing::debug!(
        "Found column {:?} at index {column_index}",
        name_or_index.as_ref()
    );

    Ok(column_index)
}

pub fn parse_field(record: &csv::StringRecord, index: usize) -> eyre::Result<f64> {
    let field = record
        .get(index)
        .ok_or(eyre::eyre!("Record {record:?} missing field {index}"))?;
    let value: f64 = field
        .parse()
        .wrap_err(format!("Failed to parse field {field:?} as f64"))?;
    Ok(value)
}

/// Stop parsing CSV records after the first read failure
///
/// Note that this does *not* mean reading will stop after the first *parse* error. Example errors:
/// * an I/O error
/// * invalid UTF-8 data
/// * CSV record has ragged length
pub fn exit_after_first_failed_read<R, E>(records: R) -> impl Iterator<Item = csv::StringRecord>
where
    R: Iterator<Item = Result<csv::StringRecord, E>>,
    E: std::error::Error,
{
    records.map_while(|record| match record {
        Ok(r) => Some(r),
        Err(e) => {
            tracing::error!("Failed to read CSV record: {e}");
            None
        }
    })
}

/// Parse a single column value, along with the whole record out of the given CSV records
pub fn parse_column_records<R>(
    records: R,
    index: usize,
) -> impl Iterator<Item = (csv::StringRecord, eyre::Result<f64>)>
where
    R: Iterator<Item = csv::StringRecord>,
{
    records.map(move |record| {
        let result = parse_field(&record, index);
        (record, result)
    })
}

/// Apply the given function to the specified field in each record
///
/// The transformed field will be appended to the end of the record (it will not modify the input
/// values).
pub fn map_column_records<R, F>(records: R, mut func: F) -> impl Iterator<Item = csv::StringRecord>
where
    R: Iterator<Item = (csv::StringRecord, eyre::Result<f64>)>,
    F: FnMut(eyre::Result<f64>) -> Option<f64>,
{
    records.map(move |(mut record, maybe_value)| {
        match func(maybe_value) {
            Some(value) => record.push_field(&format!("{value}")),
            None => record.push_field(""),
        }
        record
    })
}

/// Parse a single column value out of the given CSV records
pub fn parse_column_values<R>(records: R, index: usize) -> impl Iterator<Item = eyre::Result<f64>>
where
    R: Iterator<Item = csv::StringRecord>,
{
    records.map(move |record| parse_field(&record, index))
}

/// Compute the summary statistics of the given values
pub fn column_stats<'v, V>(values: V) -> rolling_stats::Stats<f64>
where
    // TODO: Augment the rolling Stats with missing data counter?
    V: Iterator<Item = &'v f64>,
{
    let mut stats = rolling_stats::Stats::new();
    for value in values {
        stats.update(*value);
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_column_index_no_header() {
        let content = b"\
            1,a\n\
            2,b\n\
        ";

        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(&content[..]);

        let idx = column_index(&mut reader, "0").unwrap();
        assert_eq!(idx, 0);

        let result = column_index(&mut reader, "2");
        assert!(result.is_err());

        let result = column_index(&mut reader, "foo");
        assert!(result.is_err());

        // Determining the column index does not consume the first row.
        let first_row = reader.records().next().unwrap().unwrap();
        let mut expected = csv::StringRecord::new();
        expected.push_field("1");
        expected.push_field("a");

        assert_eq!(first_row, expected);
    }

    #[test]
    fn test_column_index_header() {
        let content = b"\
            foo,bar\n\
            1,a\n\
            2,b\n\
        ";

        let mut reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_reader(&content[..]);

        let idx = column_index(&mut reader, "0").unwrap();
        assert_eq!(idx, 0);
        let idx = column_index(&mut reader, "foo").unwrap();
        assert_eq!(idx, 0);
        let idx = column_index(&mut reader, "bar").unwrap();
        assert_eq!(idx, 1);
        let idx = column_index(&mut reader, "1").unwrap();
        assert_eq!(idx, 1);

        let result = column_index(&mut reader, "2");
        assert!(result.is_err());

        let result = column_index(&mut reader, "baz");
        assert!(result.is_err());

        // Determining the column index does not consume the first row.
        let first_row = reader.records().next().unwrap().unwrap();
        let mut expected = csv::StringRecord::new();
        expected.push_field("1");
        expected.push_field("a");

        assert_eq!(first_row, expected);
    }

    #[test]
    fn test_parse_column() {
        let content = b"\
            foo,bar\n\
            1,a\n\
            2,b\n\
            3,c\n\
        ";
        let reader = csv::Reader::from_reader(&content[..]);
        let records = exit_after_first_failed_read(reader.into_records());
        let values: Vec<_> = parse_column_values(records, 0).flatten().collect();
        let expected = [1.0, 2.0, 3.0];
        assert_eq!(values, expected);
    }

    #[test]
    fn test_parse_column_missing_data() {
        let content = b"\
            foo,bar\n\
            1,a\n\
             ,b\n\
            3,c\n\
        ";
        let reader = csv::Reader::from_reader(&content[..]);
        let records = exit_after_first_failed_read(reader.into_records());
        let maybe_values: Vec<_> = parse_column_values(records, 0).collect();
        assert_eq!(maybe_values.len(), 3);
        assert!(maybe_values[1].is_err());

        let values: Vec<_> = maybe_values.into_iter().filter_map(|r| r.ok()).collect();
        let expected = [1.0, 3.0];
        assert_eq!(values, expected);
    }
}
