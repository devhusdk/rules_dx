"""Numpy style fixture."""


def scale(values, factor=2):
    """Scale values.

    Parameters
    ----------
    values : list of int
        The values.
    factor : int
        The factor.

    Returns
    -------
    list of int
        The scaled values.
    """
    return [v * factor for v in values]
