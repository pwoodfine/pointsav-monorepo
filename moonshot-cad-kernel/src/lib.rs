//! moonshot-cad-kernel — sovereign CAD kernel foundation (Phase 0).
//!
//! Per `BRIEF-workplace-bim-cad-tool` (research synthesis 2026-07-14), the CAD tool's
//! **document model is the crown jewel to own**: an append-only **operation log** where
//! every change is a typed record, the drawing state is a *rebuildable projection* of the
//! log, and undo/redo + a git-like branching history fall out for free. This crate is that
//! model plus the 2D geometry primitives — Phase 0, no GPU, no constraint solver yet
//! (those layers — `wgpu` rendering, an ISOtope-style solver, and forking `truck` for
//! B-rep 3D — are later phases). The canonical on-disk format is **JSON-Lines** (one op
//! per line): diffable, git-friendly, 50-year-readable.
//!
//! Sovereign, offline, WASM-ready. Deps: `serde` + `serde_json` only (Apache/MIT).
//!
//! ADR note: geometry here is deterministic Rust — no AI inference touches the model.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ─── Geometry primitives ─────────────────────────────────────────────────────

/// A 2D point (model units).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }
    pub fn distance(&self, other: &Point) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
    fn translated(&self, dx: f64, dy: f64) -> Point {
        Point::new(self.x + dx, self.y + dy)
    }
}

/// An axis-aligned bounding box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: Point,
    pub max: Point,
}

impl Bounds {
    fn of_points(pts: &[Point]) -> Option<Bounds> {
        let first = pts.first()?;
        let mut min = *first;
        let mut max = *first;
        for p in pts {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
        }
        Some(Bounds { min, max })
    }
    fn union(a: Option<Bounds>, b: Option<Bounds>) -> Option<Bounds> {
        match (a, b) {
            (Some(a), Some(b)) => Some(Bounds {
                min: Point::new(a.min.x.min(b.min.x), a.min.y.min(b.min.y)),
                max: Point::new(a.max.x.max(b.max.x), a.max.y.max(b.max.y)),
            }),
            (Some(a), None) => Some(a),
            (None, b) => b,
        }
    }
}

/// A drawable 2D entity. The Phase-0 set; extended (splines, text, dimensions) later.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Entity {
    Line { a: Point, b: Point },
    Circle { center: Point, radius: f64 },
    /// Angles in degrees, counter-clockwise from +X.
    Arc {
        center: Point,
        radius: f64,
        start_deg: f64,
        end_deg: f64,
    },
    Polyline { points: Vec<Point>, closed: bool },
}

impl Entity {
    /// Total length (perimeter for closed shapes; circumference for a circle).
    pub fn length(&self) -> f64 {
        match self {
            Entity::Line { a, b } => a.distance(b),
            Entity::Circle { radius, .. } => 2.0 * std::f64::consts::PI * radius,
            Entity::Arc {
                radius,
                start_deg,
                end_deg,
                ..
            } => radius * (end_deg - start_deg).abs().to_radians(),
            Entity::Polyline { points, closed } => {
                let mut total = 0.0;
                for w in points.windows(2) {
                    total += w[0].distance(&w[1]);
                }
                if *closed {
                    if let (Some(first), Some(last)) = (points.first(), points.last()) {
                        total += last.distance(first);
                    }
                }
                total
            }
        }
    }

    fn translated(&self, dx: f64, dy: f64) -> Entity {
        match self {
            Entity::Line { a, b } => Entity::Line {
                a: a.translated(dx, dy),
                b: b.translated(dx, dy),
            },
            Entity::Circle { center, radius } => Entity::Circle {
                center: center.translated(dx, dy),
                radius: *radius,
            },
            Entity::Arc {
                center,
                radius,
                start_deg,
                end_deg,
            } => Entity::Arc {
                center: center.translated(dx, dy),
                radius: *radius,
                start_deg: *start_deg,
                end_deg: *end_deg,
            },
            Entity::Polyline { points, closed } => Entity::Polyline {
                points: points.iter().map(|p| p.translated(dx, dy)).collect(),
                closed: *closed,
            },
        }
    }

    /// Conservative axis-aligned bounds (arcs approximated by their full circle).
    pub fn bounds(&self) -> Bounds {
        match self {
            Entity::Line { a, b } => Bounds::of_points(&[*a, *b]).unwrap(),
            Entity::Circle { center, radius } | Entity::Arc { center, radius, .. } => Bounds {
                min: Point::new(center.x - radius, center.y - radius),
                max: Point::new(center.x + radius, center.y + radius),
            },
            Entity::Polyline { points, .. } => {
                Bounds::of_points(points).unwrap_or(Bounds {
                    min: Point::new(0.0, 0.0),
                    max: Point::new(0.0, 0.0),
                })
            }
        }
    }
}

// ─── Document state (a projection of the op log) ─────────────────────────────

/// A drawing layer.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Layer {
    pub id: u64,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
}

/// An entity placed on a layer.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Placed {
    pub layer: u64,
    pub entity: Entity,
}

/// The current drawing state — deterministically rebuilt by replaying the op log.
/// `BTreeMap` keeps iteration order stable (important for reproducible output/tests).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    pub layers: BTreeMap<u64, Layer>,
    pub entities: BTreeMap<u64, Placed>,
}

impl Document {
    fn apply(&mut self, op: &Op) {
        match op {
            Op::AddLayer { id, name } => {
                self.layers.insert(
                    *id,
                    Layer {
                        id: *id,
                        name: name.clone(),
                        visible: true,
                        locked: false,
                    },
                );
            }
            Op::SetLayerVisible { id, visible } => {
                if let Some(l) = self.layers.get_mut(id) {
                    l.visible = *visible;
                }
            }
            Op::AddEntity { id, layer, entity } => {
                self.entities.insert(
                    *id,
                    Placed {
                        layer: *layer,
                        entity: entity.clone(),
                    },
                );
            }
            Op::MoveEntity { id, dx, dy } => {
                if let Some(p) = self.entities.get_mut(id) {
                    p.entity = p.entity.translated(*dx, *dy);
                }
            }
            Op::DeleteEntity { id } => {
                self.entities.remove(id);
            }
        }
    }

    /// Bounds of all entities, or `None` if empty.
    pub fn bounds(&self) -> Option<Bounds> {
        self.entities
            .values()
            .fold(None, |acc, p| Bounds::union(acc, Some(p.entity.bounds())))
    }
}

// ─── The operation log ───────────────────────────────────────────────────────

/// One entry in the append-only feature/operation log. Serialized one-per-line as JSON.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    AddLayer { id: u64, name: String },
    SetLayerVisible { id: u64, visible: bool },
    AddEntity { id: u64, layer: u64, entity: Entity },
    MoveEntity { id: u64, dx: f64, dy: f64 },
    DeleteEntity { id: u64 },
}

/// A drawing = the operation log + a rebuildable current state + an undo/redo cursor.
///
/// `ops[0..applied]` are the operations currently in effect; `ops[applied..]` is the
/// redo tail (undone but replayable). A new `apply` truncates the redo tail. Save/load
/// use JSON-Lines over the *applied* prefix — the canonical, diffable, git-friendly format.
#[derive(Clone, Debug, Default)]
pub struct Drawing {
    ops: Vec<Op>,
    applied: usize,
    state: Document,
    next_id: u64,
}

impl Drawing {
    pub fn new() -> Self {
        Drawing::default()
    }

    /// Allocate a fresh id (for layers or entities) unique within this drawing session.
    pub fn fresh_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// Apply an operation: drops any redo tail, appends, and updates state.
    pub fn apply(&mut self, op: Op) {
        self.ops.truncate(self.applied);
        self.state.apply(&op);
        self.ops.push(op);
        self.applied += 1;
    }

    /// Undo the last applied op (rebuilds state from the surviving prefix). Returns
    /// `true` if something was undone.
    pub fn undo(&mut self) -> bool {
        if self.applied == 0 {
            return false;
        }
        self.applied -= 1;
        self.rebuild();
        true
    }

    /// Redo a previously-undone op. Returns `true` if something was redone.
    pub fn redo(&mut self) -> bool {
        if self.applied >= self.ops.len() {
            return false;
        }
        self.state.apply(&self.ops[self.applied]);
        self.applied += 1;
        true
    }

    fn rebuild(&mut self) {
        self.state = Document::default();
        for op in &self.ops[..self.applied] {
            self.state.apply(op);
        }
    }

    pub fn document(&self) -> &Document {
        &self.state
    }

    pub fn op_count(&self) -> usize {
        self.applied
    }

    // ── Convenience builders (allocate an id, apply the op, return the id) ──

    pub fn add_layer(&mut self, name: impl Into<String>) -> u64 {
        let id = self.fresh_id();
        self.apply(Op::AddLayer {
            id,
            name: name.into(),
        });
        id
    }

    pub fn add_entity(&mut self, layer: u64, entity: Entity) -> u64 {
        let id = self.fresh_id();
        self.apply(Op::AddEntity { id, layer, entity });
        id
    }

    // ── Serialization: canonical JSON-Lines over the applied prefix ──

    /// Serialize the applied op log as JSON-Lines (one op per line).
    pub fn to_jsonl(&self) -> String {
        let mut out = String::new();
        for op in &self.ops[..self.applied] {
            out.push_str(&serde_json::to_string(op).expect("Op serializes"));
            out.push('\n');
        }
        out
    }

    /// Rebuild a drawing by replaying a JSON-Lines op log. `next_id` is set past the
    /// highest id seen so fresh ids don't collide. Blank lines are ignored.
    pub fn from_jsonl(input: &str) -> Result<Drawing, serde_json::Error> {
        let mut d = Drawing::new();
        for line in input.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let op: Op = serde_json::from_str(line)?;
            let max_id = match &op {
                Op::AddLayer { id, .. }
                | Op::SetLayerVisible { id, .. }
                | Op::AddEntity { id, .. }
                | Op::MoveEntity { id, .. }
                | Op::DeleteEntity { id } => *id,
            };
            d.next_id = d.next_id.max(max_id);
            d.apply(op);
        }
        Ok(d)
    }
}

pub fn system_status() -> &'static str {
    "moonshot-cad-kernel: Phase 0 (geometry + op-log document model)"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_lengths() {
        let line = Entity::Line {
            a: Point::new(0.0, 0.0),
            b: Point::new(3.0, 4.0),
        };
        assert_eq!(line.length(), 5.0);
        let sq = Entity::Polyline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(2.0, 0.0),
                Point::new(2.0, 2.0),
                Point::new(0.0, 2.0),
            ],
            closed: true,
        };
        assert_eq!(sq.length(), 8.0); // closed 2×2 square perimeter
    }

    #[test]
    fn apply_builds_state() {
        let mut d = Drawing::new();
        let layer = d.add_layer("walls");
        let e = d.add_entity(
            layer,
            Entity::Line {
                a: Point::new(0.0, 0.0),
                b: Point::new(10.0, 0.0),
            },
        );
        assert_eq!(d.document().layers.len(), 1);
        assert_eq!(d.document().entities.len(), 1);
        assert_eq!(d.document().entities[&e].layer, layer);
    }

    #[test]
    fn move_entity_translates_geometry() {
        let mut d = Drawing::new();
        let layer = d.add_layer("l");
        let e = d.add_entity(layer, Entity::Circle { center: Point::new(1.0, 1.0), radius: 2.0 });
        d.apply(Op::MoveEntity { id: e, dx: 5.0, dy: 0.0 });
        if let Entity::Circle { center, .. } = d.document().entities[&e].entity {
            assert_eq!(center, Point::new(6.0, 1.0));
        } else {
            panic!("expected circle");
        }
    }

    #[test]
    fn undo_redo_round_trips_state() {
        let mut d = Drawing::new();
        let layer = d.add_layer("l");
        let e = d.add_entity(layer, Entity::Line { a: Point::new(0.0, 0.0), b: Point::new(1.0, 0.0) });
        d.apply(Op::MoveEntity { id: e, dx: 0.0, dy: 3.0 });
        let moved = d.document().clone();

        assert!(d.undo()); // undo the move
        assert_eq!(d.document().entities[&e].entity, Entity::Line { a: Point::new(0.0, 0.0), b: Point::new(1.0, 0.0) });
        assert!(d.redo()); // redo the move
        assert_eq!(*d.document(), moved);

        // Undo everything.
        assert!(d.undo());
        assert!(d.undo());
        assert!(d.undo());
        assert!(!d.undo()); // nothing left
        assert!(d.document().entities.is_empty());
        assert!(d.document().layers.is_empty());
    }

    #[test]
    fn apply_after_undo_drops_redo_tail() {
        let mut d = Drawing::new();
        let layer = d.add_layer("l");
        d.add_entity(layer, Entity::Circle { center: Point::new(0.0, 0.0), radius: 1.0 });
        d.undo(); // undo the circle
        d.add_entity(layer, Entity::Circle { center: Point::new(9.0, 9.0), radius: 1.0 }); // new branch
        assert!(!d.redo(), "redo tail must be gone after a new apply");
        assert_eq!(d.document().entities.len(), 1);
    }

    #[test]
    fn jsonl_round_trip_reproduces_state() {
        let mut d = Drawing::new();
        let walls = d.add_layer("walls");
        d.add_entity(walls, Entity::Line { a: Point::new(0.0, 0.0), b: Point::new(4000.0, 0.0) });
        d.add_entity(walls, Entity::Arc { center: Point::new(0.0, 0.0), radius: 500.0, start_deg: 0.0, end_deg: 90.0 });
        let jsonl = d.to_jsonl();
        // Three lines (add_layer + 2 add_entity).
        assert_eq!(jsonl.lines().count(), 3);

        let reloaded = Drawing::from_jsonl(&jsonl).unwrap();
        assert_eq!(*reloaded.document(), *d.document());
        // fresh_id must not collide with reloaded ids.
        let mut reloaded = reloaded;
        assert!(reloaded.fresh_id() > 3);
    }

    #[test]
    fn document_bounds() {
        let mut d = Drawing::new();
        let l = d.add_layer("l");
        d.add_entity(l, Entity::Line { a: Point::new(1.0, 2.0), b: Point::new(5.0, 2.0) });
        d.add_entity(l, Entity::Circle { center: Point::new(0.0, 0.0), radius: 3.0 });
        let b = d.document().bounds().unwrap();
        assert_eq!(b.min, Point::new(-3.0, -3.0));
        assert_eq!(b.max, Point::new(5.0, 3.0));
    }
}
