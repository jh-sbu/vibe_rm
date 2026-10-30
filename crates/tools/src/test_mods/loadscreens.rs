//! `test-mod loadscreens`: loading screens (`LSCR`) whose text says which
//! condition put them up, to settle how the game picks one
//! (`plans/loading-screen.md`, step 0; `research/ui/loading-screen.md`). No
//! scripts: the user reads the first text each loading screen shows (a new
//! one comes every 15.5 seconds, which may be picked differently).
//!
//! | Screen | Conditions | Text starts |
//! | --- | --- | --- |
//! | `VrmLSInn` | GetInCurrentLoc(RiverwoodSleepingGiantInnLocation) = 1 | "VRM INN" |
//! | `VrmLSTown` | GetInCurrentLoc(RiverwoodLocation) = 1 AND GetInCurrentLoc(inn) = 0 | "VRM TOWN" |
//! | `VrmLSAlways` | GetRandomPercent >= 0 (always passes) | "VRM ALWAYS" |
//! | `VrmLSMenu` | none; "Displays In Main Menu" flag | "VRM MENU" |
//!
//! Each screen uses the vanilla `LoadScreenLogo` model, so the model doesn't
//! tell them apart.
//!
//! What to do: go through the Sleeping Giant Inn's front door into the inn
//! and back out, five times each way; then go through some door far from
//! Riverwood five times; load a save made outside Riverwood once in game and
//! once from the main menu. Note the first text each time.
//!
//! | Reading | Into the inn | Out of the inn | Elsewhere |
//! | --- | --- | --- | --- |
//! | GetInCurrentLoc sees the destination, passing screens always win | INN or ALWAYS | TOWN or ALWAYS | ALWAYS |
//! | ... sees the departure, passing screens always win | TOWN or ALWAYS | INN or ALWAYS | ALWAYS |
//! | passing screens are only more likely | sometimes a vanilla tip | sometimes a vanilla tip | mostly vanilla tips |
//!
//! MENU appearing on the main-menu load (and not, or less often, in game)
//! says the flag restricts to or prefers main-menu loads. MENU in game says
//! unconditioned screens stay eligible.

use anyhow::{Context, Result, ensure};
use esp::writer::{Node, RecordBuilder, write_plugin};
use esp::{FormId, LoadOrder};

use super::own;
use super::records::{ctda, op, run_on};

const GET_RANDOM_PERCENT: u16 = 77;
const GET_IN_CURRENT_LOC: u16 = 359;
/// `LSCR` record flag 10: "Displays In Main Menu" (xEdit).
const DISPLAYS_IN_MAIN_MENU: u32 = 1 << 10;

const INN: u32 = 0x800;
const TOWN: u32 = 0x801;
const ALWAYS: u32 = 0x802;
const MENU: u32 = 0x803;
const NEXT_OBJECT_ID: u32 = 0x804;

/// A loading screen as the Creation Kit writes one (conditions after
/// `DESC`): the logo, unscaled, unturned, free to turn, on the most common
/// camera path.
fn screen(
    object_id: u32,
    edid: &str,
    text: &str,
    conditions: &[[u8; 32]],
    logo: FormId,
) -> RecordBuilder {
    let rnam: Vec<u8> = [0i16; 3].iter().flat_map(|v| v.to_le_bytes()).collect();
    let onam: Vec<u8> = [-180i16, 180]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let xnam: Vec<u8> = [0f32; 3].iter().flat_map(|v| v.to_le_bytes()).collect();
    conditions
        .iter()
        .fold(
            RecordBuilder::new(b"LSCR", own(object_id))
                .zstring(b"EDID", edid)
                .zstring(b"DESC", text),
            |rec, c| rec.field(b"CTDA", c),
        )
        .field(b"NNAM", &logo.0.to_le_bytes())
        .field(b"SNAM", &1.0f32.to_le_bytes())
        .field(b"RNAM", &rnam)
        .field(b"ONAM", &onam)
        .field(b"XNAM", &xnam)
        .zstring(b"MOD2", "Cameras\\LSCameraPanZoomInSmall.nif")
}

pub fn build(lo: &LoadOrder) -> Result<Vec<u8>> {
    let id = |edid: &str| -> Result<FormId> {
        let id = lo
            .find_editor_id(edid)
            .with_context(|| format!("{edid} not found"))?;
        ensure!(id.0 >> 24 == 0, "{edid} ({id}) isn't from Skyrim.esm");
        Ok(id)
    };
    let logo = id("LoadScreenLogo")?;
    let inn = id("RiverwoodSleepingGiantInnLocation")?;
    let town = id("RiverwoodLocation")?;
    let in_loc = |loc: FormId, value: f32| {
        ctda(GET_IN_CURRENT_LOC, loc.0, 0, op::EQ, value, run_on::SUBJECT)
    };
    let screens = vec![
        screen(
            INN,
            "VrmLSInn",
            "VRM INN: GetInCurrentLoc(RiverwoodSleepingGiantInnLocation) passed.",
            &[in_loc(inn, 1.0)],
            logo,
        ),
        screen(
            TOWN,
            "VrmLSTown",
            "VRM TOWN: in RiverwoodLocation, not in the inn.",
            &[in_loc(town, 1.0), in_loc(inn, 0.0)],
            logo,
        ),
        screen(
            ALWAYS,
            "VrmLSAlways",
            "VRM ALWAYS: GetRandomPercent >= 0, a condition that always passes.",
            &[ctda(GET_RANDOM_PERCENT, 0, 0, op::GE, 0.0, run_on::SUBJECT)],
            logo,
        ),
        screen(
            MENU,
            "VrmLSMenu",
            "VRM MENU: no conditions, Displays In Main Menu.",
            &[],
            logo,
        )
        .flags(DISPLAYS_IN_MAIN_MENU),
    ];
    Ok(write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "Loading screens naming the condition that showed them.",
        NEXT_OBJECT_ID,
        &[Node::top(b"LSCR", screens)],
    ))
}
