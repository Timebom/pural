import re

_UNITS = {
    "zero": 0, "one": 1, "two": 2, "three": 3, "four": 4, "five": 5,
    "six": 6, "seven": 7, "eight": 8, "nine": 9, "ten": 10, "eleven": 11,
    "twelve": 12, "thirteen": 13, "fourteen": 14, "fifteen": 15,
    "sixteen": 16, "seventeen": 17, "eighteen": 18, "nineteen": 19,
}
_TENS = {
    "twenty": 20, "thirty": 30, "forty": 40, "fifty": 50,
    "sixty": 60, "seventy": 70, "eighty": 80, "ninety": 90,
}
_SECONDS_PER = {
    "second": 1, "seconds": 1, "sec": 1, "secs": 1,
    "minute": 60, "minutes": 60, "min": 60, "mins": 60,
    "hour": 3600, "hours": 3600, "hr": 3600, "hrs": 3600,
}
_DIGITS = re.compile(r"^\d+(?:\.\d+)?$")
NUMBER_WORDS = frozenset(_UNITS) | frozenset(_TENS) | {"hundred"}

def _as_number(token: str) -> float | None:
    """'5' -> 5.0, '2.5' -> 2.5, 'five' -> 5.0, 'twenty'-> 20.0, else None"""
    if token in _UNITS:
        return float(_UNITS[token])
    if token in _TENS:
        return float(_TENS[token])
    if _DIGITS.match(token):
        return float(token)
    return None


def parse_number(text: str) -> int | None:
    """'40' -> 40, 'forty five' -> 45, 'a hundred' -> 100, '40%' -> 40.
    Finds the first number in the text. Returns None if there isn't one."""
    tokens = text.lower().replace("-", " ").replace("%", " ").split()
    value: int | None = None

    for tok in tokens:
        if tok == "hundred":
            if value is None or 1 <= value <= 9:      # "hundred", "one hundred", not "forty hundred"
                value = (1 if value is None else value) * 100
                continue
            break

        number = _as_number(tok)
        if number is None:
            if value is not None:
                break                                  # the number has ended
            continue                                   # still searching: skip filler words

        n = round(number)
        if value is None:
            value = n
        elif 20 <= value < 100 and value % 10 == 0 and tok in _UNITS and n < 10:
            value += n                                 # "twenty" + "five"
        else:
            break                                      # a second, separate number

    return value


def parse_duration(text: str) -> int | None:
    """'two and a half hours' -> 9000. Returns seconds, or None if no duration found."""
    tokens = text.lower().replace("-", " ").split()
    total = 0.0
    number: float | None = None
    found_unit = False

    for i, tok in enumerate(tokens):
        if tok in _SECONDS_PER:
            total += (1.0 if number is None else number) * _SECONDS_PER[tok]
            number = None
            found_unit = True
        elif tok == "half":
            number = (number or 0.0) + 0.5
        elif tok in ("a", "an"):
            # "an hour" -> 1. But before "half", stay out of the way:
            # the "half" branch adds the 0.5 itself.
            if number is None and tokens[i + 1:i + 2] != ["half"]:
                number = 1.0
        else:
            value = _as_number(tok)
            if value is None:
                continue
            is_tens_then_unit = (
                number is not None and number >= 20 and number % 10 == 0 and tok in _UNITS and value < 10
            )
            number = number + value if is_tens_then_unit and (number is not None) else value
    return round(total) if found_unit and total > 0 else None
