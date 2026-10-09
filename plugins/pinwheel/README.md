# pinwheel

Pinwheel is a local reference board: add image files, tag and filter them, then
double-click or use **Pop out** to place the selected reference on the canvas.
It persists only paths and tags in Krita's application-data directory.

The board uses a compact thumbnail grid with a review pane. References can be
filtered, tagged, retagged, and removed from the board without deleting their
original image files.

Pop out uses Krita's **Paste as Reference Image** action. Through the shared
`kassist_krita.py` helpers it creates a temporary paint layer from the selected
file, selects and copies its pixels, invokes that action, removes the temporary
layer, and restores the layer that was active before. The result is a native
Krita reference image, not an ordinary paint layer.

The board itself uses `kassist_krita.py` for its application-data directory, the
active document, and dock widget registration.
