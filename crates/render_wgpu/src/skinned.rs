//! First-person skeletal animation asset loader and two-bone CPU skinner.
//!
//! The 8-joint authored skeleton and 10 keyframed animations are imported from
//! `assets/animations/viewmodel_rig.json` at build time. The authorable asset
//! drives actual weighted vertices; the previous rigid sword/forearm box pose
//! function is no longer used. CPU skinning keeps compatibility with Android
//! Vulkan devices without storage buffers or compute-shader prerequisites.

use crate::WorldVertex;
use engine_core::Palette;
use gameplay_combat::{AttackKind, AttackState, CombatState};
use gameplay_player::LocomotionState;
use glam::{EulerRot, Mat4, Quat, Vec2, Vec3};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Joint { name: String, parent: i32, translation: [f32; 3] }
#[derive(Deserialize)]
struct Frame { t: f32, rotations: Vec<[f32; 3]>, offsets: Vec<[f32; 3]> }
#[derive(Deserialize)]
struct Clip { r#loop: bool, frames: Vec<Frame> }
#[derive(Deserialize)]
struct RigAsset { joints: Vec<Joint>, clips: HashMap<String, Clip> }

#[derive(Clone)]
struct SkinVertex {
    position: Vec3, normal: Vec3, color: [f32; 4],
    joint_a: usize, joint_b: usize, weight_a: f32,
}
struct SkinnedModel {
    rig: RigAsset,
    inverse_bind: Vec<Mat4>,
    vertices: Vec<SkinVertex>,
}

fn rgb(x: [f32; 4]) -> [f32; 4] { x }
fn v3(v: [f32; 3]) -> Vec3 { Vec3::from_array(v) }
fn rot(v: [f32; 3]) -> Quat {
    Quat::from_euler(EulerRot::XYZ, v[0].to_radians(), v[1].to_radians(), v[2].to_radians())
}
fn bind_transforms(rig: &RigAsset) -> Vec<Mat4> {
    let mut world = Vec::with_capacity(rig.joints.len());
    for joint in &rig.joints {
        let local = Mat4::from_translation(v3(joint.translation));
        let full = if joint.parent >= 0 { world[joint.parent as usize] * local } else { local };
        world.push(full);
    }
    world
}
fn add_tri(out: &mut Vec<SkinVertex>, points: [Vec3; 3], joints: [(usize, usize, f32); 3], color: [f32; 4]) {
    let n = (points[1] - points[0]).cross(points[2] - points[0]).normalize_or_zero();
    for (p, (a, b, w)) in points.into_iter().zip(joints) {
        out.push(SkinVertex { position: p, normal: n, color, joint_a: a, joint_b: b, weight_a: w });
    }
}
fn add_skin_tube(out: &mut Vec<SkinVertex>, p0: Vec3, p1: Vec3, bone0: usize, bone1: usize,
                 r0: f32, r1: f32, color: [f32; 4]) {
    let direction = (p1 - p0).normalize_or_zero();
    let x = direction.cross(Vec3::Y).normalize_or_zero();
    let x = if x.length_squared() < 0.1 { Vec3::X } else { x };
    let y = direction.cross(x).normalize_or_zero();
    let n = 8;
    let rings = [0.0_f32, 0.5, 1.0];
    for seg in 0..2 {
        let lo = rings[seg]; let hi = rings[seg + 1];
        for i in 0..n {
            let a = i as f32 * std::f32::consts::TAU / n as f32;
            let b = (i + 1) as f32 * std::f32::consts::TAU / n as f32;
            let point = |t: f32, ang: f32| {
                let rad = r0 + (r1 - r0) * t;
                p0.lerp(p1, t) + (x * ang.cos() + y * ang.sin()) * rad
            };
            let quad = [point(lo, a), point(lo, b), point(hi, b), point(hi, a)];
            let influence = |t: f32| (bone0, bone1, 1.0 - t);
            add_tri(out, [quad[0], quad[1], quad[2]],
                [influence(lo), influence(lo), influence(hi)], color);
            add_tri(out, [quad[0], quad[2], quad[3]],
                [influence(lo), influence(hi), influence(hi)], color);
        }
    }
}
fn add_box_local(out: &mut Vec<SkinVertex>, bind: &[Mat4], bone: usize,
                 local_center: Vec3, local_half: Vec3, color: [f32; 4]) {
    let (x,y,z) = (local_half.x, local_half.y, local_half.z);
    let c = local_center;
    let a = [
        c + Vec3::new(-x,-y,-z), c + Vec3::new(x,-y,-z),
        c + Vec3::new(x,y,-z), c + Vec3::new(-x,y,-z),
        c + Vec3::new(-x,-y,z), c + Vec3::new(x,-y,z),
        c + Vec3::new(x,y,z), c + Vec3::new(-x,y,z),
    ];
    let faces = [(0,3,2,1),(4,5,6,7),(0,4,7,3),(1,2,6,5),(3,7,6,2),(0,1,5,4)];
    for (i,j,k,l) in faces {
        let skin = |idx: usize| bind[bone].transform_point3(a[idx]);
        let joint = (bone,bone,1.0);
        add_tri(out, [skin(i),skin(j),skin(k)], [joint;3], color);
        add_tri(out, [skin(i),skin(k),skin(l)], [joint;3], color);
    }
}
/// Import the complete original OBJ weapon (blade bevels, etched fuller, bronze
/// guard, leather grip and pommel) and bind it to the animated sword joint.
/// This is genuine imported mesh geometry, not the old hard-coded sword blocks.
fn add_imported_inkblade(out: &mut Vec<SkinVertex>, bind: &[Mat4]) {
    let source=include_str!("../../../assets/models/inkblade.obj");
    let mut points: Vec<Vec3>=Vec::new();
    let mut color=[0.72,0.81,0.88,1.0];
    let binding=(7,7,1.0);
    for line in source.lines() {
        let mut words=line.split_whitespace();
        match words.next() {
            Some("v") => {
                let coords: Vec<f32>=words.take(3).filter_map(|s|s.parse::<f32>().ok()).collect();
                if coords.len()==3 {points.push(Vec3::new(coords[0],coords[1],coords[2]));}
            }
            Some("usemtl") => {
                color=match words.next().unwrap_or("") {
                    "edge" => [0.96,0.90,0.72,1.0],
                    "bronze" => [0.77,0.43,0.23,1.0],
                    "ink" => [0.19,0.17,0.22,1.0],
                    "glow" => [1.0,0.48,0.17,1.0],
                    _ => [0.72,0.81,0.88,1.0],
                };
            }
            Some("f") => {
                let ids: Vec<usize>=words.take(3).filter_map(|s|s.parse::<usize>().ok())
                    .filter(|x|*x>0).map(|x|x-1).collect();
                if ids.len()==3 && ids.iter().all(|i|*i<points.len()) {
                    let pos=[bind[7].transform_point3(points[ids[0]]),
                             bind[7].transform_point3(points[ids[1]]),
                             bind[7].transform_point3(points[ids[2]])];
                    add_tri(out,pos,[binding;3],color);
                }
            }
            _ => {}
        }
    }
}

fn make_model() -> SkinnedModel {
    let rig: RigAsset = serde_json::from_str(include_str!("../../../assets/animations/viewmodel_rig.json"))
        .expect("Inkbound authored skeletal animation JSON must be valid");
    assert_eq!(rig.joints.len(), 8, "Viewmodel joint count must be eight");
    let bind = bind_transforms(&rig);
    let center = |i: usize| bind[i].transform_point3(Vec3::ZERO);
    let mut vertices = Vec::with_capacity(1600);
    let sleeve = [0.38, 0.31, 0.31, 1.0];
    let cuff = [0.18, 0.16, 0.19, 1.0];
    add_skin_tube(&mut vertices, center(1), center(2), 1, 2, 0.079, 0.061, sleeve);
    add_skin_tube(&mut vertices, center(2), center(3), 2, 3, 0.063, 0.036, cuff);
    add_skin_tube(&mut vertices, center(4), center(5), 4, 5, 0.072, 0.055, sleeve);
    add_skin_tube(&mut vertices, center(5), center(6), 5, 6, 0.056, 0.033, cuff);
    add_box_local(&mut vertices,&bind,3, Vec3::ZERO, Vec3::new(0.042, 0.042, 0.054),Palette::PAPER);
    add_box_local(&mut vertices,&bind,6, Vec3::ZERO, Vec3::new(0.043, 0.045, 0.052),Palette::PAPER);
    // The imported OBJ contains every metal and grip part, already skinned to wrist.
    add_imported_inkblade(&mut vertices,&bind);
    let inverse_bind = bind.iter().map(|m| m.inverse()).collect();
    SkinnedModel {rig, inverse_bind, vertices}
}

fn model() -> &'static SkinnedModel {
    static MODEL: OnceLock<SkinnedModel> = OnceLock::new();
    MODEL.get_or_init(make_model)
}

fn phase(combat: &CombatState) -> f32 {
    let p=combat.phase_progress();
    match combat.state {
        AttackState::Windup => p * 0.28,
        AttackState::Active => 0.28 + p * 0.32,
        AttackState::Recovery => 0.60 + p * 0.40,
        AttackState::Charged => 0.25,
        _ => 0.0,
    }
}
fn clip_name(combat: &CombatState, locomotion: LocomotionState,
             hook: bool, speed: f32) -> &'static str {
    if combat.state != AttackState::Idle {
        return match combat.current_attack {
            Some(AttackKind::LightA) => "light_a",
            Some(AttackKind::LightB) => "light_b",
            Some(AttackKind::LightC) => "light_c",
            Some(AttackKind::Heavy) => "heavy",
            Some(AttackKind::Dimensional) => "dimensional",
            None => "idle",
        };
    }
    if hook {return "hook";}
    match locomotion {
        LocomotionState::Sliding => "slide",
        LocomotionState::WallRunning => "wall_run",
        LocomotionState::Falling => "fall",
        _ if speed > 0.22 => "run",
        _ => "idle",
    }
}
fn sample_pose(model: &SkinnedModel, clip_name: &str, t: f32) -> Vec<Mat4> {
    let clip = model.rig.clips.get(clip_name).unwrap_or(&model.rig.clips["idle"]);
    let t = if clip.r#loop {t.fract()} else {t.clamp(0.0,1.0)};
    let mut a=&clip.frames[0]; let mut b=&clip.frames[clip.frames.len()-1];
    for pair in clip.frames.windows(2) {
        if t >= pair[0].t && t <= pair[1].t {a=&pair[0]; b=&pair[1];break;}
    }
    let alpha = ((t-a.t)/(b.t-a.t).max(0.0001)).clamp(0.0,1.0);
    let mut transforms = Vec::with_capacity(model.rig.joints.len());
    for (index,joint) in model.rig.joints.iter().enumerate() {
        let r=rot(a.rotations[index]).slerp(rot(b.rotations[index]),alpha);
        let shift=v3(a.offsets[index]).lerp(v3(b.offsets[index]),alpha);
        let local=Mat4::from_rotation_translation(r,v3(joint.translation)+shift);
        let full=if joint.parent>=0 {transforms[joint.parent as usize]*local} else {local};
        transforms.push(full);
    }
    transforms
}

/// Skinned triangles are CPU transformed into the shared world batch; the weighted
/// arm vertices are interpolated between adjacent animated joint matrices.
#[allow(clippy::too_many_arguments)]
pub fn draw_viewmodel(out: &mut Vec<WorldVertex>, camera_pos: Vec3, look: Vec3,
                      bob: f32, inertia: Vec2, locomotion: LocomotionState,
                      combat: &CombatState, hook: bool, speed: f32,
                      animation_seconds: f32, reduced_motion: bool) {
    let model=model();
    let clip=clip_name(combat,locomotion,hook,speed);
    let phase=if combat.state != AttackState::Idle {phase(combat)}
              else {animation_seconds * (if clip=="run" {1.65} else {0.75})};
    let bones=sample_pose(model,clip,phase);
    let skin: Vec<Mat4>=bones.iter().zip(&model.inverse_bind).map(|(a,b)| *a * *b).collect();
    let fwd=look.normalize_or_zero();
    let right=fwd.cross(Vec3::Y).normalize_or_zero();
    let up=right.cross(fwd).normalize_or_zero();
    let bob_height=if reduced_motion {0.0} else {bob.sin().abs()*0.018};
    let origin=camera_pos+right * (-inertia.x*0.045)+up*(bob_height-inertia.y*0.025);
    for v in &model.vertices {
        let a=skin[v.joint_a].transform_point3(v.position);
        let b=skin[v.joint_b].transform_point3(v.position);
        let local=a*v.weight_a+b*(1.0-v.weight_a);
        let n=(skin[v.joint_a].transform_vector3(v.normal)*v.weight_a+
               skin[v.joint_b].transform_vector3(v.normal)*(1.0-v.weight_a)).normalize_or_zero();
        let world=origin+right*local.x+up*local.y+fwd*local.z;
        let world_n=(right*n.x+up*n.y+fwd*n.z).normalize_or_zero();
        out.push(WorldVertex {position:world.to_array(),normal:world_n.to_array(),
                              color:rgb(v.color), material:-1.0});
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imported_clips_and_bones_are_valid() {
        let rig=model();
        for name in ["idle", "run", "slide", "wall_run", "hook", "light_a",
                     "light_b", "light_c", "heavy", "dimensional"] {
            assert!(rig.rig.clips.contains_key(name));
            assert_eq!(sample_pose(rig,name,0.5).len(),8);
        }
        assert!(rig.vertices.len()>300);
    }
}
