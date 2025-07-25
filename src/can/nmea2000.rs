use std::collections::HashMap;

use byteorder::{ByteOrder, LittleEndian};
use serde::ser::SerializeStruct;

use crate::can::CanMessage;

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct GpsData {
    /// CAN src address of this GPS stream, to support multiple streams on the same bus
    pub src: u8,
    pub seq_id: u8,
    pub longitude_deg: f64,
    pub latitude_deg: f64,
    pub altitude_m: f64,
    pub sog_mps: f64,
    pub cog_deg_cwfn: f64,
    pub cog_ref: u8, // 0=true, 1=magnetic, 2=error, 3=null
    pub method: u8,
    pub msg_timestamp: f64,
    pub gps_timestamp: (),
    pub gps_age: f64,
    pub msg: &'static str,
}

#[repr(transparent)]
#[derive(Clone, Debug, PartialEq)]
pub struct GpsDataWkt(pub GpsData);

// NOTE: I tried to do
//
//     #[derive(serde::Serialize)]
//     pub struct GpsDataWkt {
//         pub geometry: String,
//         #[serde(flatten)]
//         pub data: GpsData,
//     }
//
// But the csv Writer complained that it couldn't serialize a map. So manually implement Serialize
// and move on.
impl serde::Serialize for GpsDataWkt {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("GpsData", 14)?; // 13 + geometry
        state.serialize_field("src", &self.0.src)?;
        state.serialize_field("seq_id", &self.0.seq_id)?;
        state.serialize_field(
            "geometry",
            &format!(
                "POINT Z ({} {} {})",
                self.0.longitude_deg, self.0.latitude_deg, self.0.altitude_m
            ),
        )?;
        state.serialize_field("longitude_deg", &self.0.longitude_deg)?;
        state.serialize_field("latitude_deg", &self.0.latitude_deg)?;
        state.serialize_field("altitude_m", &self.0.altitude_m)?;
        state.serialize_field("sog_mps", &self.0.sog_mps)?;
        state.serialize_field("cog_deg_cwfn", &self.0.cog_deg_cwfn)?;
        state.serialize_field("cog_ref", &self.0.cog_ref)?;
        state.serialize_field("method", &self.0.method)?;
        state.serialize_field("msg_timestamp", &self.0.msg_timestamp)?;
        state.serialize_field("gps_timestamp", &self.0.gps_timestamp)?;
        state.serialize_field("gps_age", &self.0.gps_age)?;
        state.serialize_field("msg", &self.0.msg)?;
        state.end()
    }
}

const NMEA_2000_PGNS: [u32; 5] = [0x1F801, 0x1F802, 0x1F803, 0x1F804, 0x1F805];

enum N2kMsg {
    #[expect(unused)]
    PositionRapidUpdate(PositionRapidUpdate), // 0x1F801,
    CogSogRapidUpdate(CogSogRapidUpdate), // 0x1F802,
    PositionDeltaHighPrecisionRapidUpdate(PositionDeltaHighPrecisionRapidUpdate), // 0x1F803
    AltitudeDeltaHighPrecisionRapidUpdate(AltitudeDeltaHighPrecisionRapidUpdate), // 0x1F804
    GnssPositionData(GnssPositionData),   // 0x1F805
}

#[derive(Clone, Debug, Default, PartialEq)]
struct PositionRapidUpdate {
    msg_timestamp: f64,
    longitude_deg: f64,
    latitude_deg: f64,
}

fn parse_position_rapid_update(msg: CanMessage) -> Option<PositionRapidUpdate> {
    if msg.data.len() != 8 {
        return None;
    }

    let raw = LittleEndian::read_i32(&msg.data[..4]);
    let latitude_deg = raw as f64 * 1e-7;
    let raw = LittleEndian::read_i32(&msg.data[4..]);
    let longitude_deg = raw as f64 * 1e-7;

    Some(PositionRapidUpdate {
        msg_timestamp: msg.timestamp,
        longitude_deg,
        latitude_deg,
    })
}

#[derive(Clone, Debug, Default, PartialEq)]
struct CogSogRapidUpdate {
    msg_timestamp: f64,
    seq_id: u8,
    cog_ref: u8,
    cog: f64,
    sog: f64,
}

fn parse_cog_sog_rapid_update(msg: CanMessage) -> Option<CogSogRapidUpdate> {
    if msg.data.len() != 8 {
        return None;
    }

    let seq_id = msg.data[0];
    let cog_ref = msg.data[1] & 0b0011;

    // let cog_raw = u16::from_le_bytes(msg.data[2..4].try_into().unwrap());
    let cog_raw = LittleEndian::read_u16(&msg.data[2..4]);
    let cog = (cog_raw as f64 * 1e-4).to_degrees();

    // let sog_raw = u16::from_le_bytes(msg.data[4..6].try_into().unwrap()) ;
    let sog_raw = LittleEndian::read_u16(&msg.data[4..6]);
    let sog = sog_raw as f64 * 1e-2;

    Some(CogSogRapidUpdate {
        msg_timestamp: msg.timestamp,
        seq_id,
        cog_ref,
        cog,
        sog,
    })
}

#[derive(Clone, Debug, Default, PartialEq)]
struct PositionDeltaHighPrecisionRapidUpdate {
    msg_timestamp: f64,
    seq_id: u8,
    time_delta: f64,
    longitude_delta: f64,
    latitude_delta: f64,
}

fn parse_position_delta_rapid_update(
    msg: CanMessage,
) -> Option<PositionDeltaHighPrecisionRapidUpdate> {
    if msg.data.len() != 8 {
        return None;
    }
    let seq_id = msg.data[0];
    let time_delta_raw = msg.data[1];
    let time_delta = time_delta_raw as f64 * 5e-3;

    let scalar = 1e-5 / (60.0 * 60.0); // Scale and convert seconds to degrees
    let raw = LittleEndian::read_i24(&msg.data[2..5]);
    let latitude_delta = raw as f64 * scalar;

    let raw = LittleEndian::read_i24(&msg.data[5..8]);
    let longitude_delta = raw as f64 * scalar;

    Some(PositionDeltaHighPrecisionRapidUpdate {
        msg_timestamp: msg.timestamp,
        seq_id,
        time_delta,
        latitude_delta,
        longitude_delta,
    })
}

#[derive(Clone, Debug, Default, PartialEq)]
struct AltitudeDeltaHighPrecisionRapidUpdate {
    msg_timestamp: f64,
    seq_id: u8,
    time_delta: f64,
    method: u8,
    cog_ref: u8,
    cog: f64,
    altitude_delta: f64,
}

fn parse_altitude_delta_rapid_update(
    msg: CanMessage,
) -> Option<AltitudeDeltaHighPrecisionRapidUpdate> {
    if msg.data.len() != 8 {
        return None;
    }

    let seq_id = msg.data[0];
    let raw = msg.data[1];
    let time_delta = raw as f64 * 5e-3;
    let method = msg.data[2] & 0x0F;
    let cog_ref = (msg.data[2] & 0b00110000) >> 4;

    let raw = LittleEndian::read_u16(&msg.data[3..5]);
    let cog = (raw as f64 * 1e-4).to_degrees();

    let raw = LittleEndian::read_i24(&msg.data[5..8]);
    let altitude_delta = raw as f64 * 1e-3;

    Some(AltitudeDeltaHighPrecisionRapidUpdate {
        msg_timestamp: msg.timestamp,
        seq_id,
        time_delta,
        method,
        cog_ref,
        cog,
        altitude_delta,
    })
}

#[derive(Clone, Debug, Default, PartialEq)]
struct GnssPositionData {
    msg_timestamp: f64,
    seq_id: u8,
    date: u16,
    time: f64,
    latitude: f64,
    longitude: f64,
    altitude: f64,
    system_type: u8,
    method: u8,
    integrity: u8,
    num_svs: u8,
    hdop: f64,
    pdop: f64,
    geoidal_separation: f64,
    num_reference_stations: u8,
    reference_stations: Vec<(u8, u16, f64)>, // type, id, age
}

fn parse_gnss_position_data(msg: CanMessage) -> Option<GnssPositionData> {
    // 33 doesn't guarantee a full message. I'm aware of at least on N2K source that doesn't send
    // the full message, and truncates it before the num_svs.
    if msg.data.len() < 33 {
        return None;
    }

    let mut out = GnssPositionData {
        msg_timestamp: msg.timestamp,
        seq_id: msg.data[0],
        ..Default::default()
    };
    out.date = LittleEndian::read_u16(&msg.data[1..3]);
    let raw = LittleEndian::read_u32(&msg.data[3..7]);
    out.time = raw as f64 * 1e-4;

    let raw = LittleEndian::read_i64(&msg.data[7..15]);
    out.latitude = raw as f64 * 1e-16;

    let raw = LittleEndian::read_i64(&msg.data[15..23]);
    out.longitude = raw as f64 * 1e-16;

    let raw = LittleEndian::read_i64(&msg.data[23..31]);
    out.altitude = raw as f64 * 1e-6;

    out.system_type = msg.data[31] & 0x0F;
    out.method = (msg.data[31] & 0xF0) >> 4;
    out.integrity = msg.data[32] & 0b0011;

    if msg.data.len() >= 34 {
        out.num_svs = msg.data[33];
    }
    if msg.data.len() >= 36 {
        let raw = LittleEndian::read_i16(&msg.data[34..36]);
        out.hdop = raw as f64 * 1e-2;
    }
    if msg.data.len() >= 38 {
        let raw = LittleEndian::read_i16(&msg.data[36..38]);
        out.pdop = raw as f64 * 1e-2;
    }
    if msg.data.len() >= 42 {
        let raw = LittleEndian::read_i32(&msg.data[38..42]);
        out.geoidal_separation = raw as f64 * 1e-2;
    }
    if msg.data.len() >= 43 {
        out.num_reference_stations = msg.data[42];
    }
    let ref_station_size = 4;
    if msg.data.len() < 43 + out.num_reference_stations as usize * ref_station_size {
        return Some(out);
    }

    let mut cursor = 43;
    for _ in 0..out.num_reference_stations {
        let station_type = msg.data[cursor] & 0x0F;
        let mut station_id = (msg.data[cursor] as u16 & 0xF0) << 8;
        station_id |= msg.data[cursor + 1] as u16;

        let raw = LittleEndian::read_u16(&msg.data[cursor + 2..cursor + 4]);
        let age = raw as f64 * 1e-2;
        out.reference_stations.push((station_type, station_id, age));
        cursor += ref_station_size;
    }

    Some(out)
}

fn parse_n2k_msg(msg: CanMessage) -> Option<N2kMsg> {
    let msg = match msg.pgn {
        0x1F801 => N2kMsg::PositionRapidUpdate(parse_position_rapid_update(msg)?),
        0x1F802 => N2kMsg::CogSogRapidUpdate(parse_cog_sog_rapid_update(msg)?),
        0x1F803 => {
            N2kMsg::PositionDeltaHighPrecisionRapidUpdate(parse_position_delta_rapid_update(msg)?)
        }
        0x1F804 => {
            N2kMsg::AltitudeDeltaHighPrecisionRapidUpdate(parse_altitude_delta_rapid_update(msg)?)
        }
        0x1F805 => N2kMsg::GnssPositionData(parse_gnss_position_data(msg)?),
        _ => unreachable!("This match covers all of NMEA_2000_PGNS"),
    };
    Some(msg)
}

struct GpsStreamParser {
    current_data: GpsData,
    position_data: HashMap<u8, GnssPositionData>,
}

impl GpsStreamParser {
    fn new(src: u8) -> Self {
        Self {
            current_data: GpsData {
                src,
                ..Default::default()
            },
            position_data: HashMap::new(),
        }
    }
    fn handle_update(&mut self, msg: N2kMsg) -> Option<GpsData> {
        match msg {
            // Ignore 1F801 position rapid updates, as the GNSS Position Data + Position Delta is
            // sufficient.
            N2kMsg::PositionRapidUpdate(_) => {}
            N2kMsg::CogSogRapidUpdate(update) => {
                // only update the GpsData timestamp with the GNSS Position Data and Position Delta
                // messages
                self.current_data.sog_mps = update.sog;
                self.current_data.cog_deg_cwfn = update.cog;
            }
            N2kMsg::PositionDeltaHighPrecisionRapidUpdate(delta) => {
                // These warnings might get spammy at times, but they can be useful to troubleshoot
                // whether an N2K stream is misbehaving.
                if !self.position_data.contains_key(&delta.seq_id) {
                    tracing::warn!(
                        "Failed to find a GNSS Position Data with seq_id: {:#X}",
                        delta.seq_id
                    );
                }

                let absolute = self.position_data.get(&delta.seq_id)?;
                let age = delta.msg_timestamp - absolute.msg_timestamp;
                // The parser should still function if a GNSS Position Data message is dropped
                // (meaning the one matching this delta's seq_id is quite old), so we need to
                // detect it, and continue working.
                //
                // This value comes from the NMEA 2000 standard. It's the maximum time delta that
                // the Position Delta and Altitude Delta can hold.
                let err_threshold = 1.26;
                if age > err_threshold {
                    tracing::error!(
                        "Position Delta seq_id: {:#X} at {} refers to (possibly dropped) GNSS Position Data at {} that's {age}s old",
                        delta.seq_id,
                        delta.msg_timestamp,
                        absolute.msg_timestamp,
                    );
                    return None;
                }

                if delta.seq_id != self.current_data.seq_id {
                    tracing::warn!(
                        "Position Delta seq_id: {:#X} at {} refers to old GNSS Position Data at {}; most recent seq_id: {:#X}",
                        delta.seq_id,
                        delta.msg_timestamp,
                        absolute.msg_timestamp,
                        self.current_data.seq_id,
                    );
                }

                self.current_data.msg_timestamp = delta.msg_timestamp;
                // TODO: time_delta
                self.current_data.longitude_deg = absolute.longitude + delta.longitude_delta;
                self.current_data.latitude_deg = absolute.latitude + delta.latitude_delta;
                self.current_data.msg = "Position Delta";
                return Some(self.current_data.clone());
            }
            N2kMsg::AltitudeDeltaHighPrecisionRapidUpdate(delta) => {
                let absolute = self.position_data.get(&delta.seq_id)?;
                // TODO: time_delta update
                self.current_data.altitude_m = absolute.altitude + delta.altitude_delta;
                self.current_data.cog_deg_cwfn = delta.cog;
                self.current_data.cog_ref = delta.cog_ref;
                self.current_data.method = delta.method;
            }
            N2kMsg::GnssPositionData(absolute) => {
                if self.position_data.is_empty() {
                    tracing::info!(
                        "Received first GNSS Position Data at {} with seq_id: {:#X}",
                        absolute.msg_timestamp,
                        absolute.seq_id
                    );
                }

                let mut expected_id = self.current_data.seq_id + 1;
                // It's technically supposed to roll over at 252, but the system I primarily work
                // with rolls over at 250 => 1
                if expected_id > 250 {
                    expected_id = 1;
                }
                // NOTE: The seq_id is actually supposed to increment with every non-delta message.
                // It's purpose is not to detect dropped Position Data messages, it's purpose is to
                // identify new measurements.
                //
                // But the system I primarily work with increments only with the Position Data
                // messages, and it's darn useful to identify dropped messages.
                if absolute.seq_id != expected_id {
                    tracing::warn!(
                        "Received GNSS Position Data msg at {} with seq_id: {:#X} but expected {expected_id:#X}",
                        absolute.msg_timestamp,
                        absolute.seq_id
                    );
                }

                self.current_data.seq_id = absolute.seq_id;
                self.current_data.longitude_deg = absolute.longitude;
                self.current_data.latitude_deg = absolute.latitude;
                self.current_data.altitude_m = absolute.altitude;
                self.current_data.method = absolute.method;
                self.current_data.msg_timestamp = absolute.msg_timestamp;
                // TODO: gps_timestamp and gps_age
                self.current_data.msg = "GNSS Position Data";

                self.position_data.insert(absolute.seq_id, absolute);
                return Some(self.current_data.clone());
            }
        }
        None
    }
}

pub fn parse_n2k_gps<I: Iterator<Item = CanMessage>>(msgs: I) -> N2kParser<I> {
    N2kParser {
        msgs,
        streams: HashMap::new(),
    }
}

pub struct N2kParser<I> {
    msgs: I,
    /// One GPS stream per source address to allow multiple streams on the same bus
    streams: HashMap<u8, GpsStreamParser>,
}

impl<I> N2kParser<I> {
    fn reconstruct_gps(&mut self, src: u8, msg: N2kMsg) -> Option<GpsData> {
        let stream_parser = self
            .streams
            .entry(src)
            .or_insert_with(|| GpsStreamParser::new(src));
        stream_parser.handle_update(msg)
    }
}

impl<I: Iterator<Item = CanMessage>> Iterator for N2kParser<I> {
    type Item = GpsData;

    fn next(&mut self) -> Option<Self::Item> {
        // Skip over any non-n2k messages
        let mut msg = self.msgs.next()?;
        loop {
            if NMEA_2000_PGNS.contains(&msg.pgn) {
                break;
            }
            msg = self.msgs.next()?;
        }

        let src = msg.src;
        // Spooky recursion is used to advance the iterator in the case the parsing an N2K message
        // doesn't yield a GpsData:
        //
        // 1. No GNSS Position Data has been received yet
        // 2. There was an invalid N2K message that needed to be skipped
        // 3. There was a non-position N2K message (altitude-delta, cog-sog-rapid-update,
        //    position-rapid-update) that didn't yield a GpsData message.
        //
        // This means that the amount it recurses is very low unless you have pathological data, in
        // which case this implementation can be improved.
        let Some(msg) = parse_n2k_msg(msg) else {
            return self.next();
        };
        let Some(gps) = self.reconstruct_gps(src, msg) else {
            return self.next();
        };
        Some(gps)
    }
}
