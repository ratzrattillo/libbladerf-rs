//! Gain and frequency range types.
//!
//! Represents hardware parameter ranges as a collection of items that may be
//! continuous intervals, discrete values, or stepped ranges with scaling factors.

/// Single element within a parameter range.
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum RangeItem {
    /// Continuous range between two endpoints (inclusive).
    Interval(f64, f64),
    /// Single discrete value.
    Value(f64),
    /// Stepped range with `min`, `max`, `step`, and `scale` factor.
    Step(f64, f64, f64, f64),
}

impl RangeItem {
    /// Returns the lower bound of this range item.
    pub fn min(&self) -> f64 {
        match self {
            RangeItem::Interval(min, _max) => *min,
            RangeItem::Value(value) => *value,
            RangeItem::Step(min, _max, _step, _scale) => *min,
        }
    }
    /// Returns the upper bound of this range item.
    pub fn max(&self) -> f64 {
        match self {
            RangeItem::Interval(_min, max) => *max,
            RangeItem::Value(value) => *value,
            RangeItem::Step(_min, max, _step, _scale) => *max,
        }
    }
    /// Returns the step increment for stepped ranges, or `None` for other variants.
    pub fn step(&self) -> Option<f64> {
        match self {
            RangeItem::Interval(_min, _max) => None,
            RangeItem::Value(_value) => None,
            RangeItem::Step(_min, _max, step, _scale) => Some(*step),
        }
    }
    /// Returns the scale factor for stepped ranges, or `None` for other variants.
    pub fn scale(&self) -> Option<f64> {
        match self {
            RangeItem::Interval(_min, _max) => None,
            RangeItem::Value(_value) => None,
            RangeItem::Step(_min, _max, _step, scale) => Some(*scale),
        }
    }
}

/// Collection of range items representing valid parameter values.
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct Range {
    items: Vec<RangeItem>,
}

impl Range {
    /// Creates a new range from the given items.
    pub fn new(items: Vec<RangeItem>) -> Self {
        Self { items }
    }
    /// Returns the minimum value across all range items, or `None` if empty.
    pub fn min(&self) -> Option<f64> {
        self.items
            .iter()
            .reduce(|a, b| if a.min() < b.min() { a } else { b })
            .map(|item| item.min())
    }
    /// Returns the maximum value across all range items, or `None` if empty.
    pub fn max(&self) -> Option<f64> {
        self.items
            .iter()
            .reduce(|a, b| if a.max() > b.max() { a } else { b })
            .map(|item| item.max())
    }
    /// Returns the step value from the first range item, or `None` if not applicable.
    pub fn step(&self) -> Option<f64> {
        self.items.first().and_then(|item| item.step())
    }
    /// Returns the scale factor from the first range item, or `None` if not applicable.
    pub fn scale(&self) -> Option<f64> {
        self.items.first().and_then(|item| item.scale())
    }

    /// Returns the step value, or an error if the range does not have a step.
    pub fn step_checked(&self) -> crate::error::Result<f64> {
        self.step()
            .ok_or(crate::error::Error::BoardState("gain range missing step"))
    }
    /// Returns the scale factor, or an error if the range does not have a scale.
    pub fn scale_checked(&self) -> crate::error::Result<f64> {
        self.scale()
            .ok_or(crate::error::Error::BoardState("gain range missing scale"))
    }
    /// Returns the minimum value, or an error if the range is empty.
    pub fn min_checked(&self) -> crate::error::Result<f64> {
        self.min()
            .ok_or(crate::error::Error::BoardState("gain range missing min"))
    }
    /// Returns the maximum value, or an error if the range is empty.
    pub fn max_checked(&self) -> crate::error::Result<f64> {
        self.max()
            .ok_or(crate::error::Error::BoardState("gain range missing max"))
    }
    /// Returns `true` if the value falls within any of the range items.
    /// For stepped ranges, checks that the value aligns with the step grid.
    /// Uses epsilon-aware comparison for floating-point equality.
    pub fn contains(&self, value: f64) -> bool {
        for item in &self.items {
            match *item {
                RangeItem::Interval(a, b) => {
                    if a <= value && value <= b {
                        return true;
                    }
                }
                RangeItem::Value(v) => {
                    if (v - value).abs() <= v.abs().max(value.abs()) * f64::EPSILON * 2.0 {
                        return true;
                    }
                }
                RangeItem::Step(min, max, step, _scale) => {
                    if value < min {
                        continue;
                    }
                    let mut v = min + ((value - min) / step).floor() * step;
                    while v <= max && v <= value {
                        if (v - value).abs() <= v.abs().max(value.abs()) * f64::EPSILON * 2.0 {
                            return true;
                        }
                        v += step;
                    }
                }
            }
        }
        false
    }
    /// Finds the value within the range that is closest to the target.
    /// If the target is already within the range, returns it as-is.
    /// Returns the nearest valid value from all range items.
    pub fn closest(&self, value: f64) -> Option<f64> {
        fn closer(target: f64, closest: Option<f64>, current: f64) -> f64 {
            match closest {
                Some(c) => {
                    if (target - current).abs() < (c - target).abs() {
                        current
                    } else {
                        c
                    }
                }
                None => current,
            }
        }
        if self.contains(value) {
            Some(value)
        } else {
            let mut close = None;
            for i in self.items.iter() {
                match i {
                    RangeItem::Interval(a, b) => {
                        close = Some(closer(value, close, *a));
                        close = Some(closer(value, close, *b));
                    }
                    RangeItem::Value(a) => {
                        close = Some(closer(value, close, *a));
                    }
                    RangeItem::Step(min, max, step, _scale) => {
                        if value <= *min {
                            close = Some(closer(value, close, *min));
                            continue;
                        }
                        if value >= *max {
                            close = Some(closer(value, close, *max));
                            continue;
                        }
                        let mut v = min + ((value - min) / step).floor() * step;
                        while v <= *max && v <= value + step {
                            close = Some(closer(value, close, v));
                            v += step;
                        }
                    }
                }
            }
            close
        }
    }
    /// Finds the smallest value within the range that is at least the target.
    /// If the target is already within the range, returns it as-is.
    /// Returns `None` if no valid value meets or exceeds the target.
    pub fn at_least(&self, value: f64) -> Option<f64> {
        fn closer_at_least(target: f64, closest: Option<f64>, current: f64) -> Option<f64> {
            match closest {
                Some(c) => {
                    if (target - current).abs() < (c - target).abs() && current >= target {
                        Some(current)
                    } else {
                        closest
                    }
                }
                None => {
                    if current >= target {
                        Some(current)
                    } else {
                        None
                    }
                }
            }
        }
        if self.contains(value) {
            Some(value)
        } else {
            let mut close = None;
            for i in self.items.iter() {
                match i {
                    RangeItem::Interval(a, b) => {
                        close = closer_at_least(value, close, *a);
                        close = closer_at_least(value, close, *b);
                    }
                    RangeItem::Value(a) => {
                        close = closer_at_least(value, close, *a);
                    }
                    RangeItem::Step(min, max, step, _scale) => {
                        if value <= *min {
                            close = closer_at_least(value, close, *min);
                            continue;
                        }
                        if value >= *max {
                            close = closer_at_least(value, close, *max);
                            continue;
                        }
                        let mut v = min + ((value - min) / step).floor() * step;
                        while v <= *max && v <= value + step {
                            close = closer_at_least(value, close, v);
                            v += step;
                        }
                    }
                }
            }
            close
        }
    }
    /// Finds the largest value within the range that does not exceed the target.
    /// If the target is already within the range, returns it as-is.
    /// Returns `None` if no valid value is at or below the target.
    pub fn at_max(&self, value: f64) -> Option<f64> {
        fn closer_at_max(target: f64, closest: Option<f64>, current: f64) -> Option<f64> {
            match closest {
                Some(c) => {
                    if (target - current).abs() < (c - target).abs() && current <= target {
                        Some(current)
                    } else {
                        closest
                    }
                }
                None => {
                    if current <= target {
                        Some(current)
                    } else {
                        None
                    }
                }
            }
        }
        if self.contains(value) {
            Some(value)
        } else {
            let mut close = None;
            for i in self.items.iter() {
                match i {
                    RangeItem::Interval(a, b) => {
                        close = closer_at_max(value, close, *a);
                        close = closer_at_max(value, close, *b);
                    }
                    RangeItem::Value(a) => {
                        close = closer_at_max(value, close, *a);
                    }
                    RangeItem::Step(min, max, step, _scale) => {
                        if value <= *min {
                            close = closer_at_max(value, close, *min);
                            continue;
                        }
                        if value >= *max {
                            close = closer_at_max(value, close, *max);
                            continue;
                        }
                        let mut v = min + ((value - min) / step).floor() * step;
                        while v <= *max && v <= value + step {
                            close = closer_at_max(value, close, v);
                            v += step;
                        }
                    }
                }
            }
            close
        }
    }
    /// Returns an iterator over the range items.
    pub fn iter(&self) -> impl Iterator<Item = &RangeItem> {
        self.items.iter()
    }
}
