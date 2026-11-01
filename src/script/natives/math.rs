//! Papyrus math uses degrees at the API boundary. Public references: research/pex-natives/.
use papyrus::Value;

pub(super) fn call(func: &str, args: &[Value]) -> Option<Value> {
    let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
    Some(match func {
        "abs" => Value::Float(arg(0).as_float().abs()),
        "sqrt" => Value::Float(arg(0).as_float().sqrt()),
        // Skyrim SE gave 0 for pow(0, -1) (autotest21), not infinity.
        "pow" => Value::Float(match (arg(0).as_float(), arg(1).as_float()) {
            (b, x) if b == 0.0 && x < 0.0 => 0.0,
            (b, x) => b.powf(x),
        }),
        "sin" => Value::Float(arg(0).as_float().to_radians().sin()),
        "cos" => Value::Float(arg(0).as_float().to_radians().cos()),
        "floor" => Value::Int(arg(0).as_float().floor() as i32),
        "ceiling" => Value::Int(arg(0).as_float().ceil() as i32),
        // Out of [-1, 1] both give 0 in Skyrim SE (autotest21: asin(2) and
        // acos(-2)), not NaN.
        "acos" => Value::Float(inverse(arg(0).as_float(), f32::acos)),
        "asin" => Value::Float(inverse(arg(0).as_float(), f32::asin)),
        "atan" => Value::Float(arg(0).as_float().atan().to_degrees()),
        "tan" => Value::Float(arg(0).as_float().to_radians().tan()),
        "degreestoradians" => Value::Float(arg(0).as_float().to_radians()),
        "radianstodegrees" => Value::Float(arg(0).as_float().to_degrees()),
        _ => return None,
    })
}

fn inverse(x: f32, f: fn(f32) -> f32) -> f32 {
    if (-1.0..=1.0).contains(&x) {
        f(x).to_degrees()
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn check(func: &str, cases: &[(f32, f32)]) {
        for &(input, expected) in cases {
            let Some(Value::Float(actual)) = call(func, &[Value::Float(input)]) else {
                panic!("{func} did not return a float");
            };
            assert!(
                (actual - expected).abs() < 0.0001,
                "{func}({input}) = {actual}, wanted {expected}"
            );
        }
    }
    #[test]
    fn acos() {
        check(
            "acos",
            &[(-1.0, 180.0), (0.0, 90.0), (0.5, 60.0), (1.0, 0.0)],
        );
        check("acos", &[(2.0, 0.0), (-2.0, 0.0)]);
    }
    #[test]
    fn asin() {
        check(
            "asin",
            &[(-1.0, -90.0), (-0.5, -30.0), (0.0, 0.0), (1.0, 90.0), (2.0, 0.0)],
        );
    }
    #[test]
    fn pow_of_zero_to_a_negative_power_is_zero() {
        let r = call("pow", &[Value::Float(0.0), Value::Float(-1.0)]).unwrap();
        assert_eq!(r.as_float(), 0.0);
        let r = call("pow", &[Value::Float(-8.0), Value::Float(0.3333)]).unwrap();
        assert!(r.as_float().is_nan());
    }
    #[test]
    fn atan() {
        check("atan", &[(-1.0, -45.0), (0.0, 0.0), (1.0, 45.0)]);
    }
    #[test]
    fn tan() {
        check(
            "tan",
            &[(-45.0, -1.0), (0.0, 0.0), (45.0, 1.0), (180.0, 0.0)],
        );
    }
    #[test]
    fn degreestoradians() {
        check(
            "degreestoradians",
            &[
                (-180.0, -std::f32::consts::PI),
                (0.0, 0.0),
                (90.0, std::f32::consts::FRAC_PI_2),
            ],
        );
    }
    #[test]
    fn radianstodegrees() {
        check(
            "radianstodegrees",
            &[
                (-std::f32::consts::PI, -180.0),
                (0.0, 0.0),
                (std::f32::consts::TAU, 360.0),
            ],
        );
    }
}
