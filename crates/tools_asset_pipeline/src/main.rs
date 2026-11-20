//! Original procedural district & collision layer validator CLI.

use world_streaming::WorldStreamer;

fn main() {
    let (streamer, anchors, puzzles) = WorldStreamer::build_vertical_slice();
    let colliders = streamer.collect_colliders();

    let structural_count = colliders
        .iter()
        .filter(|c| c.layers.is_solid() && !c.layers.is_slashable())
        .count();
    let slashable_count = colliders.iter().filter(|c| c.layers.is_slashable()).count();

    println!("Inkbound Rooftops — Asset & World Validator");
    println!("  Cells loaded:           {}", streamer.cells.len());
    println!("  Active LOD0 colliders:  {}", colliders.len());
    println!("  Structural (immune):    {}", structural_count);
    println!("  Slashable props:        {}", slashable_count);
    println!("  Paint platform anchors: {}", anchors.len());
    println!("  Paint glyph puzzles:    {}", puzzles.len());

    assert!(structural_count > 0, "Vertical slice must contain structural rooftops");
    println!("Validation PASSED.");
}
