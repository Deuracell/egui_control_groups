//! [`Number`]: a numeric value with display formatting for a
//! [`FieldGroup`](crate::FieldGroup).

use std::ops::RangeInclusive;

use crate::fields::{FieldValue, format_float};

/// A number type [`Number`] can wrap: `f32`, `f64` and the integer types
/// [`FieldValue`] supports.
pub trait NumberValue: Copy + PartialOrd {
    /// The text for the value. `precision` is the number of decimals; whole
    /// number types ignore it.
    fn format_number(self, precision: Option<usize>) -> String;
    /// Parse already stripped text. `None` if it is not a valid number.
    fn parse_number(text: &str) -> Option<Self>;
    /// Round to `precision` decimals (whole number types: unchanged).
    fn round_to(self, precision: usize) -> Self;
}

fn fixed(v: f64, precision: usize) -> String {
    let s = format!("{v:.precision$}");
    // "-0.00" is noise: show it as "0.00".
    match s.strip_prefix('-') {
        Some(rest) if rest.chars().all(|c| matches!(c, '0' | '.')) => rest.to_owned(),
        _ => s,
    }
}

macro_rules! number_float {
    ($($t:ty),*) => {$(
        impl NumberValue for $t {
            fn format_number(self, precision: Option<usize>) -> String {
                match precision {
                    Some(p) if self.is_finite() => fixed(f64::from(self), p),
                    _ => format_float(f64::from(self)),
                }
            }
            fn parse_number(text: &str) -> Option<Self> {
                text.replace(',', ".").parse::<$t>().ok().filter(|v| v.is_finite())
            }
            fn round_to(self, precision: usize) -> Self {
                fixed(f64::from(self), precision).parse().unwrap_or(self)
            }
        }
    )*};
}
macro_rules! number_int {
    ($($t:ty),*) => {$(
        impl NumberValue for $t {
            fn format_number(self, _precision: Option<usize>) -> String {
                self.to_string()
            }
            fn parse_number(text: &str) -> Option<Self> {
                // Saturate: a valid integer outside the type's range becomes
                // its MIN / MAX; text that is not an integer is rejected.
                text.parse::<$t>().ok().or_else(|| {
                    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
                    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
                        .then(|| if text.starts_with('-') { <$t>::MIN } else { <$t>::MAX })
                })
            }
            fn round_to(self, _precision: usize) -> Self {
                self
            }
        }
    )*};
}
number_float!(f32, f64);
number_int!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

/// A number with display formatting, for use as a [`FieldValue`].
///
/// ```no_run
/// # use egui_control_groups::{FieldGroup, Number};
/// # egui::__run_test_ui(|ui| {
/// let mut opacity = 0.5_f32;
/// let mut percent = Number::new(&mut opacity).precision(1).range(0.0..=1.0);
/// ui.add(FieldGroup::new("opacity").labeled("Alpha", &mut percent));
/// # });
/// ```
///
/// The wrapper borrows the value, so bind it to a variable before handing it
/// to [`FieldGroup::field`](crate::FieldGroup::field) or
/// [`labeled`](crate::FieldGroup::labeled).
///
/// * [`precision`](Self::precision) fixes the number of decimals shown (floats
///   only). A committed float is rounded to it, so the value always matches
///   what is displayed. Without it floats show up to six decimals, trailing
///   zeros removed.
/// * [`range`](Self::range) clamps committed values into the range. Integers
///   typed beyond their type's limits saturate first (`300` into a `u8` is
///   `255`, then clamped to the range).
/// * [`prefix`](Self::prefix) and [`suffix`](Self::suffix) are shown around
///   the number and accepted, but not required, when parsing.
///
/// Like every field, a value is only written when the text was edited, so
/// opening a field and leaving it never rounds or clamps the stored value.
#[must_use = "Pass to `FieldGroup::field` or `labeled`."]
pub struct Number<'a, T: NumberValue> {
    value: &'a mut T,
    precision: Option<usize>,
    range: Option<RangeInclusive<T>>,
    prefix: String,
    suffix: String,
}

impl<'a, T: NumberValue> Number<'a, T> {
    /// Wrap `value` with the default formatting.
    pub fn new(value: &'a mut T) -> Self {
        Self {
            value,
            precision: None,
            range: None,
            prefix: String::new(),
            suffix: String::new(),
        }
    }

    /// Number of decimals to show and round to. Ignored for integers.
    pub fn precision(mut self, decimals: usize) -> Self {
        self.precision = Some(decimals);
        self
    }

    /// Clamp committed values into `range`. An empty range (start above end)
    /// is ignored.
    pub fn range(mut self, range: RangeInclusive<T>) -> Self {
        self.range = (range.start() <= range.end()).then_some(range);
        self
    }

    /// Text shown before the number, e.g. `"$"`.
    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Text shown after the number, e.g. `" px"` or `"%"`.
    pub fn suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = suffix.into();
        self
    }
}

impl<T: NumberValue> FieldValue for Number<'_, T> {
    fn to_field_text(&self) -> String {
        format!(
            "{}{}{}",
            self.prefix,
            self.value.format_number(self.precision),
            self.suffix
        )
    }

    fn set_from_field_text(&mut self, text: &str) -> bool {
        let mut t = text.trim();
        let (prefix, suffix) = (self.prefix.trim(), self.suffix.trim());
        if let Some(rest) = t.strip_prefix(prefix) {
            t = rest.trim_start();
        }
        if let Some(rest) = t.strip_suffix(suffix) {
            t = rest.trim_end();
        }
        let Some(mut v) = T::parse_number(t) else {
            return false;
        };
        if let Some(p) = self.precision {
            v = v.round_to(p);
        }
        if let Some(r) = &self.range {
            if v < *r.start() {
                v = *r.start();
            } else if v > *r.end() {
                v = *r.end();
            }
        }
        *self.value = v;
        true
    }
}
