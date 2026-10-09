# ModelPreview

ModelPreview loads GLB, glTF, and FBX scenes through `trimesh` and shows a
native Qt wireframe preview dialog. It intentionally limits preview geometry to
a configurable face budget, transforms vertices with NumPy, and caches the
projected edges between repaints so large scenes remain responsive. FBX support
depends on the importer backend available to the packaged `trimesh` build; the
dock reports a direct importer error when one is unavailable.

Pose sliders control yaw, pitch, roll, scale, and the preview face budget.
The MakeHuman controls write a portable `.mhm` modifier profile for use in
MakeHuman; generating a profile does not require bundling MakeHuman itself.
