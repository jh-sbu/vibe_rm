//! Papyrus math uses degrees at the API boundary. Public references: refs/pex-natives/.
use papyrus::Value;

pub(super) fn call(func: &str, args: &[Value]) -> Option<Value> {
    let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
    Some(match func {
        "abs" => Value::Float(arg(0).as_float().abs()),
        "sqrt" => Value::Float(arg(0).as_float().sqrt()),
        "pow" => Value::Float(arg(0).as_float().powf(arg(1).as_float())),
        "sin" => Value::Float(arg(0).as_float().to_radians().sin()),
        "cos" => Value::Float(arg(0).as_float().to_radians().cos()),
        "floor" => Value::Int(arg(0).as_float().floor() as i32),
        "ceiling" => Value::Int(arg(0).as_float().ceil() as i32),
        "acos" => Value::Float(arg(0).as_float().acos().to_degrees()),
        "asin" => Value::Float(arg(0).as_float().asin().to_degrees()),
        "atan" => Value::Float(arg(0).as_float().atan().to_degrees()),
        "tan" => Value::Float(arg(0).as_float().to_radians().tan()),
        "degreestoradians" => Value::Float(arg(0).as_float().to_radians()),
        "radianstodegrees" => Value::Float(arg(0).as_float().to_degrees()),
        _ => return None,
    })
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
        assert!(
            call("acos", &[Value::Float(2.0)])
                .unwrap()
                .as_float()
                .is_nan()
        );
    }
    #[test]
    fn asin() {
        check(
            "asin",
            &[(-1.0, -90.0), (-0.5, -30.0), (0.0, 0.0), (1.0, 90.0)],
        );
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
