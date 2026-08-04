//! Console demo: runs a simulated flight path through [`drone_geofence_demo::NO_FLY_ZONE`]
//! and prints the geofence check result for each fix.

use drone_geofence_demo::check_position;
use tpt_gis_core::GeoPoint;

fn main() {
    let flight_path = [
        GeoPoint::new(37.760, -122.430), // outside, approaching from the southwest
        GeoPoint::new(37.775, -122.415), // breach: just inside the western edge
        GeoPoint::new(37.780, -122.410), // breach: deeper inside
        GeoPoint::new(37.800, -122.395), // clear of the zone again, to the northeast
    ];

    for position in flight_path {
        let check = check_position(position);
        if check.breached {
            println!(
                "BREACH at ({:.4}, {:.4}): escape bearing {:.1} deg, {:.1} m to nearest boundary",
                position.lat_deg,
                position.lon_deg,
                check.escape_bearing_deg.unwrap(),
                check.escape_distance_m.unwrap(),
            );
        } else {
            println!(
                "OK     at ({:.4}, {:.4}): outside the no-fly zone",
                position.lat_deg, position.lon_deg
            );
        }
    }
}
