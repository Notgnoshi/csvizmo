/// A CSV record from 'Manufactuerer IDs.csv'
#[derive(Debug, Clone, serde::Deserialize)]
struct ManufacturerCsvRecord {
    value: u32,
    manufacturer: String,
    location: String,
    date_created_or_last_modified: String,
}

fn parse_manufacturers_from_csv<R: std::io::Read>(
    reader: csv::Reader<R>,
) -> eyre::Result<Vec<ManufacturerCsvRecord>> {
    let records = reader.into_deserialize::<ManufacturerCsvRecord>();
    let mut manufacturers = Vec::new();
    for record in records {
        let record = record?;
        manufacturers.push(record);
    }
    Ok(manufacturers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_manufacturers_from_csv() {
        let csv_data = "\
            value,manufacturer,location,date_created_or_last_modified\n\
            0,\"Reserved for experimental use, not for production use.\",N.A.,\n\
            1,\"Bendix Commercial Vehicle Systems LLC (formerly Allied Signal Inc.)\",\"Elyria, OH   USA\",2013-06-28\n\
            2,\"Allison Transmission, Inc.\",\"Indianapolis, IN   USA\",2015-04-28\n\
        ";
        let reader = csv::Reader::from_reader(csv_data.as_bytes());
        let records = parse_manufacturers_from_csv(reader).unwrap();
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].value, 0);
        assert_eq!(
            records[0].manufacturer,
            "Reserved for experimental use, not for production use."
        );
        assert_eq!(records[0].location, "N.A.");
        assert_eq!(records[0].date_created_or_last_modified, "");
    }

    #[test]
    fn test_parse_manufacturers_from_iso_export_csv() {
        let reader = crate::get_csv_reader("Manufacturer IDs.csv").unwrap();
        let records = parse_manufacturers_from_csv(reader).unwrap();
        assert!(records.len() >= 1591);
    }
}
