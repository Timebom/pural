from src.assistant.interfaces import Intent, ToolResult

i = Intent("set_timer", {"duration": "5 minutes"})
print(i)
print(Intent("get_time") == Intent("get_time"))

try:
    i.name = "other"
except Exception as e:
    print(type(e).__name__)
