//! Local loose-object avoidance; choices are in offscreen-navigation.md.
use glam::{Vec2, Vec3};

fn hits(a: Vec2, b: Vec2, lo: Vec2, hi: Vec2) -> bool {
    let d = b - a;
    let (mut enter, mut exit) = (0f32, 1f32);
    for axis in 0..2 {
        if d[axis].abs() < 1e-5 {
            if a[axis] < lo[axis] || a[axis] > hi[axis] {
                return false;
            }
        } else {
            let x = (lo[axis] - a[axis]) / d[axis];
            let y = (hi[axis] - a[axis]) / d[axis];
            enter = enter.max(x.min(y));
            exit = exit.min(x.max(y));
        }
    }
    enter <= exit
}

pub(super) fn steer(
    from: Vec3,
    to: Vec3,
    clutter: &[(Vec3, Vec3)],
    dt: f32,
    height: impl Fn(Vec3) -> Option<f32>,
) -> Vec3 {
    let delta = (to - from).truncate();
    let length = delta.length();
    if length < 0.001 || length > 100. || dt <= 0. {
        return to;
    }
    let direction = delta / length;
    let ahead = from.truncate() + direction * 90.;
    let relevant: Vec<_> = clutter
        .iter()
        .filter(|(lo, hi)| hi.z > from.z + 12. && lo.z < from.z + 100.)
        .map(|(lo, hi)| {
            (
                lo.truncate() - Vec2::splat(24.),
                hi.truncate() + Vec2::splat(24.),
            )
        })
        .collect();
    let Some(&(lo, hi)) = relevant
        .iter()
        .find(|(lo, hi)| hits(from.truncate(), ahead, *lo, *hi))
    else {
        return to;
    };
    let left = direction.perp();
    let side = if left.dot((lo + hi) * 0.5 - from.truncate()) > 0. {
        -left
    } else {
        left
    };
    for turn in [side, -side] {
        let step = (direction * 0.35 + turn * 0.65) * length.min(80. * dt);
        let mut p = from + step.extend(0.);
        if relevant
            .iter()
            .any(|(lo, hi)| hits(from.truncate(), p.truncate(), *lo, *hi))
        {
            continue;
        }
        if let Some(z) = height(p)
            && (z - from.z).abs() < 30.
        {
            p.z = z;
            return p;
        }
    }
    from
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slows_and_sidesteps_before_contact_and_waits_without_floor() {
        let from = Vec3::ZERO;
        let to = Vec3::new(8., 0., 0.);
        let box_ = [(Vec3::new(65., -15., 0.), Vec3::new(85., 15., 45.))];
        let p = steer(from, to, &box_, 0.1, |_| Some(0.));
        assert!(p.x > 0. && p.x < to.x && p.y.abs() > 4.);
        assert_eq!(steer(from, to, &box_, 0.1, |_| None), from);
        assert_eq!(
            steer(
                from,
                to,
                &[(box_[0].0 + Vec3::Z * 150., box_[0].1 + Vec3::Z * 150.)],
                0.1,
                |_| Some(0.)
            ),
            to
        );
        let box_ = [(Vec3::new(20., -100., 0.), Vec3::new(85., 100., 45.))];
        assert_eq!(steer(from, to, &box_, 0.1, |_| Some(0.)), from);
    }
}
