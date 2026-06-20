use libbladerf_rs::bladerf1::DcPair;
use libbladerf_rs::bladerf1::calibration::{DcCalEntry, DcCalTable};
use libbladerf_rs::bladerf1::hardware::lms6002d::dc_calibration::DcCals;

fn test_table() -> DcCalTable {
    DcCalTable::new(
        DcCals::new(20, 10, 15, 25, 30, 5, 12, 18, 8, 22),
        vec![
            DcCalEntry::new(100_000_000, DcPair::new(100, 200)).with_agc(
                DcPair::new(1000, 2000),
                DcPair::new(500, 600),
                DcPair::new(100, 200),
            ),
            DcCalEntry::new(200_000_000, DcPair::new(200, 400)).with_agc(
                DcPair::new(2000, 4000),
                DcPair::new(1000, 1200),
                DcPair::new(200, 400),
            ),
            DcCalEntry::new(300_000_000, DcPair::new(300, 600)).with_agc(
                DcPair::new(3000, 6000),
                DcPair::new(1500, 1800),
                DcPair::new(300, 600),
            ),
        ],
    )
}

#[test]
fn roundtrip() {
    let table = test_table();
    let json = serde_json::to_string(&table).unwrap();
    let parsed: DcCalTable = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.reg_vals().lpf_tuning(), 20);
    assert_eq!(parsed.reg_vals().tx_lpf_i(), 10);
    assert_eq!(parsed.reg_vals().rxvga2b_q(), 22);
    assert_eq!(parsed.entries().len(), 3);
    for (orig, got) in table.entries().iter().zip(parsed.entries().iter()) {
        assert_eq!(orig, got);
    }
}

#[test]
fn save_and_load() {
    let table = test_table();
    let dir = std::env::temp_dir().join("libbladerf_rs_dc_cal_table_unit_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.json");
    table.save(&path).unwrap();
    let loaded = DcCalTable::load(&path).unwrap();
    assert_eq!(loaded.entries().len(), 3);
    for (orig, got) in table.entries().iter().zip(loaded.entries().iter()) {
        assert_eq!(orig, got);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn lookup_exact_match() {
    let table = test_table();
    let entry = table.lookup(200_000_000);
    assert_eq!(entry.freq, 200_000_000);
    assert_eq!(entry.dc.i, 200);
    assert_eq!(entry.dc.q, 400);
    assert_eq!(entry.max_dc.i, 2000);
    assert_eq!(entry.max_dc.q, 4000);
}

#[test]
fn lookup_interpolation() {
    let table = test_table();
    let entry = table.lookup(150_000_000);
    assert_eq!(entry.freq, 150_000_000);
    assert_eq!(entry.dc.i, 150);
    assert_eq!(entry.dc.q, 300);
    assert_eq!(entry.max_dc.i, 1500);
    assert_eq!(entry.max_dc.q, 3000);
    assert_eq!(entry.mid_dc.i, 750);
    assert_eq!(entry.mid_dc.q, 900);
    assert_eq!(entry.min_dc.i, 150);
    assert_eq!(entry.min_dc.q, 300);
}

#[test]
fn lookup_below_range() {
    let table = test_table();
    let entry = table.lookup(50_000_000);
    assert_eq!(entry.dc.i, 100);
    assert_eq!(entry.dc.q, 200);
}

#[test]
fn lookup_above_range() {
    let table = test_table();
    let entry = table.lookup(400_000_000);
    assert_eq!(entry.dc.i, 300);
    assert_eq!(entry.dc.q, 600);
}

#[test]
fn empty_table() {
    let table = DcCalTable::new(DcCals::new(-1, -1, -1, -1, -1, -1, -1, -1, -1, -1), vec![]);
    let entry = table.lookup(100_000_000);
    assert_eq!(entry.freq, 100_000_000);
    assert_eq!(entry.dc.i, 0);
    assert_eq!(entry.dc.q, 0);
}

#[test]
fn single_entry() {
    let table = DcCalTable::new(
        DcCals::new(-1, -1, -1, -1, -1, -1, -1, -1, -1, -1),
        vec![DcCalEntry::new(200_000_000, DcPair::new(42, 84))],
    );
    let entry = table.lookup(1);
    assert_eq!(entry.dc.i, 42);
    assert_eq!(entry.dc.q, 84);
    let entry2 = table.lookup(999_000_000_000);
    assert_eq!(entry2.dc.i, 42);
    assert_eq!(entry2.dc.q, 84);
}

#[test]
fn invalid_json() {
    let result: Result<DcCalTable, _> = serde_json::from_str("not json");
    assert!(result.is_err());
}
