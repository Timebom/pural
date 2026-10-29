import ast
import operator
import re

from assistant.core.parsing import words_to_digits
from assistant.interfaces import ToolResult

_BINARY = {
    ast.Add: operator.add, ast.Sub: operator.sub, ast.Mult: operator.mul,
    ast.Div: operator.truediv, ast.FloorDiv: operator.floordiv,
    ast.Mod: operator.mod, ast.Pow: operator.pow,
}
_UNARY = {ast.USub: operator.neg, ast.UAdd: operator.pos}

# spoken -> symbol. Order matters: "to the power of" before "times"
_SPOKEN = [
    (re.compile(r"\bto the power of\b|\^"), "**"),
    (re.compile(r"\bdivided by\b|\bover\b"), "/"),
    (re.compile(r"\bmultiplied by\b|\btimes\b"), "*"),
    (re.compile(r"(?<=\d)\s*x\s*(?=\d)"), "*"),
    (re.compile(r"\bplus\b"), "+"),
    (re.compile(r"\bminus\b"), "-"),
    (re.compile(r"\bmodulo\b|\bmod\b"), "%"),
]


def _eval(node: ast.AST) -> float:
    if isinstance(node, ast.Expression):
        return _eval(node.body)
    if (isinstance(node, ast.Constant) and isinstance(node.value, (int, float)) and not isinstance(node.value, bool)):
        return node.value
    if isinstance(node, ast.BinOp) and type(node.op) in _BINARY:
        left, right = _eval(node.left), _eval(node.right)
        if isinstance(node.op, ast.Pow) and abs(right) > 100:
            raise ValueError("exponent too large")
        return _BINARY[type(node.op)](left, right)
    if isinstance(node, ast.UnaryOp) and type(node.op) in _UNARY:
        return _UNARY[type(node.op)](_eval(node.operand))
    raise ValueError("unsupported expression")


def calculate(slots: dict[str,str]) -> ToolResult:
    expr = slots.get("expression", "")
    expr = words_to_digits(expr)
    for pattern, symbol in _SPOKEN:
        expr = pattern.sub(f" {symbol} ", expr)
    try:
        value = _eval(ast.parse(expr.strip(), mode="eval"))
    except ZeroDivisionError:
        return ToolResult(False, "I can't divide by zero.")
    except (SyntaxError, ValueError, ArithmeticError):
        return ToolResult(False, "Sorry, I couldn't work that out.")
    shown = f"{value:.6g}" if isinstance(value, float) else str(value)
    return ToolResult(True, f"The answer is {shown}.", {"value": value})
