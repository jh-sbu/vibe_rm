# Sound categories

Implemented in `src/world/sound.rs` (static/category volume and DOBJ routing)
and `src/audio/director.rs` (dynamic category policy and menu settings).
Music and dialogue carry MDSC/DDSC category chains alongside descriptors.
Sources: UESP's [SNCT page](https://en.uesp.net/w/index.php?oldid=3508410)
(`FNAM` flags, `PNAM` parent, `VNAM` static volume multiplier, `UNAM`
default menu value, and the 16-bit float coding), the [SNDR
page](https://en.uesp.net/w/index.php?oldid=2309034) (`GNAM` names a sound's
category), and the base game's data (18 categories: `vrm-tool esp-list <data>
SNCT`, `esp-dump`).

## Settled from the data

- **The chain.** Every category but `_AudioCategoryMaster` has a parent;
  walking `PNAM` from a sound's `GNAM` always reaches the master. The
  multipliers (`VNAM`, 0xFFFF being 1.0: only 1.0, 0.9 and 0.7 occur) are
  taken to multiply down the chain.
- **Menu categories.** `FNAM` bit 2 ("Should Appear on Menu") is set on
  `AudioCategorySFX`, `FST`, `VOCGeneral` and `MUS`: the Audio menu's
  Effects, Footsteps, Voice and Music sliders. Their `UNAM` is the slider's
  default (1.0, 0.8, 0.8, 0.5).

## Open (choices made here)

- **Defaults multiply.** A sound's volume is its descriptor's static
  attenuation times each category's `VNAM` times, for a menu category, its
  `UNAM` default (the slider at its default setting), all the way up through
  the master category's own 0.9. The data gives SFX 0.81, footsteps 0.65,
  voice 0.65, music 0.32 (MUS 0.7 x 0.5 x master 0.9). That the sliders'
  defaults are the `UNAM` values and multiply into the sound's volume
  isn't stated anywhere public. A separate 0.8 headroom factor stays on top of the user's master slider.
  The headroom is a mixer choice, not an AudioMenu setting.
- **Descriptor-free categories are now sourced.** DOBJ DDSC supplies dialogue
  (including INFO barks and scene responses), MDSC music. Barks play INFO voice
  files, so they remain dialogue, not NDSC grunts. No descriptor-free non-dialogue
  vocal path exists yet; NDSC is available when one does. See research/audio/defaults.md.

## Dialogue fade

DOBJ SFDC identifies the affected subtree, leaving UI sounds outside it.
Neither public SNCT/DOBJ documentation nor the records specify fade gain or
timing. We choose -6 dB, 3 seconds out and 1 second in (the menu fade times).
Only a player conversation counts, not scenes or INFO barks. Listening in SE
could refine this choice. Script state multiplies with this policy.

## Settings (8)

Stored slider values replace UNAM, including a zero default. Descriptor and
music/voice gains now carry only VNAM multipliers; the director applies the
current menu gain to all existing/new descendants. Master preferences remain
separate from 0.8 mixer headroom. Layout follows SkyrimPrefs AudioMenu, while
storage belongs to this engine (not Skyrim's INI or save). Invalid/nonfinite
values are rejected. Config is read at startup; explicit API/console changes
write it atomically. Environment overrides are effective startup values.
