//! Real-time 3D plane-mesh slicer with closed cut-cap triangulation (PRD v3.0 P0).
//!
//! Classifies vertices by signed distance to a world-space `CutPlane`, splits
//! crossing edges cleanly, preserves CCW winding and exterior materials, generates
//! closed interior cross-section caps with distinct core materials, and rejects
//! degenerate triangles.

use crate::Aabb;
use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

const PLANE_EPS: f32 = 1e-4;

/// World-space slicing plane defined by aim origin, plane normal, and spatial reach.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CutPlane {
    pub origin: Vec3,
    pub normal: Vec3,
    pub forward: Vec3,
    pub range: f32,
    pub half_width: f32,
    pub half_height: f32,
}

impl CutPlane {
    pub fn new(origin: Vec3, normal: Vec3, forward: Vec3, range: f32, half_width: f32) -> Self {
        Self {
            origin,
            normal: normal.normalize_or_zero(),
            forward: forward.normalize_or_zero(),
            range,
            half_width,
            half_height: half_width,
        }
    }

    #[inline]
    pub fn signed_distance(&self, p: Vec3) -> f32 {
        (p - self.origin).dot(self.normal)
    }

    /// Quick broadphase test: does this cut plane's finite sweep box intersect `bounds`
    /// and do `bounds` corners straddle both sides of the plane?
    pub fn intersects_aabb(&self, bounds: &Aabb) -> bool {
        let center = bounds.center();
        let to_c = center - self.origin;
        let along = to_c.dot(self.forward);
        let radius = bounds.half_extents().length();
        if along < -radius || along > self.range + radius {
            return false;
        }
        // Check lateral distance from cut center ray
        let closest_on_ray = self.origin + self.forward * along.clamp(0.0, self.range);
        let lateral = (center - closest_on_ray).length();
        if lateral > self.half_width + radius {
            return false;
        }

        // Verify corners actually lie on both sides of the plane
        let mut has_pos = false;
        let mut has_neg = false;
        for ix in 0..2 {
            for iy in 0..2 {
                for iz in 0..2 {
                    let corner = Vec3::new(
                        if ix == 0 { bounds.min.x } else { bounds.max.x },
                        if iy == 0 { bounds.min.y } else { bounds.max.y },
                        if iz == 0 { bounds.min.z } else { bounds.max.z },
                    );
                    let d = self.signed_distance(corner);
                    if d > PLANE_EPS {
                        has_pos = true;
                    } else if d < -PLANE_EPS {
                        has_neg = true;
                    }
                }
            }
        }
        has_pos && has_neg
    }
}

/// Single closed-mesh triangle with exterior/interior cut-face material metadata.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MeshTri {
    pub v: [Vec3; 3],
    pub normal: Vec3,
    pub color: [f32; 4],
    pub is_cut_face: bool,
}

impl MeshTri {
    pub fn new(v0: Vec3, v1: Vec3, v2: Vec3, color: [f32; 4], is_cut_face: bool) -> Option<Self> {
        let cross = (v1 - v0).cross(v2 - v0);
        let area2 = cross.length();
        if area2 < 1e-5 {
            return None;
        }
        Some(Self {
            v: [v0, v1, v2],
            normal: cross / area2,
            color,
            is_cut_face,
        })
    }
}

/// Authoritative triangle mesh for a sliceable building module (wall, roof, tower, crate).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliceableMesh {
    pub tris: Vec<MeshTri>,
}

impl SliceableMesh {
    /// Generates a closed 12-triangle box mesh from an `Aabb`.
    pub fn from_box(b: Aabb, color: [f32; 4]) -> Self {
        let (x0, y0, z0) = (b.min.x, b.min.y, b.min.z);
        let (x1, y1, z1) = (b.max.x, b.max.y, b.max.z);
        let mut tris = Vec::with_capacity(12);
        let mut quad = |a: Vec3, b_p: Vec3, c: Vec3, d: Vec3| {
            if let Some(t1) = MeshTri::new(a, b_p, c, color, false) {
                tris.push(t1);
            }
            if let Some(t2) = MeshTri::new(a, c, d, color, false) {
                tris.push(t2);
            }
        };
        // +Y, -Y, +Z, -Z, +X, -X (CCW outward)
        quad(Vec3::new(x0, y1, z1), Vec3::new(x1, y1, z1), Vec3::new(x1, y1, z0), Vec3::new(x0, y1, z0));
        quad(Vec3::new(x0, y0, z0), Vec3::new(x1, y0, z0), Vec3::new(x1, y0, z1), Vec3::new(x0, y0, z1));
        quad(Vec3::new(x0, y0, z1), Vec3::new(x1, y0, z1), Vec3::new(x1, y1, z1), Vec3::new(x0, y1, z1));
        quad(Vec3::new(x1, y0, z0), Vec3::new(x0, y0, z0), Vec3::new(x0, y1, z0), Vec3::new(x1, y1, z0));
        quad(Vec3::new(x1, y0, z1), Vec3::new(x1, y0, z0), Vec3::new(x1, y1, z0), Vec3::new(x1, y1, z1));
        quad(Vec3::new(x0, y0, z0), Vec3::new(x0, y0, z1), Vec3::new(x0, y1, z1), Vec3::new(x0, y1, z0));
        Self { tris }
    }

    /// Generates a closed 6-triangle pitched orange roof pyramid from an `Aabb`.
    pub fn from_pitched_roof(b: Aabb, color: [f32; 4]) -> Self {
        let (x0, y0, z0) = (b.min.x, b.min.y, b.min.z);
        let (x1, y1, z1) = (b.max.x, b.max.y, b.max.z);
        let apex = Vec3::new((x0 + x1) * 0.5, y1, (z0 + z1) * 0.5);
        let c0 = Vec3::new(x0, y0, z1);
        let c1 = Vec3::new(x1, y0, z1);
        let c2 = Vec3::new(x1, y0, z0);
        let c3 = Vec3::new(x0, y0, z0);
        let mut tris = Vec::with_capacity(6);
        for (a, b_p, c) in [
            (c0, c1, apex),
            (c1, c2, apex),
            (c2, c3, apex),
            (c3, c0, apex),
            (c0, c3, c2),
            (c0, c2, c1),
        ] {
            if let Some(t) = MeshTri::new(a, b_p, c, color, false) {
                tris.push(t);
            }
        }
        Self { tris }
    }

    /// Computes exact tight AABB of all vertices in this mesh.
    pub fn compute_bounds(&self) -> Option<Aabb> {
        let first = self.tris.first()?.v[0];
        let mut min = first;
        let mut max = first;
        for t in &self.tris {
            for &p in &t.v {
                min = min.min(p);
                max = max.max(p);
            }
        }
        Some(Aabb::new(min, max))
    }

    pub fn centroid(&self) -> Vec3 {
        if self.tris.is_empty() {
            return Vec3::ZERO;
        }
        let mut sum = Vec3::ZERO;
        for t in &self.tris {
            sum += (t.v[0] + t.v[1] + t.v[2]) / 3.0;
        }
        sum / self.tris.len() as f32
    }

    /// Approximate volume via AABB extents (used for mass proportional to volume).
    pub fn approx_volume(&self) -> f32 {
        match self.compute_bounds() {
            Some(b) => {
                let sz = b.max - b.min;
                (sz.x * sz.y * sz.z).max(0.05)
            }
            None => 0.0,
        }
    }

    /// Slices this closed mesh along `plane`, returning `(positive_half, negative_half)`
    /// with closed interior cross-section caps colored with `cap_color`.
    pub fn slice(&self, plane: &CutPlane, cap_color: [f32; 4]) -> Option<(Self, Self)> {
        let mut pos_tris = Vec::with_capacity(self.tris.len());
        let mut neg_tris = Vec::with_capacity(self.tris.len());
        let mut cut_points: Vec<Vec3> = Vec::new();

        for tri in &self.tris {
            let d = [
                plane.signed_distance(tri.v[0]),
                plane.signed_distance(tri.v[1]),
                plane.signed_distance(tri.v[2]),
            ];
            let pos = [d[0] >= -PLANE_EPS, d[1] >= -PLANE_EPS, d[2] >= -PLANE_EPS];
            let pos_count = pos.iter().filter(|&&b| b).count();

            if pos_count == 3 {
                pos_tris.push(*tri);
            } else if pos_count == 0 {
                neg_tris.push(*tri);
            } else {
                // Find the solo vertex index (1 on one side, 2 on the other)
                let solo_is_pos = pos_count == 1;
                let solo_idx = (0..3).find(|&i| pos[i] == solo_is_pos).unwrap_or(0);
                let i0 = solo_idx;
                let i1 = (solo_idx + 1) % 3;
                let i2 = (solo_idx + 2) % 3;

                let v0 = tri.v[i0];
                let v1 = tri.v[i1];
                let v2 = tri.v[i2];

                let t01 = (d[i0] / (d[i0] - d[i1])).clamp(0.0, 1.0);
                let t02 = (d[i0] / (d[i0] - d[i2])).clamp(0.0, 1.0);
                let p01 = v0.lerp(v1, t01);
                let p02 = v0.lerp(v2, t02);

                cut_points.push(p01);
                cut_points.push(p02);

                let (solo_dst, pair_dst) = if solo_is_pos {
                    (&mut pos_tris, &mut neg_tris)
                } else {
                    (&mut neg_tris, &mut pos_tris)
                };

                if let Some(t) = MeshTri::new(v0, p01, p02, tri.color, tri.is_cut_face) {
                    solo_dst.push(t);
                }
                if let Some(t) = MeshTri::new(p01, v1, v2, tri.color, tri.is_cut_face) {
                    pair_dst.push(t);
                }
                if let Some(t) = MeshTri::new(p01, v2, p02, tri.color, tri.is_cut_face) {
                    pair_dst.push(t);
                }
            }
        }

        if pos_tris.is_empty() || neg_tris.is_empty() {
            return None;
        }

        // Generate closed interior cut caps for both halves via planar angular sorting
        build_cut_caps(&cut_points, plane.normal, cap_color, &mut pos_tris, &mut neg_tris);

        Some((Self { tris: pos_tris }, Self { tris: neg_tris }))
    }
}

/// Deduplicates coplanar cut vertices, sorts them cyclically around their centroid
/// in the cut plane's 2D basis, and emits closed opposing cap fans for both halves.
fn build_cut_caps(
    raw_points: &[Vec3],
    plane_normal: Vec3,
    cap_color: [f32; 4],
    pos_tris: &mut Vec<MeshTri>,
    neg_tris: &mut Vec<MeshTri>,
) {
    let mut unique: Vec<Vec3> = Vec::new();
    for &p in raw_points {
        if !unique.iter().any(|q| q.distance_squared(p) < 1e-4) {
            unique.push(p);
        }
    }
    if unique.len() < 3 {
        return;
    }

    let centroid = unique.iter().copied().sum::<Vec3>() / unique.len() as f32;
    let ref_axis = if plane_normal.y.abs() < 0.9 {
        Vec3::Y
    } else {
        Vec3::X
    };
    let u = plane_normal.cross(ref_axis).normalize_or_zero();
    let v = plane_normal.cross(u).normalize_or_zero();

    let mut ordered: Vec<(f32, Vec3)> = unique
        .into_iter()
        .map(|p| {
            let rel = p - centroid;
            let p2 = Vec2::new(rel.dot(u), rel.dot(v));
            (p2.y.atan2(p2.x), p)
        })
        .collect();
    ordered.sort_by(|a, b| a.0.total_cmp(&b.0));

    let n = ordered.len();
    for i in 0..n {
        let a = ordered[i].1;
        let b = ordered[(i + 1) % n].1;

        // Positive half cap faces -plane_normal; Negative half cap faces +plane_normal
        if let Some(mut tp) = MeshTri::new(centroid, b, a, cap_color, true) {
            if tp.normal.dot(-plane_normal) < 0.0 {
                tp.v.swap(1, 2);
                tp.normal = -tp.normal;
            }
            pos_tris.push(tp);
        }
        if let Some(mut tn) = MeshTri::new(centroid, a, b, cap_color, true) {
            if tn.normal.dot(plane_normal) < 0.0 {
                tn.v.swap(1, 2);
                tn.normal = -tn.normal;
            }
            neg_tris.push(tn);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slicing_box_wall_produces_two_closed_capped_halves_and_supports_repeat_cut() {
        let wall = SliceableMesh::from_box(
            Aabb::new(Vec3::new(-3.0, 0.0, -0.5), Vec3::new(3.0, 4.0, 0.5)),
            [0.95, 0.95, 0.93, 1.0],
        );
        // Diagonal cut plane across the wall
        let plane = CutPlane::new(
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(0.3, 0.95, 0.0),
            Vec3::NEG_Z,
            20.0,
            6.0,
        );
        let cap_col = [0.82, 0.78, 0.72, 1.0];
        let (upper, lower) = wall.slice(&plane, cap_col).expect("Wall should slice cleanly");

        assert!(upper.tris.iter().any(|t| t.is_cut_face), "Upper half must have closed cut cap");
        assert!(lower.tris.iter().any(|t| t.is_cut_face), "Lower half must have closed cut cap");
        assert!(upper.centroid().y > lower.centroid().y);

        // Repeat cut on the upper half (PRD v3.0 Gate 3)
        let plane2 = CutPlane::new(
            upper.centroid(),
            Vec3::X,
            Vec3::NEG_Z,
            20.0,
            6.0,
        );
        let (left, right) = upper.slice(&plane2, cap_col).expect("Second cut on sliced piece should succeed");
        assert!(left.tris.len() >= 6 && right.tris.len() >= 6);
    }
}
