# Remaining animation inputs

Complete five bounded subitems of ROADMAP item 1's remaining generator and
transition-effect inputs. Each item receives its own implementation, review,
focused regression tests, ambiguity notes and commit. Implementation is delegated
to a Luna 6 High agent; the parent reviews and validates each result.

Use public community documentation/source (including the ignored local references)
and, where useful, installed game assets. Never inspect engine binary data or
leaked SDK material, and never commit game assets. Source-backed facts,
implementation policies and unresolved original-runtime behavior must be distinct.

1. **Transition blend curves.** Establish the authored enum and serialized layout;
   retain authored/variable/property inputs; select the cross-fade curve at commit.
   Test distinct curves, binding precedence, fallback and pending transitions.
2. **Transition target start fraction.** Establish the scalar member/layout and
   public semantics; apply supported destination start positioning at commit.
   Test target timing, bindings, cropping and unsupported-generator behavior.
3. **Transition end modes.** Establish authored enum/layout and public semantics;
   resolve supported source-end duration policies at commit. Test the supported
   modes against clip remaining time and document compound-generator limits.
4. **Clip enforced duration.** Establish member/layout and public semantics; use
   the supported duration in playback and synchronized timing consistently. Test
   speed/crop interaction, bindings and invalid-input fallback.
5. **Blender synchronization master.** Establish authored index/layout and public
   semantics; share master selection between direct and nested synchronized
   playback. Test explicit/automatic master choice, bindings and invalid indices.

If evidence cannot support an item, record its blocker in known_ambiguities and
choose another implementable roadmap subitem; do not count a blocker as completed.
Synthetic graphs verify our implementation, not Skyrim SE runtime parity.
