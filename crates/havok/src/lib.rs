//! Havok packfile (`.hkx`) reading for Skyrim SE (hk_2010.2.0-r1, 64-bit pointers).
//!
//! Only the classes needed for character animation are interpreted:
//! `hkRootLevelContainer`, `hkaAnimationContainer`, `hkaSkeleton`,
//! `hkaAnimationBinding`, `hkaSplineCompressedAnimation` and
//! `hkaInterleavedUncompressedAnimation`, plus the generator tree of behaviour
//! graphs (see [`behavior`]).

mod anim;
pub mod behavior;
mod packfile;
mod spline;

pub use anim::{
    Animation, AnimationContainer, Annotation, Binding, QsTransform, Skeleton, SkeletonBone,
};
pub use packfile::{Object, Packfile};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a Havok packfile")]
    BadMagic,
    #[error("unsupported packfile layout: {0}")]
    Unsupported(String),
    #[error("corrupt packfile: {0}")]
    Corrupt(String),
}

pub type Result<T> = std::result::Result<T, Error>;
