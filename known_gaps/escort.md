# Escort packages

Implemented in `src/ai/mod.rs` (`Behaviour::Escort`). Based on the Creation Kit's
Escort procedure template inputs as the data lists them and how the base game's six
Escort packages use them.

## What is known

- The `Escort` template's inputs (`vrm-tool esp-dump <data> Escort`): Destination,
  Target to Escort, Number of Followers (1), Distance to Wait for Follower(s) (512),
  Follower Min / Max Distance (120 / 256), Ride Horse?, PreferPreferredPath?,
  Run If Behind Distance (500), and Initial Location to find / Follower(s) /
  Follower(s) ObjectList.
- Users (`SHOW=Escort vrm-tool pack-templates <data>`): Illia in Darklight Tower
  (four stages leading the player), Telrav, Nirya (MQ205).

## Open questions

- "Run If Behind Distance": read here as the escort running when the escorted actor
  is that far *ahead* of it (nearer the destination), to catch up. It could instead
  apply to the followers (escorted NPCs) running to keep up with the escort.
- Facing: the escort turns in place to face the one it waits for once they are
  more than ~35° off to one side. The real threshold and whether the game does this
  at all (rather than only head tracking) is unverified.
- Follower Min / Max Distance, several followers and the follower object list aren't
  used; neither is Ride Horse? or PreferPreferredPath?.
