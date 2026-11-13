use data_collector::sim_lidar_bridge::export_sim_lidar_scans;
use std::io::Cursor;

#[test]
fn exports_simulator_hits_in_mapper_units_and_counts_empty_scans() {
    let input = concat!(
        "{\"timestamp\":\"1970-01-01T00:00:00Z\",\"scan_id\":\"00000000-0000-4000-8000-000000000001\",\"sensor_id\":\"sim\",\"status\":\"no_hits\",\"points\":[]}\n",
        "{\"timestamp\":\"1970-01-01T00:00:01Z\",\"scan_id\":\"00000000-0000-4000-8000-000000000002\",\"sensor_id\":\"sim\",\"status\":\"ok\",\"points\":[{\"timestamp\":\"1970-01-01T00:00:01Z\",\"angle\":90,\"distance\":2000,\"range_m\":2,\"quality\":100}]}\n"
    );
    let dir = tempfile::tempdir().unwrap();
    let result = export_sim_lidar_scans(Cursor::new(input), dir.path()).unwrap();
    assert_eq!((result.exported, result.no_hits), (1, 1));
    let scan: shared::schemas::LidarScan = serde_json::from_slice(
        &std::fs::read(
            dir.path()
                .join("scan_00000000-0000-4000-8000-000000000002.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(scan.points[0].distance, 2000.0);
    assert!(export_sim_lidar_scans(Cursor::new(input), dir.path()).is_err());
}

#[test]
fn reports_sensor_failures() {
    let dir = tempfile::tempdir().unwrap();
    let input = "{\"timestamp\":\"1970-01-01T00:00:00Z\",\"scan_id\":\"00000000-0000-4000-8000-000000000001\",\"sensor_id\":\"sim\",\"status\":\"dropout\",\"points\":[]}\n";
    assert!(export_sim_lidar_scans(Cursor::new(input), dir.path()).is_err());
}

#[test]
fn rejects_invalid_ranges_before_creating_scan_files() {
    for range in [-1.0, 0.0, 1e38] {
        let dir = tempfile::tempdir().unwrap();
        let input = serde_json::json!({
            "timestamp":"1970-01-01T00:00:00Z", "scan_id":"00000000-0000-4000-8000-000000000001",
            "sensor_id":"sim", "status":"ok", "points":[{
                "timestamp":"1970-01-01T00:00:00Z", "angle":0, "range_m":range, "quality":100
            }]
        })
        .to_string();
        assert!(export_sim_lidar_scans(Cursor::new(input), dir.path()).is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}
