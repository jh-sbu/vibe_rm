//! The skills menu as the game shows it (`statsmenu.swf`, with
//! `levelupmenu.swf` over it while a level up is waiting): the ring of
//! skills, a skill's perk tree, the player's card and the bottom bar's meters.
//!
//! The movie only draws. The game keeps the selection, turns the ring
//! (`SetDirection`, an angle in degrees, 20 to a skill), fills its static
//! `StatsMenu.SkillStatsA` (level, name, progress percent, level colour and
//! legendary count of each skill) for `UpdateSkillList`, the description card
//! (`SetDescriptionCard`), the player's card (`SetPlayerInfo`) and the perks'
//! labels (`StatsMenu.PerkNamesA`: an HTML name and the screen fractions it
//! sits at, for `UpdatePerkText`, which fades them in a frame a call). The
//! movie's only calls are `MoveCamera` (the game's 3D camera) and
//! `SetFadedIn`. Choices: `known_ambiguities/skills-menu.md`.

use esp::FormId;
use esp::actor_value as av;
use ui_swf::{GameCall, Menu, UiSwf, Value};
use winit::keyboard::KeyCode;

use crate::engine::{Engine, PLAYER_REF};
use crate::perks::TreeNode;

use super::{Buttons, num, text};

const M: &str = "_root.StatsMenuBaseInstance";
const LEVEL_UP: &str = "_root.LevelUpMenu_mc";
const STATIC: &str = "StatsMenu";

/// The ring's skills in the order the movie's constants give them: the three
/// constellations' groups, magic, stealth, then combat (the movie's
/// `Mysticism` is Illusion).
const RING: [u32; 18] = [
    18, // Alteration
    19, // Conjuration
    20, // Destruction
    21, // Illusion
    22, // Restoration
    av::ENCHANTING,
    av::LIGHT_ARMOR,
    av::PICKPOCKET,
    av::LOCKPICKING,
    av::SNEAK,
    16, // Alchemy
    av::SPEECHCRAFT,
    av::ONE_HANDED,
    av::TWO_HANDED,
    av::MARKSMAN,
    av::BLOCK,
    10, // Smithing
    av::HEAVY_ARMOR,
];
/// Degrees a skill takes on the ring (`AnimatedSkillText.SKILL_ANGLE`).
const SKILL_ANGLE: f32 = 20.0;
/// Degrees a second the ring turns.
const TURN_SPEED: f32 = 520.0;
/// The movie's frame rate, which the perk labels' fade steps at.
const FPS: f32 = 24.0;
/// The root timeline's frame the fade out ends on (`FadedOut`).
const FADED_OUT: f64 = 16.0;
/// Where perk trees sit on the screen, as fractions of its width and height
/// (left, top, right, bottom), when the 3D constellations don't place them.
const TREE_AREA: [f32; 4] = [0.1, 0.2, 0.9, 0.6];
/// How near a click must be to a perk to pick it, screen heights.
const REACH: f32 = 0.06;

/// What the player did, for the next update (the keys have no engine).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Act {
    Left,
    Right,
    Up,
    Down,
    Accept,
    Back,
    Legendary,
    /// A click, as fractions of the screen.
    Click(f32, f32),
    Wheel(f32),
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// Turning the ring.
    Ring,
    /// A skill's tree, with the node selected (its `TreeNode::index`).
    Perks { skill: u32, node: Option<u32> },
}

/// What the movie was last told, to tell it again only when it changes.
#[derive(Default, PartialEq)]
struct Shown {
    player: Vec<String>,
    skills: Vec<String>,
    card: Vec<String>,
    labels: Vec<String>,
    points: Option<(bool, u32)>,
}

pub(super) struct StatsMenu {
    pub menu: Menu,
    /// The level up popup, while a level up is waiting.
    pub level_up: Option<Menu>,
    mode: Mode,
    /// The ring's selected skill (an index of `RING`) and where it is turned
    /// to now, degrees.
    selected: usize,
    angle: f32,
    /// A level was taken here, so the menu is the game's level up mode.
    leveled: bool,
    acts: Vec<Act>,
    shown: Shown,
    /// Seconds towards the next frame of the labels' fade.
    fade: f32,
    /// Closing: it fades out, then the engine's menu closes.
    pub closing: bool,
}

/// A skill's progress to its next level, in percent.
fn progress(engine: &Engine, skill: u32) -> f32 {
    let need = engine.skill_xp_needed(skill);
    if need <= 0.0 {
        return 0.0;
    }
    let xp = engine.skills.xp[(skill - av::FIRST_SKILL) as usize];
    (xp / need * 100.0).clamp(0.0, 100.0)
}

/// A skill's `AVIF` description.
fn skill_description(engine: &Engine, skill: u32) -> String {
    engine
        .lo
        .get(FormId(0x446 + skill))
        .and_then(|r| r.get(b"DESC").map(|d| engine.lo.lstring(&r, d)))
        .unwrap_or_default()
}

fn setting(engine: &Engine, name: &str, english: &str) -> String {
    engine
        .gmst_string(name)
        .unwrap_or_else(|| english.to_string())
}

impl StatsMenu {
    pub fn open(
        ui: &UiSwf,
        size: (u32, u32),
        engine: &mut Engine,
        buttons: std::sync::Arc<Buttons>,
    ) -> anyhow::Result<Self> {
        let menu = ui.open("statsmenu.swf", size)?;
        menu.set_responder(move |call, _| super::respond(&buttons, call));
        menu.invoke(&format!("{M}.InitExtensions"), &[]);
        menu.invoke("_root.gotoAndPlay", &[text("FadeIn")]);
        menu.callback("ShowLegendaryButtonHint", &[]);
        let mut level_up = None;
        if engine.skills.pending > 0 {
            level_up = Some(open_level_up(ui, size)?);
        }
        let mode = match engine.perks.tree {
            Some(skill) if RING.contains(&skill) => Mode::Perks { skill, node: None },
            _ => Mode::Ring,
        };
        let selected = match mode {
            Mode::Perks { skill, .. } => RING.iter().position(|&s| s == skill).unwrap_or(0),
            Mode::Ring => 0,
        };
        let mut m = StatsMenu {
            menu,
            level_up,
            mode,
            selected,
            angle: selected as f32 * SKILL_ANGLE,
            leveled: false,
            acts: Vec::new(),
            shown: Shown::default(),
            fade: 0.0,
            closing: false,
        };
        engine.perks.tree = None;
        m.update(engine, 0.0);
        Ok(m)
    }

    /// The popup is on top of the menu: it takes the input.
    pub fn top(&self) -> &Menu {
        self.level_up.as_ref().unwrap_or(&self.menu)
    }

    /// The perk tree open: the key to leave it is Back, not close.
    pub fn act(&mut self, act: Act) {
        if !self.closing {
            self.acts.push(act);
        }
    }

    /// The menu's own keys: the ring turns with the arrows (and A / D),
    /// Accept (E, Enter) opens a skill's perks or takes a perk, Tab and Esc
    /// go back or leave, Ready Weapon (R) makes the skill legendary. True for
    /// keys it took.
    pub fn key(&mut self, code: KeyCode) -> bool {
        if self.level_up.is_some() {
            // The popup's own buttons take them (Esc waits on its answer).
            return code == KeyCode::Escape || code == KeyCode::Tab;
        }
        let act = match code {
            KeyCode::ArrowLeft | KeyCode::KeyA => Act::Left,
            KeyCode::ArrowRight | KeyCode::KeyD => Act::Right,
            KeyCode::ArrowUp | KeyCode::KeyW => Act::Up,
            KeyCode::ArrowDown | KeyCode::KeyS => Act::Down,
            KeyCode::KeyE | KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space => Act::Accept,
            KeyCode::Escape | KeyCode::Tab | KeyCode::KeyK => Act::Back,
            KeyCode::KeyR => Act::Legendary,
            _ => return false,
        };
        self.act(act);
        true
    }

    /// The selected ring skill.
    fn skill(&self) -> u32 {
        RING[self.selected]
    }

    /// Where a tree's nodes sit on the screen, as fractions of it: the grid
    /// fitted to `TREE_AREA`, its y growing up.
    fn tree_points(nodes: &[TreeNode]) -> Vec<(f32, f32)> {
        let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for n in nodes {
            (x0, x1, y0, y1) = (x0.min(n.x), x1.max(n.x), y0.min(n.y), y1.max(n.y));
        }
        let [left, top, right, bottom] = TREE_AREA;
        let fit = |v: f32, lo: f32, hi: f32, a: f32, b: f32| {
            if hi - lo < 1e-3 {
                (a + b) / 2.0
            } else {
                a + (v - lo) / (hi - lo) * (b - a)
            }
        };
        nodes
            .iter()
            .map(|n| (fit(n.x, x0, x1, left, right), fit(n.y, y0, y1, bottom, top)))
            .collect()
    }

    /// The node `dx`, `dy` (screen directions) from `from`: the nearest within
    /// a quarter turn of that way.
    fn neighbour(points: &[(f32, f32)], from: usize, dx: f32, dy: f32) -> Option<usize> {
        let (fx, fy) = points[from];
        points
            .iter()
            .enumerate()
            .filter(|&(i, _)| i != from)
            .filter_map(|(i, &(x, y))| {
                let (vx, vy) = (x - fx, y - fy);
                let along = vx * dx + vy * dy;
                let across = (vx * dy - vy * dx).abs();
                (along > 0.0 && across <= along * 1.4)
                    .then_some((i, along * along + across * across * 2.0))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    /// What the player did, then the movie's state.
    pub fn update(&mut self, engine: &mut Engine, dt: f32) {
        // A level up: the popup's buttons call back with the attribute.
        if let Some(popup) = &self.level_up {
            // One press can reach the button twice (the key and the press it
            // makes): a level a batch.
            let mut taken = false;
            for call in popup.take_calls() {
                let attribute = match call.name.as_str() {
                    "addHealth" => av::HEALTH,
                    "addMagicka" => av::MAGICKA,
                    "addStamina" => av::STAMINA,
                    _ => {
                        super::answer(popup, engine, &call);
                        continue;
                    }
                };
                if !taken && engine.take_level_up(attribute) {
                    self.leveled = true;
                    taken = true;
                    let at = engine.camera.position;
                    engine.play_sound_at("UILevelUpSD", at);
                }
            }
            if engine.skills.pending == 0 {
                self.level_up = None;
            }
        }
        let acts = std::mem::take(&mut self.acts);
        for act in acts {
            self.apply(engine, act);
        }
        self.turn(engine, dt);
        self.sync(engine);
        self.labels(dt);
        self.closing_done(engine);
    }

    fn apply(&mut self, engine: &mut Engine, act: Act) {
        match (self.mode, act) {
            (Mode::Ring, Act::Left) => {
                self.selected = (self.selected + RING.len() - 1) % RING.len();
                self.sound(engine, "UISkillsBackwardSD");
            }
            (Mode::Ring, Act::Right) => {
                self.selected = (self.selected + 1) % RING.len();
                self.sound(engine, "UISkillsForwardSD");
            }
            (Mode::Ring, Act::Wheel(w)) => {
                let n = RING.len();
                self.selected = if w > 0.0 {
                    (self.selected + n - 1) % n
                } else {
                    (self.selected + 1) % n
                };
                self.sound(
                    engine,
                    if w > 0.0 {
                        "UISkillsBackwardSD"
                    } else {
                        "UISkillsForwardSD"
                    },
                );
            }
            (Mode::Ring, Act::Accept) => self.enter(engine),
            (Mode::Ring, Act::Click(x, _)) => {
                if (0.4..0.6).contains(&x) {
                    self.enter(engine);
                } else {
                    let n = RING.len();
                    self.selected = if x < 0.5 {
                        (self.selected + n - 1) % n
                    } else {
                        (self.selected + 1) % n
                    };
                    self.sound(
                        engine,
                        if x < 0.5 {
                            "UISkillsBackwardSD"
                        } else {
                            "UISkillsForwardSD"
                        },
                    );
                }
            }
            (Mode::Ring, Act::Back) => self.close(),
            (Mode::Perks { .. }, Act::Back) => {
                self.mode = Mode::Ring;
                self.shown.labels.clear();
                self.menu
                    .invoke(&format!("{M}.UpdatePerkText"), &[Value::Bool(false)]);
                self.sound(engine, "UIMenuCancelSD");
            }
            (Mode::Perks { skill, node }, act) => self.perks_act(engine, skill, node, act),
            _ => {}
        }
    }

    fn perks_act(&mut self, engine: &mut Engine, skill: u32, node: Option<u32>, act: Act) {
        let nodes = engine.perk_tree(skill);
        if nodes.is_empty() {
            return;
        }
        let points = Self::tree_points(&nodes);
        let at = node.and_then(|n| nodes.iter().position(|t| t.index == n));
        let pick = |this: &mut Self, i: usize| {
            this.mode = Mode::Perks {
                skill,
                node: Some(nodes[i].index),
            };
        };
        match act {
            Act::Left | Act::Right | Act::Up | Act::Down => {
                let (dx, dy) = match act {
                    Act::Left => (-1.0, 0.0),
                    Act::Right => (1.0, 0.0),
                    Act::Up => (0.0, -1.0),
                    _ => (0.0, 1.0),
                };
                let next = match at {
                    // The first press picks the lowest node (a tree's start).
                    None => (0..nodes.len()).max_by(|&a, &b| points[a].1.total_cmp(&points[b].1)),
                    Some(i) => Self::neighbour(&points, i, dx, dy),
                };
                if let Some(i) = next {
                    pick(self, i);
                    self.sound(engine, "UISkillsFocusSD");
                }
            }
            Act::Accept => {
                let Some(i) = at else {
                    // Nothing picked yet: pick the lowest node.
                    if let Some(i) =
                        (0..nodes.len()).max_by(|&a, &b| points[a].1.total_cmp(&points[b].1))
                    {
                        pick(self, i);
                    }
                    return;
                };
                if let Some((perk, true)) = engine.next_perk_rank(nodes[i].perk)
                    && engine.buy_perk(perk)
                {
                    self.sound(engine, "UISkillsPerkSelectASD");
                    self.shown.labels.clear();
                } else {
                    self.sound(engine, "UIMenuPrevNextSD");
                }
            }
            Act::Click(x, y) => {
                let aspect = 16.0 / 9.0;
                let near = points
                    .iter()
                    .enumerate()
                    .map(|(i, &(px, py))| {
                        let (dx, dy) = ((px - x) * aspect, py - y);
                        (i, (dx * dx + dy * dy).sqrt())
                    })
                    .filter(|&(_, d)| d < REACH)
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                if let Some((i, _)) = near {
                    if at == Some(i) {
                        self.perks_act(engine, skill, node, Act::Accept);
                    } else {
                        pick(self, i);
                        self.sound(engine, "UISkillsFocusSD");
                    }
                }
            }
            Act::Legendary => {
                if crate::skills::can_make_legendary(skill, engine.av_base(PLAYER_REF, skill)) {
                    let text = setting(
                        engine,
                        "sLegendaryResetConfirm",
                        "Making a skill Legendary will set it to 15 and return its perks. Are you sure? ",
                    );
                    engine.ask_legendary(skill, text, false);
                }
            }
            Act::Wheel(_) | Act::Back => {}
        }
    }

    /// Opens the selected skill's perks.
    fn enter(&mut self, engine: &mut Engine) {
        let skill = self.skill();
        self.mode = Mode::Perks { skill, node: None };
        self.shown.labels.clear();
        self.fade = 0.0;
        self.menu.invoke(&format!("{M}.HideRing"), &[]);
        self.sound(engine, "UISkillsTransitionUp");
    }

    fn sound(&self, engine: &mut Engine, sound: &str) {
        let at = engine.camera.position;
        engine.play_sound_at(sound, at);
    }

    /// Starts fading out.
    pub fn close(&mut self) {
        if self.closing {
            return;
        }
        self.closing = true;
        self.menu.invoke("_root.gotoAndPlay", &[text("FadeOut")]);
    }

    /// The fade out done: the engine's menu closes.
    fn closing_done(&mut self, engine: &mut Engine) {
        if !self.closing {
            return;
        }
        if matches!(
            self.menu.get("_root._currentframe"),
            Value::Number(f) if f == FADED_OUT
        ) && engine.menu == Some(crate::items::Menu::Skills)
        {
            engine.menu = None;
        }
    }

    /// The ring turns towards the selected skill by the shortest way.
    fn turn(&mut self, engine: &mut Engine, dt: f32) {
        let ring = RING.len() as f32 * SKILL_ANGLE;
        let target = self.selected as f32 * SKILL_ANGLE;
        let mut delta = (target - self.angle).rem_euclid(ring);
        if delta > ring / 2.0 {
            delta -= ring;
        }
        let step = TURN_SPEED * dt;
        self.angle = if delta.abs() <= step || dt == 0.0 {
            target
        } else {
            (self.angle + step.copysign(delta)).rem_euclid(ring)
        };
        if self.mode == Mode::Ring {
            self.menu
                .invoke(&format!("{M}.SetDirection"), &[num(self.angle)]);
        }
        if let Some(view) = &mut engine.stats_view {
            view.set_angle(self.angle);
            view.set_perks(matches!(self.mode, Mode::Perks { .. }));
        }
    }

    /// Tells the movie what changed.
    fn sync(&mut self, engine: &mut Engine) {
        let (ring_skill, mode, leveled) = (self.skill(), self.mode, self.leveled);
        let shown = &mut self.shown;
        let m = &self.menu;
        // The player's card.
        let name = engine
            .actor_name(PLAYER_REF)
            .unwrap_or_else(|| "Player".into());
        let race = engine
            .race_of(PLAYER_REF)
            .map(|r| engine.form_name(r))
            .unwrap_or_default();
        let level = engine.skills.level;
        let meter = |a: u32| {
            [
                format!("{}", engine.actor_value(PLAYER_REF, a).round() as i32),
                format!("{}", engine.av_max(PLAYER_REF, a).round() as i32),
            ]
        };
        let (mag, hp, sta) = (meter(av::MAGICKA), meter(av::HEALTH), meter(av::STAMINA));
        let percent = engine.level_progress() * 100.0;
        let player = vec![
            name.clone(),
            level.to_string(),
            format!("{percent:.1}"),
            race.clone(),
            mag.join("/"),
            hp.join("/"),
            sta.join("/"),
        ];
        if player != shown.player {
            let colour = text("#FFFFFF");
            let mut args = vec![text(&name), num(level as f32), num(percent), text(&race)];
            for [cur, max] in [&mag, &hp, &sta] {
                args.extend([
                    num(cur.parse().unwrap_or(0.0)),
                    num(max.parse().unwrap_or(0.0)),
                    colour.clone(),
                ]);
            }
            m.callback("SetPlayerInfo", &args);
            shown.player = player;
        }
        // The ring's labels.
        let rows: Vec<[String; 5]> = RING
            .iter()
            .map(|&s| {
                [
                    (engine.av_base(PLAYER_REF, s).round() as i32).to_string(),
                    engine.skill_name(s),
                    format!("{:.1}", progress(engine, s)),
                    "#FFFFFF".into(),
                    engine.skills.legendary[(s - av::FIRST_SKILL) as usize].to_string(),
                ]
            })
            .collect();
        let flat: Vec<String> = rows.iter().flatten().cloned().collect();
        if flat != shown.skills {
            let list = rows
                .iter()
                .flat_map(|r| {
                    [
                        num(r[0].parse().unwrap_or(0.0)),
                        text(&r[1]),
                        num(r[2].parse().unwrap_or(0.0)),
                        text(&r[3]),
                        num(r[4].parse().unwrap_or(0.0)),
                    ]
                })
                .collect();
            m.set(&format!("{STATIC}.SkillStatsA"), Value::List(list));
            m.callback("UpdateSkillList", &[Value::Bool(true)]);
            shown.skills = flat;
        }
        // The perk points and mode.
        let points = Some((!leveled, engine.skills.perk_points));
        if points != shown.points {
            let (stats, n) = points.unwrap();
            m.callback("SetStatsMode", &[Value::Bool(stats), num(n as f32)]);
            shown.points = points;
        }
        // The card and, in a tree, the labels.
        match mode {
            Mode::Ring => {
                let skill = ring_skill;
                let level = engine.av_base(PLAYER_REF, skill).round() as i32;
                let card = vec![
                    "ring".into(),
                    skill.to_string(),
                    level.to_string(),
                    format!("{:.1}", progress(engine, skill)),
                    engine.skills.legendary[(skill - av::FIRST_SKILL) as usize].to_string(),
                ];
                if card != shown.card {
                    m.callback(
                        "SetDescriptionCard",
                        &[
                            Value::Bool(false),
                            text(&engine.skill_name(skill)),
                            num(progress(engine, skill)),
                            text(&skill_description(engine, skill)),
                            text(""),
                            num(level as f32),
                            text(&engine.skill_name(skill)),
                            num(card[4].parse().unwrap_or(0.0)),
                        ],
                    );
                    shown.card = card;
                }
            }
            Mode::Perks { skill, node } => self.perks_card(engine, skill, node),
        }
    }

    /// A skill's tree: the labels at their nodes, and the card of the node
    /// picked.
    fn perks_card(&mut self, engine: &mut Engine, skill: u32, node: Option<u32>) {
        let nodes = engine.perk_tree(skill);
        let points = Self::tree_points(&nodes);
        let selected = node.and_then(|n| nodes.iter().position(|t| t.index == n));
        let mut labels = Vec::new();
        let mut list = Vec::new();
        for (i, (n, &(x, y))) in nodes.iter().zip(&points).enumerate() {
            let ranks = engine.perk_ranks(n.perk);
            let owned = ranks
                .iter()
                .filter(|&&p| engine.has_perk(PLAYER_REF, p))
                .count();
            let available = engine.next_perk_rank(n.perk).is_some_and(|x| x.1);
            let name = engine
                .perk(n.perk)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            let colour = if Some(i) == selected {
                "#FFFFFF"
            } else if owned == ranks.len() {
                "#E8C766"
            } else if owned > 0 {
                "#C8AA6E"
            } else if available {
                "#B0B0B0"
            } else {
                "#606060"
            };
            let label = if ranks.len() > 1 && owned > 0 {
                format!("{name} {owned}/{}", ranks.len())
            } else {
                name
            };
            let html = format!("<font color='{colour}'>{}</font>", super::html(&label));
            labels.push(format!("{html}|{x:.3}|{y:.3}"));
            list.extend([text(&html), num(x), num(y)]);
        }
        if labels != self.shown.labels {
            self.menu
                .set(&format!("{STATIC}.PerkNamesA"), Value::List(list));
            self.shown.labels = labels;
            self.menu
                .invoke(&format!("{M}.UpdatePerkText"), &[Value::Bool(true)]);
        }
        let level = engine.av_base(PLAYER_REF, skill).round() as i32;
        let legendary = engine.skills.legendary[(skill - av::FIRST_SKILL) as usize];
        let card = match selected {
            Some(i) => {
                let n = &nodes[i];
                let next = engine.next_perk_rank(n.perk);
                let ranks = engine.perk_ranks(n.perk);
                // The next rank's text, or the last's once all are taken.
                let showing = next.map_or(*ranks.last().unwrap(), |x| x.0);
                let perk = engine.perk(showing);
                let name = engine
                    .perk(n.perk)
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                let desc = perk
                    .as_ref()
                    .map(|p| p.description.clone())
                    .unwrap_or_default();
                let needs = match next {
                    Some((p, false)) => self.requirements(engine, p),
                    _ => String::new(),
                };
                vec![name, desc, needs]
            }
            None => vec![String::new(), String::new(), String::new()],
        };
        let key = vec![
            "perks".into(),
            skill.to_string(),
            card.join("|"),
            level.to_string(),
            legendary.to_string(),
        ];
        if key != self.shown.card {
            self.menu.callback(
                "SetDescriptionCard",
                &[
                    Value::Bool(true),
                    text(&card[0]),
                    num(0.0),
                    text(&card[1]),
                    text(&card[2]),
                    num(level as f32),
                    text(&engine.skill_name(skill)),
                    num(legendary as f32),
                ],
            );
            self.shown.card = key;
        }
    }

    /// What a perk asks that the player lacks, from its conditions: skill
    /// levels and perks, as "Requires Alteration 30".
    fn requirements(&self, engine: &Engine, perk: FormId) -> String {
        let Some(p) = engine.perk(perk) else {
            return String::new();
        };
        let mut parts = Vec::new();
        for c in &p.conditions {
            match c.func {
                // GetActorValue, GetBaseActorValue
                14 | 277 if (av::FIRST_SKILL..=av::LAST_SKILL).contains(&c.p1) => {
                    parts.push(format!("{} {}", engine.skill_name(c.p1), c.value as i32));
                }
                // HasPerk
                448 => {
                    let wanted = FormId(c.p1);
                    if !engine.has_perk(PLAYER_REF, wanted)
                        && let Some(q) = engine.perk(wanted)
                    {
                        parts.push(q.name.clone());
                    }
                }
                _ => {}
            }
        }
        if parts.is_empty() {
            return String::new();
        }
        format!(
            "{} {}",
            setting(engine, "sRequirementsText", "Requires"),
            parts.join(", ")
        )
    }

    /// The labels fade in a frame a call at the movie's rate.
    fn labels(&mut self, dt: f32) {
        if !matches!(self.mode, Mode::Perks { .. }) {
            return;
        }
        self.fade += dt;
        while self.fade >= 1.0 / FPS {
            self.fade -= 1.0 / FPS;
            self.menu
                .invoke(&format!("{M}.UpdatePerkText"), &[Value::Bool(true)]);
        }
    }

    /// The movie's calls this answers; false for those it leaves to the
    /// menus' shared ones.
    pub fn answer(&mut self, _engine: &mut Engine, call: &GameCall) -> bool {
        // `MoveCamera` is the 3D camera's; `SetFadedIn` the fade's end.
        matches!(call.name.as_str(), "MoveCamera" | "SetFadedIn" | "myLog")
    }
}

/// The level up popup.
fn open_level_up(ui: &UiSwf, size: (u32, u32)) -> anyhow::Result<Menu> {
    let menu = ui.open("levelupmenu.swf", size)?;
    menu.invoke(&format!("{LEVEL_UP}.InitExtensions"), &[]);
    // The first button is focused, so the arrows and Accept work.
    menu.invoke(
        "Selection.setFocus",
        &[text(&format!("{LEVEL_UP}.MagickaButton"))],
    );
    Ok(menu)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ring_holds_each_skill_once() {
        let mut ring = RING.to_vec();
        ring.sort_unstable();
        assert_eq!(ring, (av::FIRST_SKILL..=av::LAST_SKILL).collect::<Vec<_>>());
    }

    fn node(index: u32, x: f32, y: f32) -> TreeNode {
        TreeNode {
            index,
            perk: FormId(index + 1),
            x,
            y,
            children: Vec::new(),
        }
    }

    #[test]
    fn a_tree_fits_its_area_with_y_up() {
        let nodes = [node(0, 0.0, 0.0), node(1, 4.0, 8.0)];
        let points = StatsMenu::tree_points(&nodes);
        let [left, top, right, bottom] = TREE_AREA;
        let near =
            |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 1e-5 && (a.1 - b.1).abs() < 1e-5;
        assert!(near(points[0], (left, bottom)));
        assert!(near(points[1], (right, top)));
    }

    #[test]
    fn arrows_pick_the_nearest_node_that_way() {
        let points = [(0.5, 0.5), (0.6, 0.5), (0.9, 0.5), (0.5, 0.2), (0.4, 0.9)];
        assert_eq!(StatsMenu::neighbour(&points, 0, 1.0, 0.0), Some(1));
        assert_eq!(StatsMenu::neighbour(&points, 0, 0.0, -1.0), Some(3));
        assert_eq!(StatsMenu::neighbour(&points, 0, 0.0, 1.0), Some(4));
        assert_eq!(StatsMenu::neighbour(&points, 2, 1.0, 0.0), None);
    }
}
