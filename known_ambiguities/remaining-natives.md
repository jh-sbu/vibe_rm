# Further PEX natives

These implementations use public community documentation and existing runtime
systems. They have not been compared experimentally with Skyrim SE. References
and source-versus-inference distinctions are in refs/pex-natives/batch*.md.

## Batch 1

Alias game timers inherit our Form registration policy: one timer per object and
event, replacing previous registrations. Exact multiple-script registration
interaction and zero/negative/NaN intervals remain unverified. Ingredient
hostility considers all its effects; whether only learned effects matter is not
specified by the terse public native description. Missing string settings return
an empty string; original diagnostic text is unverified.
