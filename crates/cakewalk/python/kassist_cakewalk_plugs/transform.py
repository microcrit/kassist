from ..kassist_cakewalk import BrushProgram, Tool
from .warp import WarpEngine


class TransformEngine(WarpEngine):
    def __init__(self, size=56.0, strength=.72):
        super().__init__(size, strength)


def create_tool():
    engine = TransformEngine()
    def configure(panel):
        panel.number("Area", engine.style.size, 4, 400, 1, 0, "size").slider("Move amount", engine.strength, .01, 1, 2, "strength")
    def dispatch(action, value):
        if action == "size": engine.style.size = float(value)
        elif action == "strength": engine.strength = float(value)
    return Tool("transform", "Local transform", BrushProgram(engine=engine), configure, dispatch)
