from ..kassist_cakewalk import BrushEngine, BrushProgram, BrushStyle, Tool


class EraserEngine(BrushEngine):
    def __init__(self, size=32.0, opacity=1.0):
        self.style = BrushStyle(size=size, opacity=opacity)

    def apply(self, context, previous, sample, state):
        def erase(ctx, x, y, coverage):
            index = (y * ctx.width + x) * 4
            ctx.surface.data[index + 3] = round(ctx.surface.data[index + 3] * (1 - coverage * self.style.rgba(sample.pressure)[3]))
            return False
        context.surface.stroke(context, previous, sample, self.style, erase)


def create_tool():
    engine = EraserEngine()
    def configure(panel):
        panel.number("Size", engine.style.size, 2, 300, 1, 0, "size").slider("Opacity", engine.style.opacity, .01, 1, 2, "opacity")
    def dispatch(action, value):
        if action == "size": engine.style.size = float(value)
        elif action == "opacity": engine.style.opacity = float(value)
    return Tool("eraser", "Eraser", BrushProgram(engine=engine), configure, dispatch)
