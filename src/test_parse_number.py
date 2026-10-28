from assistant.core.parsing import parse_number

cases = {
    "40": 40, "forty": 40, "forty five": 45, "twenty-five": 25,
    "fifty percent": 50, "40%": 40, "a hundred": 100, "one hundred": 100,
    "hundred": 100, "five": 5, "zero": 0, "the volume to twenty": 20,
    "twenty twenty": 20, "max": None, "": None,
}
for text, want in cases.items():
    got = parse_number(text)
    print("OK " if got == want else "BAD", repr(text), "->", got)
