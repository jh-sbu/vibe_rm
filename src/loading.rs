//! Loads behind a loading screen: every cell change the player makes through
//! a load door, fast travel, an arrest or a script move, and every native
//! save load, is one [`LoadRequest`]. Callers check what they refuse first
//! and then queue the request; the engine carries it out at the start of
//! the next frame (`plans/loading-screen.md`).

use anyhow::Result;
use esp::FormId;
use glam::Vec3;

use crate::engine::Engine;

/// Where a load takes the player.
#[derive(Debug, Clone, PartialEq)]
pub enum LoadTarget {
    /// An interior cell, at a position and heading (none: the cell's own
    /// entry point).
    Interior {
        cell: FormId,
        spawn: Option<(Vec3, f32)>,
    },
    /// A worldspace, the player's feet at `feet`, facing `yaw`.
    Exterior { world: FormId, feet: Vec3, yaw: f32 },
    /// A save game, by name.
    Save(String),
}

/// What asked for a load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadCause {
    Door,
    FastTravel,
    Script,
    Arrest,
    SaveLoad,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadRequest {
    pub target: LoadTarget,
    pub cause: LoadCause,
}

impl LoadRequest {
    pub fn new(target: LoadTarget, cause: LoadCause) -> Self {
        LoadRequest { target, cause }
    }

    pub fn save(name: String) -> Self {
        LoadRequest::new(LoadTarget::Save(name), LoadCause::SaveLoad)
    }
}

impl Engine {
    /// Queue a load for the next frame. A load already queued stays: the
    /// first request in a frame wins.
    pub fn request_load(&mut self, request: LoadRequest) {
        match &self.pending_load {
            Some(queued) => log::warn!("load {request:?} dropped: {queued:?} is queued"),
            None => self.pending_load = Some(request),
        }
    }

    /// Carry out the queued load, if any.
    pub fn apply_pending_load(&mut self) {
        if let Some(request) = self.pending_load.take() {
            self.run_load(request);
        }
    }

    /// The load itself, and what each cause does when it fails.
    fn run_load(&mut self, request: LoadRequest) {
        let result = match &request.target {
            LoadTarget::Save(name) => {
                self.finish_save_load(name.clone());
                return;
            }
            target => self.enter(target),
        };
        if let Err(error) = result {
            log::warn!("{:?} load failed: {error:#}", request.cause);
            if request.cause == LoadCause::FastTravel {
                let text = crate::fast_travel::Refusal::InvalidDestination.message(self);
                self.scripts.notify(text);
            }
        }
    }

    /// Enter a cell target (not a save).
    pub(crate) fn enter(&mut self, target: &LoadTarget) -> Result<()> {
        match *target {
            LoadTarget::Interior { cell, spawn } => self.enter_interior(cell, spawn),
            LoadTarget::Exterior { world, feet, yaw } => self.enter_exterior(world, feet, yaw),
            LoadTarget::Save(_) => unreachable!("saves load through finish_save_load"),
        }
    }

    /// The target a door's destination (`XTEL`) leads to.
    pub fn door_target(&self, dest_door: FormId, pos: Vec3, yaw: f32) -> Result<LoadTarget> {
        let cell = anyhow::Context::context(
            self.lo.cell_of_ref(dest_door),
            "destination door has no cell",
        )?;
        let idx = self.lo.cell(cell).cloned().unwrap_or_default();
        Ok(match idx.world {
            Some(world) => LoadTarget::Exterior {
                world,
                feet: pos,
                yaw,
            },
            None => LoadTarget::Interior {
                cell,
                spawn: Some((pos, yaw)),
            },
        })
    }
}
