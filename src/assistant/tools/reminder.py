import json
from datetime import datetime
from pathlib import Path
from typing import Any


class ReminderManager:
    def __init__(
        self,
        db_path: str = "../memory/reminders.json",
        operation: str | None = None,
        **kwargs,
    ) -> None:
        self.db_path = Path(db_path)
        if not self.db_path.exists():
            Path(db_path.rsplit("/", maxsplit=1)[0]).mkdir(parents=True, exist_ok=True)
            with open(self.db_path, "w") as f:
                json.dump([], f)

        self.operation = operation
        self.args = kwargs
        self.results = self.execute_operation()

    def _load(self) -> list[dict[str, Any]]:
        with open(self.db_path, "r") as f:
            return json.load(f)

    def _save(self, data: list[dict[str, Any]]) -> None:
        with open(self.db_path, "w") as f:
            json.dump(data, f, indent=2)

    def _to_ts(self, iso: str) -> int:
        return int(datetime.fromisoformat(iso).timestamp())

    def _from_ts(self, ts: int) -> str:
        return datetime.fromtimestamp(ts).strftime("%d-%m-%Y %H:%M")

    def add_reminder(self, task: str, time: str) -> str:
        """Add a reminder. `time` must be ISO format (YYYY-MM-DDTHH:MM:SS…)."""
        reminders = self._load()
        reminders.append(
            {
                "task": task,
                "timestamp": self._to_ts(time),
                "status": "pending",
            }
        )
        self._save(reminders)
        return f"Set reminder for '{task}' at {datetime.fromisoformat(time)}"

    def get_reminder(self, task: str) -> str | None:
        for obj in self._load():
            if obj["task"] == task:
                return (
                    f"{obj['task']} ({self._from_ts(obj['timestamp'])}) "
                    f"[{obj['status'].capitalize()}]"
                )
        return None

    def list_reminders(self) -> list[str] | str:
        reminders = self._load()
        if not reminders:
            return "No reminders found"
        return [
            f"{obj['task']} ({self._from_ts(obj['timestamp'])}) "
            f"[{obj['status'].capitalize()}]"
            for obj in reminders
        ]

    def update_reminder(
        self,
        task: str,
        new_task: str | None = None,
        time: str | None = None,
        status: str | None = None,
    ) -> str:
        if not any([new_task, time, status]):
            return f"Give proper update arguments for updation for {task}"

        status_list = {"completed", "pending", "overlaps"}
        reminders = self._load()

        for obj in reminders:
            if obj.get("task") == task:
                if new_task:
                    obj["task"] = new_task
                if time:
                    obj["timestamp"] = self._to_ts(time)
                if status and status in status_list:
                    obj["status"] = status
                break

        self._save(reminders)
        return f"{task} is updated successfully"

    def completed_reminder(self, task: str) -> str:
        reminders = self._load()
        for obj in reminders:
            if obj.get("task") == task:
                obj["status"] = "completed"
                break
        self._save(reminders)
        return f"{task} is completed successfully"

    def delete_reminder(self, task: str) -> str:
        reminders = self._load()
        reminders = [obj for obj in reminders if obj.get("task") != task]
        self._save(reminders)
        return f"{task} is deleted successfully"

    def execute_operation(self):
        if not self.operation:
            return None

        task = self.args.get("task", "")
        new_task = self.args.get("new_task")
        time = self.args.get("time")
        status = self.args.get("status")

        ops = {
            "add": lambda: self.add_reminder(task, time) if time else None,
            "update": lambda: self.update_reminder(task, new_task, time, status),
            "complete": lambda: self.completed_reminder(task),
            "delete": lambda: self.delete_reminder(task),
            "get": lambda: self.get_reminder(task),
            "retrieve": lambda: self.list_reminders(),
        }

        handler = ops.get(self.operation)
        return handler() if handler else None
