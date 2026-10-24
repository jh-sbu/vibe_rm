# Math natives

Public documentation specifies degrees for inverse trig results, but does not
settle out-of-domain or nonfinite inputs, exact rounding, or tangent at its poles.
We use Rust f32 operations (NaN for inverse trig outside [-1, 1]); these are
implementation choices, not behavior verified in Skyrim SE. No engine binaries
were examined. References are in refs/pex-natives/.
