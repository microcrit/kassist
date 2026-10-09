# Cakewalk Modifiers

Cakewalk Modifiers is the independent home for pixel-driven Cakewalk tools.
It replaces the former HairFlow plugin with a **Hair modifier** that writes a
semi-transparent guide layer underneath selected painted artwork. Its renderer
is the Cakewalk `hair-guides` plug, so it shares the same user-program and
engine architecture as the other Cakewalk tools.

The **Brushes** tab is a functional canvas-tool picker: every stock brush is
loaded from its own Cakewalk plug and rendered with its real engine for the
preview. It includes bounded shading, edge ink, blur, eraser, smudge, clone,
contiguous and alpha-bounded fill, direct color picking, two stabilizers, local
transform, warp, and rotation-aware distortion. Selecting **Enable canvas
tool** installs the pressure/touch filter and manually writes pixels; it never
delegates the stroke to Krita's preset engine.

**Create user brush** writes a safe starter program in Krita's per-user
application-data plug directory. Edit that file and use **Reload brushes** to
explicitly load it into the picker. Discovery alone never executes user code.
