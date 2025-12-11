use std::collections::HashMap;

/// A CSV record from 'Manufactuerer IDs.csv'
#[derive(Debug, Clone, serde::Deserialize)]
struct ManufacturerCsvRecord {
    value: u32,
    manufacturer: String,
    #[allow(unused)]
    location: String,
    #[allow(unused)]
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

fn get_normalized_manufacturer_name(value: u32, name: &str) -> String {
    // Use the integer value to enable special casing specific manufacturers that are hard to
    // normalize generically.
    if value == 0 {
        return "Reserved".into();
    }

    // Remove any (formerly ...) suffixes
    let name = name.split('(').next().unwrap_or(name).trim();

    // Replace non-alphanumeric characters with spaces
    let mut name: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();

    // Identifiers cannot start with a digit, so prefix with 'N' if it does
    if name.chars().next().unwrap().is_numeric() {
        name.insert(0, 'N');
    }

    // Convert to PascalCase
    convert_case::ccase!(pascal, name)
}

fn generate_manufactuerer_enum_impl(manufacturers: &[ManufacturerCsvRecord]) -> codegen::Enum {
    let mut enum_def = codegen::Enum::new("Manufacturer");
    enum_def
        .vis("pub")
        .r#macro("#[non_exhaustive]")
        .derive("Debug")
        .derive("Clone")
        .derive("Copy")
        .derive("PartialEq")
        .derive("Eq");

    // TODO: Derive TryFromRepr
    // TODO: .doc

    let mut counts = HashMap::<String, usize>::new();
    for record in manufacturers {
        let name = get_normalized_manufacturer_name(record.value, &record.manufacturer);
        let val = counts.entry(name).or_insert(0);
        *val += 1;
    }

    // TODO: Add a 'Custom(u32)' variant for unknown manufacturers?
    // TODO: 'impl Display for Manufacturer' using the original manufacturer names from the CSV
    for record in manufacturers {
        let mut name = get_normalized_manufacturer_name(record.value, &record.manufacturer);
        if counts[&name] > 1 {
            name = format!("{}{}", name, record.value);
        }
        let mut variant = codegen::Variant::new(name);
        variant.discriminant(record.value);
        enum_def.push_variant(variant);
    }
    enum_def
}

/// Generate the `Manufacturer` enum from the 'Manufacturer IDs.csv' file
pub fn generate_manufacturer_enum() -> eyre::Result<String> {
    let reader = crate::get_csv_reader("Manufacturer IDs.csv")?;
    let manufacturers = parse_manufacturers_from_csv(reader)?;
    let mut scope = codegen::Scope::new();
    scope.push_enum(generate_manufactuerer_enum_impl(&manufacturers));
    Ok(scope.to_string())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

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

    #[test]
    fn test_normalized_name() {
        let normalized = get_normalized_manufacturer_name(
            0,
            "Reserved for experimental use, not for production use.",
        );
        assert_eq!(normalized, "Reserved");
        let normalized = get_normalized_manufacturer_name(
            1,
            "Bendix Commercial Vehicle Systems LLC (formerly Allied Signal Inc.)",
        );
        assert_eq!(normalized, "BendixCommercialVehicleSystemsLlc");
        let normalized = get_normalized_manufacturer_name(2, "Allison Transmission, Inc.");
        assert_eq!(normalized, "AllisonTransmissionInc");
        let normalized = get_normalized_manufacturer_name(3, "Ametek, US Gauge Division");
        assert_eq!(normalized, "AmetekUsGaugeDivision");

        let normalized = get_normalized_manufacturer_name(12, "Deere & Company, Precision Farming");
        assert_eq!(normalized, "DeereCompanyPrecisionFarming");

        let normalized = get_normalized_manufacturer_name(15, "DICKEY-john Corporation");
        assert_eq!(normalized, "DickeyJohnCorporation");

        let normalized = get_normalized_manufacturer_name(70, "Flex-coil Limited");
        assert_eq!(normalized, "FlexCoilLimited");

        let normalized = get_normalized_manufacturer_name(1569, "621 Technologies Inc.");
        assert_eq!(normalized, "N621TechnologiesInc");

        // non-ascii whitespace
        let normalized = get_normalized_manufacturer_name(879, "PG Trionic, Inc.");
        assert_eq!(normalized, "PgTrionicInc");
    }

    #[test]
    fn test_generate_enum() {
        let reader = crate::get_csv_reader("Manufacturer IDs.csv").unwrap();
        let records = parse_manufacturers_from_csv(reader).unwrap();

        let mut scope = codegen::Scope::new();
        scope.push_enum(generate_manufactuerer_enum_impl(&records[..4]));
        let expected = "\
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Manufacturer {
    Reserved = 0,
    BendixCommercialVehicleSystemsLlc = 1,
    AllisonTransmissionInc = 2,
    AmetekUsGaugeDivision = 3,
}";
        assert_eq!(expected, scope.to_string());
    }
}
