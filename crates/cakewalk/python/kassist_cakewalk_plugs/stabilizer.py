from dataclasses import replace

from PyQt5.QtCore import QPointF

from ..kassist_cakewalk import BrushEngine, BrushProgram, BrushStyle, Tool


class StabilizerEngine(BrushEngine):
    def __init__(self, color="#20252eff", size=14.0, opacity=.8, smoothing=.68):
        self.style = BrushStyle(color, size, opacity, .72)
        self.smoothing = float(smoothing)

    def begin(self, context, sample, state):
        state["smoothed"] = sample
        return state

    def apply(self, context, previous, sample, state):
        prior = state["smoothed"]
        amount = 1.0 - self.smoothing
        point = QPointF(prior.x + (sample.x - prior.x) * amount, prior.y + (sample.y - prior.y) * amount)
        current = replace(sample, document_position=point, pressure=prior.pressure + (sample.pressure - prior.pressure) * amount)
        context.surface.stroke(context, prior, current, self.style)
        state["smoothed"] = current


def create_tool():
    engine = StabilizerEngine()
    def configure(panel):
        panel.color_field("Color", engine.style.color, "color").number("Size", engine.style.size, 2, 300, 1, 0, "size").slider("Opacity", engine.style.opacity, .01, 1, 2, "opacity").slider("Smoothing", engine.smoothing, 0, .95, 2, "smoothing")
    def dispatch(action, value):
        if action == "color": engine.style.color = str(value)
        elif action == "size": engine.style.size = float(value)
        elif action == "opacity": engine.style.opacity = float(value)
        elif action == "smoothing": engine.smoothing = float(value)
    return Tool("stabilizer", "Stabilized ink", BrushProgram(engine=engine), configure, dispatch)
