pub fn decay_salience(salience: f64, delta_days: f64) -> f64 {
    salience * f64::exp(-delta_days / 30.0)
}
