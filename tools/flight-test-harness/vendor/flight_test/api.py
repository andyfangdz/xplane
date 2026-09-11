"""Small dependency-free client for the X-Plane 12 Web API."""

from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
import http.client
import json
import threading
from typing import Any, Iterable


class XPlaneApiError(RuntimeError):
    pass


class XPlaneConnectionError(XPlaneApiError):
    """The transport failed; a non-idempotent request may still have been accepted."""


class XPlaneApi:
    """Catalog-aware API client with per-worker persistent HTTP connections."""

    def __init__(self, port: int = 8142, timeout: float = 20.0, workers: int = 8) -> None:
        self.port = port
        self.timeout = timeout
        self.base_path = "/api/v3"
        self.datarefs: dict[str, dict[str, Any]] = {}
        self.commands: dict[str, dict[str, Any]] = {}
        self._local = threading.local()
        self._connections: list[http.client.HTTPConnection] = []
        self._connections_lock = threading.Lock()
        self._executor = ThreadPoolExecutor(max_workers=workers, thread_name_prefix="xplane-api")

    def __enter__(self) -> "XPlaneApi":
        return self

    def __exit__(self, *_: object) -> None:
        self.close()

    def _connection(self) -> http.client.HTTPConnection:
        connection = getattr(self._local, "connection", None)
        if connection is None:
            connection = http.client.HTTPConnection("127.0.0.1", self.port, timeout=self.timeout)
            self._local.connection = connection
            with self._connections_lock:
                self._connections.append(connection)
        return connection

    def _drop_thread_connection(self) -> None:
        connection = getattr(self._local, "connection", None)
        if connection is not None:
            connection.close()
            self._local.connection = None

    def request(self, method: str, path: str, body: Any = None) -> Any:
        encoded = None if body is None else json.dumps(body, separators=(",", ":")).encode("utf-8")
        headers = {} if encoded is None else {"Content-Type": "application/json"}
        last_error: Exception | None = None
        for attempt in range(2):
            connection = self._connection()
            try:
                connection.request(method, f"{self.base_path}{path}", body=encoded, headers=headers)
                response = connection.getresponse()
                payload = response.read()
                if response.status < 200 or response.status >= 300:
                    message = payload.decode("utf-8", errors="replace")
                    raise XPlaneApiError(f"HTTP {response.status} {method} {path}: {message}")
                if not payload.strip():
                    return None
                return json.loads(payload)
            except (ConnectionError, OSError, http.client.HTTPException) as error:
                last_error = error
                self._drop_thread_connection()
                if attempt == 0 and method == "GET":
                    continue
                raise XPlaneConnectionError(f"{method} {path} failed: {error}") from error
        raise XPlaneConnectionError(str(last_error))

    def refresh_catalogs(self) -> None:
        datarefs = self.request("GET", "/datarefs?limit=30000")["data"]
        commands = self.request("GET", "/commands?limit=20000")["data"]
        self.datarefs = {str(item["name"]): item for item in datarefs}
        self.commands = {str(item["name"]): item for item in commands}

    def require_datarefs(self, names: Iterable[str]) -> None:
        missing = [name for name in names if name not in self.datarefs]
        if missing:
            raise XPlaneApiError(f"Missing datarefs: {', '.join(missing)}")

    def get_raw(self, name: str) -> Any:
        try:
            identifier = self.datarefs[name]["id"]
        except KeyError as error:
            raise XPlaneApiError(f"Missing dataref: {name}") from error
        return self.request("GET", f"/datarefs/{identifier}/value")["data"]

    def get_scalar(self, name: str) -> Any:
        value = self.get_raw(name)
        return value[0] if isinstance(value, list) else value

    def get_batch(self, names: Iterable[str]) -> dict[str, Any]:
        ordered = list(names)
        futures = {name: self._executor.submit(self.get_raw, name) for name in ordered}
        return {name: futures[name].result() for name in ordered}

    def set_dataref(self, name: str, value: Any) -> None:
        try:
            identifier = self.datarefs[name]["id"]
        except KeyError as error:
            raise XPlaneApiError(f"Missing dataref: {name}") from error
        self.request("PATCH", f"/datarefs/{identifier}/value", {"data": value})

    def command(self, name: str, duration: float = 0.15) -> None:
        try:
            identifier = self.commands[name]["id"]
        except KeyError as error:
            raise XPlaneApiError(f"Missing command: {name}") from error
        self.request("POST", f"/command/{identifier}/activate", {"duration": duration})

    def load_flight(self, payload: dict[str, Any]) -> None:
        self.request("POST", "/flight", payload)

    def close(self) -> None:
        self._executor.shutdown(wait=True, cancel_futures=True)
        with self._connections_lock:
            for connection in self._connections:
                connection.close()
            self._connections.clear()
