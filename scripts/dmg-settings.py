from pathlib import Path

app = Path(defines["app"])
files = [str(app), "assets/First-launch.txt"]
symlinks = {"Applications": "/Applications"}
icon_locations = {app.name: (145, 187), "Applications": (488, 187), "First-launch.txt": (318, 335)}
window_rect = ((180, 140), (640, 400))
icon_size = 72
text_size = 13
background = defines["background"]
icon = str(app / "Contents/Resources/AppIcon.icns")
