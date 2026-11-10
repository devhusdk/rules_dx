def appimage_name_errors(name, desktop, icon):
    """Returns the metadata names that do not match the application name."""
    errors = []
    if desktop != name + ".desktop":
        errors.append("desktop file '" + desktop + "' must be named " + name + ".desktop")
    if icon not in [name + ".png", name + ".svg"]:
        errors.append("icon '" + icon + "' must be named " + name + ".png or " + name + ".svg")
    return errors
