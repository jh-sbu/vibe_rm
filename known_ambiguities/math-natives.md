# Math natives

Public documentation specifies degrees for inverse trig results, but does not
settle out-of-domain or nonfinite inputs, exact rounding, or tangent at its poles.
No engine binaries were examined. References are in research/pex-natives/.

Checked in Skyrim SE (autotest21, 2026-11-01; `research/autotest21.md`):
`asin(2)` and `acos(-2)` give 0, `sqrt(-1)` NaN, `pow(0, -1)` 0, `pow(-8, 0.3333)`
NaN. `tan(90)` gives -22877332 and `tan(270)` -83858280, which are f32's tangents
of the converted radians, as ours. `Floor(-0.5)` gives -1 and `Ceiling(-0.5)` 0.
Casting to String gives six decimals (`0.333333`), prints NaN as `-nan(ind)`, and
prints true as `TRUE` (false as `False`). A float out of Int range casts to
-2147483648. We do all of these.

Still our choice: out-of-domain inputs other than those tried (we return 0 for
any input outside [-1, 1]), `pow` of zero to other negative powers (0), and how
infinity prints (`inf`, the C runtime's name).
