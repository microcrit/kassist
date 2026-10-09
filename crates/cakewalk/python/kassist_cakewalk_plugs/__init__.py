import hashlib
from importlib import import_module, util
from pathlib import Path

PLUGS = {
    "blur": (".blur", "create_tool"),
    "eraser": (".eraser", "create_tool"),
    "transform": (".transform", "create_tool"),
    "warp": (".warp", "create_tool"),
    "distort": (".distort", "create_tool"),
    "smudge": (".smudge", "create_tool"),
    "clone": (".clone", "create_tool"),
    "fill": (".fill", "create_tool"),
    "bounded-fill": (".bounded_fill", "create_tool"),
    "color-picker": (".color_picker", "create_tool"),
    "stabilizer": (".stabilizer", "create_tool"),
    "lazy-stabilizer": (".lazy_stabilizer", "create_tool"),
    "bounded-shading": (".bounded_shading", "create_tool"),
    "edge-detect": (".edge_detect", "create_tool"),
    "hair-guides": (".hair_guides", "HairGuideEngine"),
    "shader": (".shader", "ShaderBrush"),
}

BRUSH_PLUGS = (
    "bounded-shading", "edge-detect", "blur", "eraser", "smudge", "clone",
    "fill", "bounded-fill", "color-picker", "stabilizer", "lazy-stabilizer",
    "transform", "warp", "distort",
)


def load(name):
    try:
        module_name, export = PLUGS[name]
    except KeyError as error:
        raise KeyError(f"unknown Cakewalk plug: {name}") from error
    return getattr(import_module(module_name, __name__), export)


def available():
    return tuple(PLUGS)


def builtin_tools():
    from ..kassist_cakewalk import Tool
    tools = []
    for name in BRUSH_PLUGS:
        tool = load(name)()
        if not isinstance(tool, Tool):
            raise TypeError(f"built-in Cakewalk plug '{name}' did not create a Tool")
        tools.append(tool)
    return tools


def user_directory(plugin_id="shared"):
    from .. import kassist_krita as krita
    safe = str(plugin_id).replace("/", "-").replace("\\", "-")
    return Path(krita.app_data_dir("cakewalk", "plugs", safe))


def discover_user(directory=None):
    root = Path(directory) if directory is not None else user_directory()
    if not root.is_dir():
        return ()
    return tuple(sorted(path.stem for path in root.glob("*.py") if not path.name.startswith("_")))


def load_user(name, directory=None):
    if not str(name).replace("-", "_").isalnum():
        raise ValueError("user plug names must contain letters, digits, '_' or '-'")
    root = Path(directory) if directory is not None else user_directory()
    path = root / (str(name) + ".py")
    if not path.is_file():
        raise FileNotFoundError(f"Cakewalk user plug is missing: {path}")
    module_id = "_cakewalk_user_" + hashlib.sha256(str(path.resolve()).encode("utf-8")).hexdigest()
    specification = util.spec_from_file_location(module_id, str(path))
    if specification is None or specification.loader is None:
        raise RuntimeError(f"Cakewalk could not create a loader for {path}")
    module = util.module_from_spec(specification)
    module.cakewalk = import_module(__package__.rsplit(".", 1)[0] + ".kassist_cakewalk")
    try:
        specification.loader.exec_module(module)
    except Exception as error:
        raise RuntimeError(f"Cakewalk could not load user plug '{name}': {error}") from error
    manifest = getattr(module, "CAKEWALK_PLUG", None)
    if not isinstance(manifest, dict):
        raise ValueError(f"user plug '{name}' needs a CAKEWALK_PLUG dictionary")
    plug_id, title, factory = manifest.get("id"), manifest.get("title"), manifest.get("factory")
    if not isinstance(plug_id, str) or not plug_id or not isinstance(title, str) or not title.strip() or not callable(factory):
        raise ValueError(f"user plug '{name}' needs string id/title and callable factory")
    return manifest


def user_tools(directory=None):
    from ..kassist_cakewalk import BrushEngine, BrushProgram, Tool
    tools = []
    for name in discover_user(directory):
        manifest = load_user(name, directory)
        value = manifest["factory"]()
        if isinstance(value, Tool):
            tools.append(value)
        elif isinstance(value, BrushProgram):
            tools.append(Tool(manifest["id"], manifest["title"], value))
        elif isinstance(value, BrushEngine):
            tools.append(Tool(manifest["id"], manifest["title"], BrushProgram(engine=value)))
        else:
            raise TypeError(f"user plug '{name}' factory must return Tool, BrushProgram, or BrushEngine")
    ids = [tool.id for tool in tools]
    if len(ids) != len(set(ids)):
        raise ValueError("user Cakewalk plugs contain duplicate tool ids")
    return tools


def listed_tools(directory=None):
    tools = builtin_tools() + user_tools(directory)
    ids = [tool.id for tool in tools]
    if len(ids) != len(set(ids)):
        raise ValueError("Cakewalk built-in and user tools contain duplicate ids")
    return tools


def write_template(name="my_brush", directory=None):
    if not str(name).replace("-", "_").isalnum():
        raise ValueError("template names must contain letters, digits, '_' or '-'")
    root = Path(directory) if directory is not None else user_directory()
    root.mkdir(parents=True, exist_ok=True)
    path = root / (str(name) + ".py")
    if path.exists():
        raise FileExistsError(f"refusing to overwrite user plug {path}")
    path.write_text('''def create():\n    return cakewalk.BrushProgram(hook=lambda sample, previous, context, state: {\n        "color": "#80c8ffff", "size": 12 + 20 * sample.pressure, "opacity": .5\n    })\n\nCAKEWALK_PLUG = {"id": "my-brush", "title": "My brush", "factory": create}\n''', encoding="utf-8")
    return path


def __getattr__(name):
    for plug in PLUGS:
        candidate = PLUGS[plug][1]
        if candidate == name:
            value = load(plug)
            globals()[name] = value
            return value
    raise AttributeError(name)
