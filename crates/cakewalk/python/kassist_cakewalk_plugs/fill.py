from collections import deque

from ..kassist_cakewalk import BrushEngine, BrushProgram, Tool


class FillEngine(BrushEngine):
    def __init__(self, color="#5ea9ffff", tolerance=.12):
        self.color, self.tolerance = color, float(tolerance)

    def begin(self, context, sample, state):
        state["filled"] = False
        return state

    def apply(self, context, previous, sample, state):
        if state["filled"]:
            return
        state["filled"] = True
        x, y = round(sample.x), round(sample.y)
        if not (0 <= x < context.width and 0 <= y < context.height):
            return
        seed = context.rgba(x, y)
        from PyQt5.QtGui import QColor
        color = QColor(self.color)
        if not color.isValid():
            raise ValueError("invalid fill color")
        def close(pixel):
            return max(abs(pixel[index] - seed[index]) for index in range(4)) <= 255 * self.tolerance
        queue, seen = deque([(x, y)]), set()
        while queue:
            point = queue.popleft()
            if point in seen:
                continue
            seen.add(point)
            px, py = point
            if not (0 <= px < context.width and 0 <= py < context.height) or not close(context.rgba(px, py)):
                continue
            context.surface.blend(px, py, color.red(), color.green(), color.blue(), color.alpha() / 255.0)
            queue.extend(((px - 1, py), (px + 1, py), (px, py - 1), (px, py + 1)))


def create_tool():
    engine = FillEngine()
    def configure(panel):
        panel.color_field("Fill color", engine.color, "color").slider("Tolerance", engine.tolerance, 0, 1, 2, "tolerance")
    def dispatch(action, value):
        if action == "color": engine.color = str(value)
        elif action == "tolerance": engine.tolerance = float(value)
    return Tool("fill", "Contiguous fill", BrushProgram(engine=engine), configure, dispatch)
