from ..kassist_cakewalk import BrushEngine, BrushProgram, BrushStyle, Tool


class BoundedShadingEngine(BrushEngine):
    def __init__(self, color="#29364f", size=72.0, opacity=.16, edge_guard=.55, alpha_floor=.05):
        self.style = BrushStyle(color, size, opacity, .35)
        self.edge_guard, self.alpha_floor = float(edge_guard), float(alpha_floor)

    def apply(self, context, previous, sample, state):
        context.surface.stroke(
            context, previous, sample, self.style,
            lambda ctx, x, y, _coverage: ctx.alpha(x, y) >= self.alpha_floor and ctx.edge(x, y)[0] <= self.edge_guard,
        )


def create_tool():
    engine = BoundedShadingEngine()
    def configure(panel):
        panel.color_field("Color", engine.style.color, "color").number("Size", engine.style.size, 2, 300, 1, 0, "size").slider("Opacity", engine.style.opacity, .01, 1, 2, "opacity").slider("Edge guard", engine.edge_guard, 0, 1, 2, "edge_guard")
    def dispatch(action, value):
        if action == "color": engine.style.color = str(value)
        elif action == "size": engine.style.size = float(value)
        elif action == "opacity": engine.style.opacity = float(value)
        elif action == "edge_guard": engine.edge_guard = float(value)
    return Tool("bounded-shade", "Bounded shading", BrushProgram(engine=engine), configure, dispatch)
