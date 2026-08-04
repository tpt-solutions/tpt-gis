//! A small, curated EPSG code registry.
//!
//! `tpt-gis-core` does not yet embed the full EPSG database (thousands of CRS
//! definitions with complex per-datum parameters) — that is tracked as future work.
//! For now, this module resolves the handful of CRSs in [`crate::crs::Crs`]: WGS84,
//! Web Mercator, and any UTM zone (computed formulaically, not tabulated).

use crate::crs::Crs;
use crate::projection::utm::Zone;

/// Looks up the [`Crs`] for a given EPSG code, if it is one this registry knows about.
#[must_use]
pub fn lookup(epsg_code: u32) -> Option<Crs> {
    match epsg_code {
        4326 => Some(Crs::Wgs84),
        3857 => Some(Crs::WebMercator),
        32601..=32660 => Some(Crs::Utm(Zone::new((epsg_code - 32600) as u8, true))),
        32701..=32760 => Some(Crs::Utm(Zone::new((epsg_code - 32700) as u8, false))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_up_wgs84() {
        assert_eq!(lookup(4326), Some(Crs::Wgs84));
    }

    #[test]
    fn looks_up_utm_zone() {
        assert_eq!(lookup(32618), Some(Crs::Utm(Zone::new(18, true))));
        assert_eq!(lookup(32718), Some(Crs::Utm(Zone::new(18, false))));
    }

    #[test]
    fn unknown_code_returns_none() {
        assert_eq!(lookup(27700), None); // British National Grid — not yet in the registry.
    }
}
