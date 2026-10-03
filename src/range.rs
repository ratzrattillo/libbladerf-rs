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

fn nearly_equal(a: f64, b: f64) -> bool {
    (a - b).abs() <= a.abs().max(b.abs()) * f64::EPSILON * 2.0
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
        self.items.iter().map(RangeItem::min).reduce(f64::min)
    }
    /// Returns the maximum value across all range items, or `None` if empty.
    pub fn max(&self) -> Option<f64> {
        self.items.iter().map(RangeItem::max).reduce(f64::max)
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
            .ok_or(crate::error::Error::Internal("gain range missing step"))
    }
    /// Returns the scale factor, or an error if the range does not have a scale.
    pub fn scale_checked(&self) -> crate::error::Result<f64> {
        self.scale()
            .ok_or(crate::error::Error::Internal("gain range missing scale"))
    }
    /// Returns the minimum value, or an error if the range is empty.
    pub fn min_checked(&self) -> crate::error::Result<f64> {
        self.min()
            .ok_or(crate::error::Error::Internal("gain range missing min"))
    }
    /// Returns the maximum value, or an error if the range is empty.
    pub fn max_checked(&self) -> crate::error::Result<f64> {
        self.max()
            .ok_or(crate::error::Error::Internal("gain range missing max"))
    }
    /// Returns `true` if the value falls within any of the range items.
    /// For stepped ranges, checks that the value aligns with the step grid.
    /// Uses epsilon-aware comparison for floating-point equality.
    pub fn contains(&self, value: f64) -> bool {
        if !value.is_finite() {
            return false;
        }
        self.items.iter().any(|item| match *item {
            RangeItem::Interval(min, max) => min <= value && value <= max,
            RangeItem::Value(allowed) => nearly_equal(allowed, value),
            RangeItem::Step(min, max, step, _scale) => {
                value >= min
                    && value <= max
                    && nearly_equal(min + ((value - min) / step).round() * step, value)
            }
        })
    }
    /// Finds the value within the range that is closest to the target.
    /// If the target is already within the range, returns it as-is.
    /// Ties select the smaller value; a NaN target yields `None`.
    /// Returns the nearest valid value from all range items.
    pub fn closest(&self, value: f64) -> Option<f64> {
        match (self.at_max(value), self.at_least(value)) {
            (Some(lower), Some(upper)) => {
                if value / 2.0 - lower / 2.0 <= upper / 2.0 - value / 2.0 {
                    Some(lower)
                } else {
                    Some(upper)
                }
            }
            (Some(candidate), None) | (None, Some(candidate)) => Some(candidate),
            (None, None) => None,
        }
    }
    /// Finds the smallest value within the range that is at least the target.
    /// If the target is already within the range, returns it as-is.
    /// Returns `None` if no valid value meets or exceeds the target, or the target is NaN.
    pub fn at_least(&self, value: f64) -> Option<f64> {
        if value.is_nan() {
            return None;
        }
        self.items
            .iter()
            .filter_map(|item| {
                let candidate = match *item {
                    RangeItem::Interval(min, max) => (value <= max).then_some(value.max(min)),
                    RangeItem::Value(allowed) => (value <= allowed).then_some(allowed),
                    RangeItem::Step(min, max, step, _scale) => {
                        if value <= min {
                            Some(min)
                        } else if value > max {
                            None
                        } else {
                            let index = ((value - min) / step).ceil();
                            let mut candidate = min + index * step;
                            if candidate < value {
                                let next = (index + 1.0).max(index.next_up());
                                candidate = min + next * step;
                            }
                            (candidate <= max).then_some(candidate)
                        }
                    }
                };
                candidate.filter(|candidate| *candidate >= value)
            })
            .reduce(f64::min)
    }
    /// Finds the largest value within the range that does not exceed the target.
    /// If the target is already within the range, returns it as-is.
    /// Returns `None` if no valid value is at or below the target, or the target is NaN.
    pub fn at_max(&self, value: f64) -> Option<f64> {
        if value.is_nan() {
            return None;
        }
        self.items
            .iter()
            .filter_map(|item| {
                let candidate = match *item {
                    RangeItem::Interval(min, max) => (value >= min).then_some(value.min(max)),
                    RangeItem::Value(allowed) => (value >= allowed).then_some(allowed),
                    RangeItem::Step(min, max, step, _scale) => (value >= min).then(|| {
                        let limit = value.min(max);
                        let index = ((limit - min) / step).floor();
                        let mut candidate = min + index * step;
                        if candidate > limit {
                            let previous = (index - 1.0).min(index.next_down()).max(0.0);
                            candidate = min + previous * step;
                        }
                        candidate
                    }),
                };
                candidate.filter(|candidate| *candidate <= value)
            })
            .reduce(f64::max)
    }
    /// Returns an iterator over the range items.
    pub fn iter(&self) -> impl Iterator<Item = &RangeItem> {
        self.items.iter()
    }
}
