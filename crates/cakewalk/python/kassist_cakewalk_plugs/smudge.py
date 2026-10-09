from ..kassist_cakewalk import BrushEngine, BrushProgram, BrushStyle, Tool


class SmudgeEngine(BrushEngine):
    def __init__(self, size=42.0, strength=.55):
        self.style = BrushStyle(size=size, opacity=1, hardness=.5)
        self.strength = float(strength)

    def apply(self, context, previous, sample, state):
        if previous is None:
            return
        dx, dy = (sample.x - previous.x) * self.strength, (sample.y - previous.y) * self.strength
        def smudge(ctx, x, y, coverage):
            source_x, source_y = round(x - dx * coverage), round(y - dy * coverage)
            if not (0 <= source_x < ctx.width and 0 <= source_y < ctx.height):
                return False
            destination = (y * ctx.width + x) * 4
            source = (source_y * ctx.width + source_x) * 4
            amount = coverage * self.strength
            for channel in range(4):
                ctx.surface.data[destination + channel] = round(ctx.surface.data[destination + channel] * (1 - amount) + ctx.surface.data[source + channel] * amount)
            return False
        context.surface.stroke(context, previous, sample, self.style, smudge)


def create_tool():
    engine = SmudgeEngine()
    def configure(panel):
        panel.number("Size", engine.style.size, 4, 400, 1, 0, "size").slider("Smudge", engine.strength, .01, 1, 2, "strength")
    def dispatch(action, value):
        if action == "size": engine.style.size = float(value)
        elif action == "strength": engine.strength = float(value)
    return Tool("smudge", "Smudge", BrushProgram(engine=engine), configure, dispatch)
