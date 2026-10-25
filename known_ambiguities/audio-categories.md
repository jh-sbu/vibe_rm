# Sound categories

Implemented in `src/world/sound.rs` (`category_volume`, folded into
`SoundDesc::volume`), with the music and dialogue voice volumes taken
from the music and voice categories (`music_volume`, `voice_volume`).
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
  isn't stated anywhere public. The mixer's own master of 0.8 (headroom,
  made up) stays on top.
- **Music and voice have no descriptor.** Music tracks play at the music
  category's volume (`AudioCategoryMUS`, replacing a hard-coded 0.45) and
  dialogue lines at `AudioCategoryVOC`'s (`VOC`'s parent `VOCGeneral` is the
  menu's Voice). Which categories the engine really files them under isn't
  public.
