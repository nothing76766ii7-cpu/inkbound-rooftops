#!/usr/bin/env python3
"""Zero-third-party sanity check for source-bundled V9 assets.
Run before `cargo check`; this DOES NOT replace shader/device build validation.
"""
from pathlib import Path
import json, struct

root = Path(__file__).resolve().parents[1]
rig = json.loads((root/'assets/animations/viewmodel_rig.json').read_text())
assert len(rig['joints']) == 8
assert rig['version'] == 1
for i, joint in enumerate(rig['joints']):
    assert -1 <= joint['parent'] < i
for name in ('idle','run','slide','wall_run','fall','hook','light_a','light_b','light_c','heavy','dimensional'):
    clip=rig['clips'][name]
    ts=[f['t'] for f in clip['frames']]
    assert ts[0] == 0 and ts[-1] == 1 and ts == sorted(ts)
    for f in clip['frames']:
        assert len(f['rotations']) == len(f['offsets']) == 8
print('PASS: 8-joint skeletal rig and 11 animation clips')

for name in ('terracotta_roof','sandstone_tower','inkblade'):
    obj=root/f'assets/models/{name}.obj'
    glb=root/f'assets/models/{name}.glb'
    verts=sum(1 for line in obj.read_text().splitlines() if line.startswith('v '))
    tris=sum(1 for line in obj.read_text().splitlines() if line.startswith('f '))
    assert verts > 40 and tris > 30, (name,verts,tris)
    content=glb.read_bytes()
    assert content[:4]==b'glTF' and struct.unpack_from('<I',content,4)[0] == 2
    print(f'PASS: {name} OBJ {tris} triangles and editable glTF binary')

for name in ('plaster','sandstone','terracotta','charcoal_steel'):
    blob=(root/f'assets/materials/{name}.png').read_bytes()
    assert blob[:8]==b'\x89PNG\r\n\x1a\n'
    assert struct.unpack_from('>II',blob,16) == (512,512)
print('PASS: 4 material PNGs are 512x512')

world=(root/'crates/render_wgpu/src/shaders/world.wgsl').read_text()
render=(root/'crates/render_wgpu/src/lib.rs').read_text()
shadow=(root/'crates/render_wgpu/src/shaders/shadow.wgsl').read_text()
for index in range(6):
    assert f'@binding({index})' in world
    assert f'binding: {index},' in render
for word in ('sun_visibility','atlas_detail','textureSampleCompareLevel','textureSampleLevel'):
    assert word in world
assert 'vs_shadow' in shadow and 'Shadow Map Pass' in render
assert 'draw_viewmodel(' in render and 'include_str!("../../../assets/models/inkblade.obj")' in (root/'crates/render_wgpu/src/skinned.rs').read_text()
print('PASS: texture/shadow shader bindings and imported viewmodel source references')
print('Asset integrity PASS; Rust compilation and Android GPU testing must still be run.')
