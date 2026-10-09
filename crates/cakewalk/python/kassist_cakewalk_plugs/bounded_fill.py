from collections import deque

from PyQt5.QtGui import QColor

from ..kassist_cakewalk import BrushEngine, BrushProgram, Tool


class BoundedFillEngine(BrushEngine):
    def __init__(self, color="#5ea9ffff", tolerance=.12, alpha_floor=.05):
        self.color = color
        self.tolerance = float(tolerance)
        self.alpha_floor = float(alpha_floor)

    def begin(self, context, sample, state):
        state["filled"] = False
        return state

    def apply(self, context, previous, sample, state):
        if state["filled"]:
            return
        state["filled"] = True
        start_x, start_y = round(sample.x), round(sample.y)
        if not (0 <= start_x < context.width and 0 <= start_y < context.height):
            return
        seed = context.rgba(start_x, start_y)
        if seed[3] / 255.0 < self.alpha_floor:
            return
        color = QColor(self.color)
        if not color.isValid():
            raise ValueError("invalid bounded-fill color")
        def matches(pixel):
            return max(abs(pixel[index] - seed[index]) for index in range(4)) <= 255 * self.tolerance
        queue, seen = deque([(start_x, start_y)]), set()
        while queue:
            x, y = queue.popleft()
            if (x, y) in seen:
                continue
            seen.add((x, y))
            pixel = context.rgba(x, y)
            if not (0 <= x < context.width and 0 <= y < context.height) or pixel[3] / 255.0 < self.alpha_floor or not matches(pixel):
                continue
            context.surface.blend(x, y, color.red(), color.green(), color.blue(), color.alpha() / 255.0)
            queue.extend(((x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)))


def create_tool():
    engine = BoundedFillEngine()
    def configure(panel):
        panel.color_field("Fill color", engine.color, "color").slider("Tolerance", engine.tolerance, 0, 1, 2, "tolerance").slider("Alpha boundary", engine.alpha_floor, 0, 1, 2, "alpha_floor")
    def dispatch(action, value):
        if action == "color": engine.color = str(value)
        elif action == "tolerance": engine.tolerance = float(value)
        elif action == "alpha_floor": engine.alpha_floor = float(value)
    return Tool("bounded-fill", "Bounded fill", BrushProgram(engine=engine), configure, dispatch)
