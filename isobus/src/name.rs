//! The ISOBUS NAME is defined by ISO 11783-5 4.3.2.
//!
//! Some of the NAME fields have global values, but the interpretation of other fields depends on
//! the values of yet other fields. See 'Figure 2 -- NAME-field relationships and dependencies'
//! from ISO 11783-5 4.3.2.
//!
//! * [SelfConfigurable]
//! * Function (lower 128 functions)
//!   * Function Instance
//!     * ECU Instance
//! * [IndustryGroup]
//!   * DeviceClass
//!     * DeviceClass Instance
//!     * Function (upper 128 functions)
//!       * Function Instance
//!         * ECU Instance
//! * [Manufacturer]
//!   * Identity number

/// Self-configurable address
///
/// Indicates whether a control function can self-configure its address.
///
/// Bit 63 of the NAME.
///
/// SPN 2844 (0x0B1C).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfConfigurable {
    /// Not capable of claiming an arbitrary address (i.e., requires a static address)
    NotConfigurable = 0,
    /// Capable of claiming an arbitrary address
    Configurable = 1,
}

/// Industry group
///
/// Bits 60..=62 of the NAME.
///
/// SPN 2846 (0x0B1E).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndustryGroup {
    Global = 0,
    OnHighway = 1,
    AgriculturalAndForestry = 2,
    Construction = 3,
    Marine = 4,
    Industrial = 5,
    Reserved6 = 6,
    Reserved7 = 7,
}

/// Device class instance
///
/// Interpretation depends on the [IndustryGroup] and [DeviceClass].
///
/// Bits 56..=59 of the NAME.
///
/// SPN 2843 (0x0B1B).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceClassInstance(pub u8);

/// Device class
///
/// Defined and assigned by ISO. Interpretation depends on the [IndustryGroup]. Also known as
/// "Vehicle System" in SAE J1939.
///
/// Bits 49..=55 of the NAME.
///
/// SPN 2842 (0x0B1A).
// TODO: Generate enum? from 'IG Specific NAME Functions.csv'
// TODO: I don't know what the best API is for heirarchical enums where the interpretation of one
// enum depends on another enum.
pub struct DeviceClass(pub u8);

/// Reserved bit (always 0)
///
/// Bit 48 of the NAME.
pub struct Reserved(pub bool);

include!(concat!(env!("OUT_DIR"), "/manufacturer.rs"));
