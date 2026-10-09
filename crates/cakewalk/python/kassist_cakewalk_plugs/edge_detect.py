from ..kassist_cakewalk import BrushEngine, BrushProgram, BrushStyle, Tool


class EdgeDetectEngine(BrushEngine):
    def __init__(self, color="#0d1117", size=14.0, opacity=.8, threshold=.16):
        self.style, self.threshold = BrushStyle(color, size, opacity, .6), float(threshold)

    def apply(self, context, previous, sample, state):
        context.surface.stroke(context, previous, sample, self.style, lambda ctx, x, y, _coverage: ctx.edge(x, y)[0] >= self.threshold)


def create_tool():
    engine = EdgeDetectEngine()
    def configure(panel):
        panel.color_field("Color", engine.style.color, "color").number("Size", engine.style.size, 2, 220, 1, 0, "size").slider("Opacity", engine.style.opacity, .01, 1, 2, "opacity").slider("Edge threshold", engine.threshold, 0, 1, 2, "threshold")
    def dispatch(action, value):
        if action == "color": engine.style.color = str(value)
        elif action == "size": engine.style.size = float(value)
        elif action == "opacity": engine.style.opacity = float(value)
        elif action == "threshold": engine.threshold = float(value)
    return Tool("edge-detect", "Edge-detect ink", BrushProgram(engine=engine), configure, dispatch)
