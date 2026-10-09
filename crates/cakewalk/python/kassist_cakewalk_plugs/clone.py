from ..kassist_cakewalk import BrushEngine, BrushProgram, BrushStyle, Tool


class CloneEngine(BrushEngine):
    def __init__(self, size=32.0, opacity=.85, offset_x=48, offset_y=48):
        self.style = BrushStyle(size=size, opacity=opacity, hardness=.62)
        self.offset_x, self.offset_y = int(offset_x), int(offset_y)

    def apply(self, context, previous, sample, state):
        def clone(ctx, x, y, coverage):
            red, green, blue, alpha = ctx.rgba(x - self.offset_x, y - self.offset_y)
            ctx.surface.blend(x, y, red, green, blue, coverage * alpha / 255.0 * self.style.rgba(sample.pressure)[3])
            return False
        context.surface.stroke(context, previous, sample, self.style, clone)


def create_tool():
    engine = CloneEngine()
    def configure(panel):
        panel.number("Size", engine.style.size, 2, 300, 1, 0, "size").slider("Opacity", engine.style.opacity, .01, 1, 2, "opacity").number("Source X offset", engine.offset_x, -2000, 2000, 1, 0, "offset_x").number("Source Y offset", engine.offset_y, -2000, 2000, 1, 0, "offset_y")
    def dispatch(action, value):
        if action == "size": engine.style.size = float(value)
        elif action == "opacity": engine.style.opacity = float(value)
        elif action == "offset_x": engine.offset_x = int(value)
        elif action == "offset_y": engine.offset_y = int(value)
    return Tool("clone", "Offset clone", BrushProgram(engine=engine), configure, dispatch)
