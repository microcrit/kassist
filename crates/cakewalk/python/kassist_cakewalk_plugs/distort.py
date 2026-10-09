from ..kassist_cakewalk import BrushProgram, Tool
from .warp import WarpEngine


class DistortEngine(WarpEngine):
    def apply(self, context, previous, sample, state):
        original = self.strength
        self.strength *= 1.0 + abs(sample.rotation) / 720.0
        try: super().apply(context, previous, sample, state)
        finally: self.strength = original


def create_tool():
    engine = DistortEngine(size=64.0, strength=.35)
    def configure(panel):
        panel.number("Area", engine.style.size, 4, 400, 1, 0, "size").slider("Distortion", engine.strength, .01, 1, 2, "strength")
    def dispatch(action, value):
        if action == "size": engine.style.size = float(value)
        elif action == "strength": engine.strength = float(value)
    return Tool("distort", "Rotation distort", BrushProgram(engine=engine), configure, dispatch)
