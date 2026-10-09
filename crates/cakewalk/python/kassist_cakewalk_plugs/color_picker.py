from PyQt5.QtGui import QColor

from .. import kassist_krita as krita
from ..kassist_cakewalk import BrushEngine, BrushProgram, Tool


class ColorPickerEngine(BrushEngine):
    def begin(self, context, sample, state):
        state["picked"] = False
        return state

    def apply(self, context, previous, sample, state):
        if state["picked"]:
            return
        state["picked"] = True
        x, y = round(sample.x), round(sample.y)
        red, green, blue, alpha = context.rgba(x, y)
        color = QColor(red, green, blue, alpha)
        window = krita.instance().activeWindow()
        view = window.activeView() if window else None
        setter = getattr(view, "setForeGroundColor", None)
        if setter is None:
            raise RuntimeError("Krita did not expose the active view color setter")
        setter(color)
        state["color"] = "#{:02x}{:02x}{:02x}{:02x}".format(red, green, blue, alpha)


def create_tool():
    engine = ColorPickerEngine()
    def configure(panel):
        panel.text("Click the canvas to sample its source pixel into Krita's foreground color.")
    return Tool("color-picker", "Color picker", BrushProgram(engine=engine), configure)
