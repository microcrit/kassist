from ..kassist_cakewalk import BrushEngine, BrushProgram, BrushStyle, Tool


class WarpEngine(BrushEngine):
    def __init__(self, size=48.0, strength=.45):
        self.style, self.strength = BrushStyle(size=size, opacity=1, hardness=.5), float(strength)

    def apply(self, context, previous, sample, state):
        if previous is None: return
        dx, dy = (sample.x - previous.x) * self.strength, (sample.y - previous.y) * self.strength
        def warp(ctx, x, y, coverage):
            red, green, blue, alpha = ctx.rgba(round(x - dx * coverage), round(y - dy * coverage))
            ctx.surface.blend(x, y, red, green, blue, alpha / 255.0)
            return False
        context.surface.stroke(context, previous, sample, self.style, warp)


def create_tool():
    engine = WarpEngine()
    def configure(panel):
        panel.number("Size", engine.style.size, 4, 400, 1, 0, "size").slider("Strength", engine.strength, .01, 1, 2, "strength")
    def dispatch(action, value):
        if action == "size": engine.style.size = float(value)
        elif action == "strength": engine.strength = float(value)
    return Tool("warp", "Warp", BrushProgram(engine=engine), configure, dispatch)
