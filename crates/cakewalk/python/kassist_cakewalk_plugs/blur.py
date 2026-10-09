from ..kassist_cakewalk import BrushEngine, BrushProgram, BrushStyle, Tool


class BlurEngine(BrushEngine):
    def __init__(self, size=24.0, strength=.35):
        self.style, self.strength = BrushStyle(size=size, opacity=1, hardness=.45), float(strength)

    def apply(self, context, previous, sample, state):
        radius = max(1, round(self.style.radius(sample.pressure) / 3))
        def blur(ctx, x, y, coverage):
            values = [ctx.rgba(x + dx, y + dy) for dx in range(-radius, radius + 1) for dy in range(-radius, radius + 1)]
            amount, index = coverage * self.strength, (y * ctx.width + x) * 4
            for channel in range(4):
                average = sum(value[channel] for value in values) / len(values)
                ctx.surface.data[index + channel] = round(ctx.surface.data[index + channel] * (1 - amount) + average * amount)
            return False
        context.surface.stroke(context, previous, sample, self.style, blur)


def create_tool():
    engine = BlurEngine()
    def configure(panel):
        panel.number("Radius", engine.style.size, 2, 180, 1, 0, "size").slider("Strength", engine.strength, .01, 1, 2, "strength")
    def dispatch(action, value):
        if action == "size": engine.style.size = float(value)
        elif action == "strength": engine.strength = float(value)
    return Tool("blur", "Blur", BrushProgram(engine=engine), configure, dispatch)
