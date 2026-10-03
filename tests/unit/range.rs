use libbladerf_rs::range::{Range, RangeItem};

#[test]
fn contains_empty() {
    let r = Range::new(Vec::new());
    assert!(!r.contains(123.0));
}

#[test]
fn contains() {
    let r = Range::new(vec![
        RangeItem::Value(123.0),
        RangeItem::Interval(23.0, 42.0),
        RangeItem::Step(100.0, 110.0, 1.0, 1.0),
    ]);
    assert!(r.contains(123.0));
    assert!(r.contains(23.0));
    assert!(r.contains(42.0));
    assert!(r.contains(40.0));
    assert!(r.contains(100.0));
    assert!(r.contains(107.0));
    assert!(r.contains(110.0));
    assert!(!r.contains(19.0));
}

#[test]
fn closest() {
    let r = Range::new(vec![
        RangeItem::Value(123.0),
        RangeItem::Interval(23.0, 42.0),
        RangeItem::Step(100.0, 110.0, 1.0, 1.0),
    ]);
    assert_eq!(r.closest(122.0), Some(123.0));
    assert_eq!(r.closest(1_000.0), Some(123.0));
    assert_eq!(r.closest(30.0), Some(30.0));
    assert_eq!(r.closest(20.0), Some(23.0));
    assert_eq!(r.closest(50.0), Some(42.0));
    assert_eq!(r.closest(99.5), Some(100.0));
    assert_eq!(r.closest(105.3), Some(105.0));
    assert_eq!(r.closest(105.8), Some(106.0));
    assert_eq!(r.closest(109.8), Some(110.0));
    assert_eq!(r.closest(113.8), Some(110.0));
}

#[test]
fn at_least() {
    let r = Range::new(vec![
        RangeItem::Value(123.0),
        RangeItem::Interval(23.0, 42.0),
        RangeItem::Step(100.0, 110.0, 1.0, 1.0),
    ]);
    assert_eq!(r.at_least(120.0), Some(123.0));
    assert_eq!(r.at_least(1_000.0), None);
    assert_eq!(r.at_least(30.0), Some(30.0));
    assert_eq!(r.at_least(10.0), Some(23.0));
    assert_eq!(r.at_least(99.0), Some(100.0));
    assert_eq!(r.at_least(105.5), Some(106.0));
}

#[test]
fn at_max() {
    let r = Range::new(vec![
        RangeItem::Value(123.0),
        RangeItem::Interval(23.0, 42.0),
        RangeItem::Step(100.0, 110.0, 1.0, 1.0),
    ]);
    assert_eq!(r.at_max(90.0), Some(42.0));
    assert_eq!(r.at_max(10.0), None);
    assert_eq!(r.at_max(30.0), Some(30.0));
    assert_eq!(r.at_max(50.0), Some(42.0));
    assert_eq!(r.at_max(101.0), Some(101.0));
    assert_eq!(r.at_max(100.3), Some(100.0));
    assert_eq!(r.at_max(111.3), Some(110.0));
}

#[test]
fn empty_range_has_no_neighbors() {
    let r = Range::new(Vec::new());
    assert_eq!(r.closest(1.0), None);
    assert_eq!(r.at_least(1.0), None);
    assert_eq!(r.at_max(1.0), None);
}

#[test]
fn nan_has_no_neighbors() {
    let r = Range::new(vec![
        RangeItem::Value(123.0),
        RangeItem::Interval(23.0, 42.0),
        RangeItem::Step(100.0, 110.0, 1.0, 1.0),
    ]);
    assert!(!r.contains(f64::NAN));
    assert_eq!(r.closest(f64::NAN), None);
    assert_eq!(r.at_least(f64::NAN), None);
    assert_eq!(r.at_max(f64::NAN), None);
}

#[test]
fn infinities_are_not_contained() {
    let r = Range::new(vec![RangeItem::Step(100.0, 110.0, 1.0, 1.0)]);
    assert!(!r.contains(f64::INFINITY));
    assert!(!r.contains(f64::NEG_INFINITY));
    assert_eq!(r.closest(f64::INFINITY), Some(110.0));
    assert_eq!(r.closest(f64::NEG_INFINITY), Some(100.0));
    assert_eq!(r.at_least(f64::INFINITY), None);
    assert_eq!(r.at_max(f64::NEG_INFINITY), None);
}

#[test]
fn closest_handles_extreme_finite_values() {
    let r = Range::new(vec![
        RangeItem::Value(-f64::MAX),
        RangeItem::Value(f64::MAX),
    ]);
    assert_eq!(r.closest(-1e300), Some(-f64::MAX));
    assert_eq!(r.closest(1e300), Some(f64::MAX));
}

#[test]
fn closest_prefers_lower_on_ties() {
    let r = Range::new(vec![RangeItem::Step(100.0, 110.0, 1.0, 1.0)]);
    assert_eq!(r.closest(105.5), Some(105.0));
}

#[test]
fn stepped_grid_boundaries() {
    let r = Range::new(vec![RangeItem::Step(0.0, 10.0, 2.0, 1.0)]);
    assert!(r.contains(0.0));
    assert!(r.contains(6.0));
    assert!(r.contains(10.0));
    assert!(!r.contains(1.0));
    assert!(!r.contains(11.0));
    assert_eq!(r.at_least(10.0), Some(10.0));
    assert_eq!(r.at_max(10.0), Some(10.0));
    assert_eq!(r.at_least(11.0), None);
    assert_eq!(r.at_max(-1.0), None);
}

#[test]
fn stepped_searches_handle_rounding_at_bounds() {
    let r = Range::new(vec![RangeItem::Step(-10.0, -1e-16, 1.0, 1.0)]);
    assert_eq!(r.at_max(-1e-16), Some(-1.0));
    assert_eq!(r.at_least(-1e-16), None);
    assert_eq!(r.closest(-1e-16), Some(-1.0));
    assert_eq!(r.closest(f64::INFINITY), Some(-1.0));
}
