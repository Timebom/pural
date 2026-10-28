from assistant.core.rules import build_default_engine
from assistant.tools.volume import VolumeControl


class Fake:
    level = 50
    def get(self): return self.level
    def set(self, n): self.level = n

v = VolumeControl(Fake())
print(v.set_volume({"level": "forty five"}).message)   # Volume 45 percent.
print(v.set_volume({"level": "zero"}).message)         # Volume 0 percent.
print(v.set_volume({"level": "max"}))                  # ok=False, follow_up set
print(build_default_engine().parse("Set the volume to forty."))
# Intent(name='set_volume', slots={'level': 'forty'}, confidence=1.0)
