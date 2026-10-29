import logging

import httpx

from assistant.interfaces import FollowUp, ToolResult

log = logging.getLogger(__name__)

GEOCODE_URL = "https://geocoding-api.open-meteo.com/v1/search"
FORECAST_URL = "https://api.open-meteo.com/v1/forecast"

# WMO weather codes as noun phrases that read well in "with <sky>" and "expect <sky>".
_SKY = {
    0: "clear skies", 1: "mostly clear skies", 2: "partly cloudy skies", 3: "overcast skies",
    45: "fog", 48: "freezing fog",
    51: "light drizzle", 53: "drizzle", 55: "heavy drizzle",
    56: "light freezing drizzle", 57: "freezing drizzle",
    61: "light rain", 63: "rain", 65: "heavy rain",
    66: "light freezing rain", 67: "freezing rain",
    71: "light snow", 73: "snow", 75: "heavy snow", 77: "snow grains",
    80: "light rain showers", 81: "rain showers", 82: "heavy rain showers",
    85: "light snow showers", 86: "heavy snow showers",
    95: "a thunderstorm", 96: "a thunderstorm with hail", 99: "a heavy thunderstorm with hail",
}
_DAYS = {"today", "tomorrow"}
_NOISE = {"now", "right", "currently", "please"}
_LEADING = {"in", "at", "for", "the", "of", "like", "weather", "forecast"}


def _split_request(text: str) -> tuple[str, str | None]:
    """'in mumbai tomorrow' -> ('mumbai', 'tomorrow'); 'for tomorrow' -> ('', 'tomorrow')."""
    day = None
    words = []
    for word in text.lower().split():
        if word in _DAYS:
            day = word
        elif word not in _NOISE:
            words.append(word)
    while words and words[0] in _LEADING:
        words.pop(0)
    return " ".join(words), day


def _pick(results: list[dict], hint: str) -> dict:
    """First result whose country/region matches the words after the city; else the first."""
    if hint:
        for r in results:
            labels = (r.get("country", ""), r.get("country_code", ""), r.get("admin1", ""))
            if any(l and (l.lower() in hint or hint in l.lower()) for l in labels):
                return r
    return results[0]


def _deg(value: float) -> str:
    n = round(value)
    return f"minus {abs(n)}" if n < 0 else str(n)


class WeatherTool:
    def __init__(self, default_location: str = "", timeout: float = 8.0,
                 client: httpx.Client | None = None) -> None:
        self._default = default_location.strip().lower()
        self._client = client or httpx.Client(timeout=timeout)
        self._geo_cache: dict[str, dict] = {}

    def _geocode(self, place: str) -> dict | None:
        if place in self._geo_cache:
            return self._geo_cache[place]
        words = place.split()
        for end in range(len(words), max(len(words) - 3, 0), -1):     # at most 3 attempts
            name, hint = " ".join(words[:end]), " ".join(words[end:])
            response = self._client.get(GEOCODE_URL, params={
                "name": name, "count": 5, "language": "en", "format": "json"})
            response.raise_for_status()
            results = response.json().get("results") or []
            if results:
                self._geo_cache[place] = _pick(results, hint)
                return self._geo_cache[place]
        return None

    def _fetch(self, geo: dict, day: str) -> dict:
        params = {"latitude": geo["latitude"], "longitude": geo["longitude"],
                  "timezone": "auto"}
        if day == "tomorrow":
            params["daily"] = "weather_code,temperature_2m_max,temperature_2m_min,precipitation_sum"
            params["forecast_days"] = 2
        else:
            params["current"] = ("temperature_2m,apparent_temperature,weather_code,"
                                 "relative_humidity_2m,wind_speed_10m,precipitation")
        response = self._client.get(FORECAST_URL, params=params)
        response.raise_for_status()
        return response.json()

    def get_weather(self, slots: dict[str, str]) -> ToolResult:
        place, spoken_day = _split_request(slots.get("location", ""))
        day = spoken_day or slots.get("day", "today")
        if day not in _DAYS:
            day = "today"
        place = place or self._default
        ask = FollowUp("get_weather", "location", {"day": day})
        if not place:
            return ToolResult(False, "Which city?", follow_up=ask)
        try:
            geo = self._geocode(place)
            if geo is None:
                return ToolResult(False, f"I couldn't find a place called {place}. Which city?",
                                  follow_up=ask)
            payload = self._fetch(geo, day)
            name = geo["name"]
            if day == "tomorrow":
                daily = payload["daily"]
                sky = _SKY.get(daily["weather_code"][1], "unsettled weather")
                hi, lo = daily["temperature_2m_max"][1], daily["temperature_2m_min"][1]
                return ToolResult(
                    True,
                    f"Tomorrow in {name}, expect {sky}, with a high of {_deg(hi)} "
                    f"and a low of {_deg(lo)} degrees.",
                    {"location": name, "high": hi, "low": lo, "code": daily["weather_code"][1]})
            cur = payload["current"]
            temp, feels = cur["temperature_2m"], cur["apparent_temperature"]
            sky = _SKY.get(cur["weather_code"], "unsettled weather")
            message = f"In {name} it's {_deg(temp)} degrees with {sky}."
            if abs(round(feels) - round(temp)) >= 2:
                message += f" It feels like {_deg(feels)}."
            return ToolResult(True, message, {
                "location": name, "temperature": temp, "feels_like": feels,
                "humidity": cur["relative_humidity_2m"], "wind_speed": cur["wind_speed_10m"],
                "code": cur["weather_code"]})
        except (httpx.HTTPError, ValueError, KeyError, IndexError, TypeError) as e:
            log.warning("weather lookup failed: %s", e)
            return ToolResult(False, "I couldn't reach the weather service right now.")
