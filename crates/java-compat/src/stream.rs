//! `java.util.stream` numeric reductions.

/// `DoubleStream.sum()` for a sequential stream: Kahan compensated summation
/// (`Collectors.sumWithCompensation` + `computeFinalSum`). Not the same as a
/// plain left-to-right sum.
pub fn double_sum(values: impl IntoIterator<Item = f64>) -> f64 {
    let (mut high, mut compensation, mut simple) = (0.0f64, 0.0f64, 0.0f64);
    for value in values {
        let tmp = value - compensation;
        let sum = high + tmp;
        compensation = (sum - high) - tmp;
        high = sum;
        simple += value;
    }
    let tmp = high - compensation;
    // Kahan yields NaN for +inf + -inf; the simple sum keeps the infinity.
    if tmp.is_nan() && simple.is_infinite() {
        simple
    } else {
        tmp
    }
}
