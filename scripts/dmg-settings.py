# dmgbuild's settings for F2's DMG (see release-macos.sh): F2 and
# Applications side by side over render-dmg-background.swift's arrow.
# Given with -D: app (F2.app's path) and background (its .png; the @2x
# beside it is found too).
import os.path

app = defines["app"]  # noqa: F821 (dmgbuild defines it)
background = defines["background"]  # noqa: F821

format = "UDZO"
files = [app]
symlinks = {"Applications": "/Applications"}
icon = os.path.join(app, "Contents/Resources/AppIcon.icns")
# (Not hide_extensions: it writes Finder information onto F2.app, after
# it's signed, which `codesign --strict` refuses. Finder hides ".app"
# anyway.)

# (The height takes in the title bar: 400 under it, the background's.)
window_rect = ((200, 160), (640, 432))
default_view = "icon-view"
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
icon_size = 128
text_size = 13
icon_locations = {os.path.basename(app): (160, 175), "Applications": (480, 175)}
