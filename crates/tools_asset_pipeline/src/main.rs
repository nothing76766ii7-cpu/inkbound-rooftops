//! Original procedural district & real-slicing validator CLI (PRD v3.0).

use engine_core::CollisionLayer;
use world_streaming::WorldStreamer;

fn main() {
    let (streamer, anchors, puzzles) = WorldStreamer::build_vertical_slice();
    let colliders = streamer.collect_colliders();

    let sliceable_arch = colliders
        .iter()
        .filter(|c| c.layers.contains(CollisionLayer::SLICEABLE_WORLD))
        .count();
    let total_tris: usize = streamer
        .cells
        .values()
        .flat_map(|c| c.modules.iter())
        .map(|m| m.mesh.tris.len())
        .sum();

    println!("Inkbound Rooftops v3.0 — World & Slicing Validator");
    println!("  Cells loaded:           {}", streamer.cells.len());
    println!("  Active LOD0 colliders:  {}", colliders.len());
    println!("  Sliceable arch modules: {}", sliceable_arch);
    println!("  Total mesh triangles:   {}", total_tris);
    println!("  Paint platform anchors: {}", anchors.len());
    println!("  Paint glyph puzzles:    {}", puzzles.len());

    assert!(sliceable_arch > 0, "Vertical slice must contain sliceable architecture");
    println!("Validation PASSED.");
}
