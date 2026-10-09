from dataclasses import replace

from PyQt5.QtCore import QPointF

from ..kassist_cakewalk import BrushEngine, BrushProgram, BrushStyle, Tool


class LazyStabilizerEngine(BrushEngine):
    def __init__(self, color="#20252eff", size=14.0, opacity=.8, drag=24.0):
        self.style = BrushStyle(color, size, opacity, .72)
        self.drag = float(drag)

    def begin(self, context, sample, state):
        state["tip"] = sample
        return state

    def apply(self, context, previous, sample, state):
        prior = state["tip"]
        dx, dy = sample.x - prior.x, sample.y - prior.y
        distance = (dx * dx + dy * dy) ** .5
        if distance <= self.drag:
            return
        move = (distance - self.drag) / distance
        point = QPointF(prior.x + dx * move, prior.y + dy * move)
        current = replace(sample, document_position=point)
        context.surface.stroke(context, prior, current, self.style)
        state["tip"] = current


def create_tool():
    engine = LazyStabilizerEngine()
    def configure(panel):
        panel.color_field("Color", engine.style.color, "color").number("Size", engine.style.size, 2, 300, 1, 0, "size").slider("Opacity", engine.style.opacity, .01, 1, 2, "opacity").number("Lazy drag", engine.drag, 1, 160, 1, 0, "drag")
    def dispatch(action, value):
        if action == "color": engine.style.color = str(value)
        elif action == "size": engine.style.size = float(value)
        elif action == "opacity": engine.style.opacity = float(value)
        elif action == "drag": engine.drag = float(value)
    return Tool("lazy-stabilizer", "Lazy stabilizer", BrushProgram(engine=engine), configure, dispatch)
