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
_HYPHEN_BETWEEN_LETTERS = re.compile(r"(?<=[a-z])-(?=[a-z])")
NUMBER_WORDS = frozenset(_UNITS) | frozenset(_TENS) | {"hundred"}


def _read_number(tokens: list[str], i: int) -> tuple[int, int] | None:
    """Read one number (0-999) at tokens[i]. Returns (value, index after it), or None.
    'forty five' -> 45, 'a hundred' -> 100, 'two hundred and five' -> 205, '40' -> 40."""

    def at(k: int) -> str:
        return tokens[k] if k < len(tokens) else ""

    def below_100(k: int) -> tuple[int, int] | None:
        word = at(k)
        if word in _TENS:
            unit = _UNITS.get(at(k + 1), 0)
            if 1 <= unit <= 9:
                return _TENS[word] + unit, k + 2          # "forty five"
            return _TENS[word], k + 1
        if word in _UNITS:
            return _UNITS[word], k + 1
        if _DIGITS.match(word):
            return round(float(word)), k + 1
        return None

    first = below_100(i)
    if first is not None and first[0] < 10 and at(first[1]) == "hundred":   # "two hundred ..."
        value, j = first[0] * 100, first[1] + 1
    elif at(i) in ("a", "an") and at(i + 1) == "hundred":                  # "a hundred ..."
        value, j = 100, i + 2
    elif at(i) == "hundred":                                               # "hundred ..."
        value, j = 100, i + 1
    else:
        return first                                                       # plain 0-99, or None
    k = j + 1 if at(j) == "and" else j                                     # "hundred and five"
    rest = below_100(k)
    if rest is not None and rest[0] < 100:
        return value + rest[0], rest[1]
    return value, j                                                        # don't consume a dangling "and"


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
    for i in range(len(tokens)):
        found = _read_number(tokens, i)
        if found is not None:
            return found[0]
    return None


def words_to_digits(text: str) -> str:
    """'seven plus nine' -> '7 plus 9'. Digits, symbols and minus signs are left alone."""
    tokens = _HYPHEN_BETWEEN_LETTERS.sub(" ", text.lower()).split()
    out: list[str] = []
    i = 0
    while i < len(tokens):
        found = None
        if tokens[i] in NUMBER_WORDS or tokens[i] in ("a", "an"):
            found = _read_number(tokens, i)
        if found is None:
            out.append(tokens[i])
            i += 1
        else:
            out.append(str(found[0]))
            i = found[1] # skip exactly the tokens the number used
    return " ".join(out)


def parse_duration(text: str) -> int | None:
    """'two and a half hours' -> 9000. Returns seconds, or None if no duration found."""
    tokens = text.lower().replace("-", " ").split()
    total = 0.0
    number: float | None = None
    found_unit = False
    last_unit = 0 # seconds in the most recent unit word

    for i, tok in enumerate(tokens):
        if tok in _SECONDS_PER:
            last_unit = _SECONDS_PER[tok]
            total += (1.0 if number is None else number) * last_unit
            number = None
            found_unit = True
        elif tok == "half":
            number = (number or 0.0) + 0.5 # "half an hour", "two and half hours"
        elif tok in ("a", "an"):
            # "an hour" -> 1. Before "half", stay out of the way: "half" adds the 0.5 itself.
            if number is None and tokens[i + 1:i + 2] != ["half"]:
                number = 1.0
        else:
            value = _as_number(tok)
            if value is None:
                continue                            # ignore filler: "and", "for", ...
            is_tens_then_unit = (
                number is not None and number >= 20 and number % 10 == 0
                and tok in _UNITS and value < 10
            )
            number = number + value if is_tens_then_unit and number is not None else value  # "twenty five"

    if number == 0.5 and last_unit:                 # "an hour and a half": half of the last unit
        total += 0.5 * last_unit

    return round(total) if found_unit and total > 0 else None
