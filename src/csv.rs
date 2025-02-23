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
}
