GLSL_450 = "#version 450\n"


class ShaderBrush:
    def __init__(self, source, fallback=None, label="Cakewalk shader brush"):
        self.source = GLSL_450 + source if not source.lstrip().startswith("#version") else source
        self.fallback = fallback
        self.label = label

    def compile(self, device):
        try:
            from . import cakewalk_gpu
        except ImportError as error:
            raise RuntimeError("Cakewalk GPU backend is unavailable; package the optional WGPU extension") from error
        return cakewalk_gpu.shader_module(device, self.label, self.source)

    def cpu_fallback(self):
        if self.fallback is None:
            raise RuntimeError("shader brush has no CPU fallback")
        return self.fallback
