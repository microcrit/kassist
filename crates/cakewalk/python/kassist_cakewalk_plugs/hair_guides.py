import math
import random

from PyQt5.QtCore import QPointF

from ..kassist_cakewalk import BrushStyle, InputSample


class HairGuideEngine:
    def render(self, context, density, length, direction, gravity, color, pressure=1.0, seed="cakewalk-hair"):
        spacing = max(3, round(18 / max(.1, float(density))))
        direction, gravity = math.radians(float(direction)), max(0, min(1, float(gravity)))
        dx, dy = math.cos(direction) * (1 - gravity), math.sin(direction) * (1 - gravity) + gravity
        scale = math.hypot(dx, dy) or 1
        dx, dy, count, randomizer = dx / scale, dy / scale, 0, random.Random(str(seed))
        for y in range(spacing // 2, context.height, spacing):
            for x in range(spacing // 2, context.width, spacing):
                if context.alpha(x, y) < .095:
                    continue
                sway = randomizer.uniform(-.24, .24)
                local_x, local_y = dx * math.cos(sway) - dy * math.sin(sway), dx * math.sin(sway) + dy * math.cos(sway)
                strand = float(length) * (.45 + .55 * pressure) * randomizer.uniform(.72, 1.18)
                point = QPointF(x, y)
                start = InputSample("generated", point, point, context.transform, pressure)
                end = InputSample("generated", point, QPointF(x + local_x * strand, y + local_y * strand), context.transform, pressure)
                context.surface.stroke(context, start, end, BrushStyle(color, max(.6, 1.4 * pressure), 1, .85, pressure_size=False, pressure_opacity=False))
                count += 1
        return count
