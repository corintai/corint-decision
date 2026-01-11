"""
CORINT Decision Engine Python wrapper using ctypes
"""

import ctypes
import json
import os
import platform
from pathlib import Path
from typing import Dict, Any, Optional


def _find_library():
    """Find the CORINT FFI library"""
    # Determine library name based on platform
    system = platform.system()
    if system == "Darwin":
        lib_name = "libcorint_ffi.dylib"
    elif system == "Linux":
        lib_name = "libcorint_ffi.so"
    elif system == "Windows":
        lib_name = "corint_ffi.dll"
    else:
        raise RuntimeError(f"Unsupported platform: {system}")

    # Search in common locations
    search_paths = [
        # Development build
        Path(__file__).parent.parent.parent.parent.parent.parent / "target" / "debug" / lib_name,
        Path(__file__).parent.parent.parent.parent.parent.parent / "target" / "release" / lib_name,
        # System install
        Path("/usr/local/lib") / lib_name,
        Path("/usr/lib") / lib_name,
    ]

    for path in search_paths:
        if path.exists():
            return str(path)

    raise RuntimeError(f"Could not find {lib_name}. Please build the FFI library first.")


# Load the library
_lib_path = _find_library()
_lib = ctypes.CDLL(_lib_path)

# Define function signatures
_lib.corint_version.argtypes = []
_lib.corint_version.restype = ctypes.c_void_p  # Return as void pointer to manually manage

_lib.corint_engine_new.argtypes = [ctypes.c_char_p]
_lib.corint_engine_new.restype = ctypes.c_void_p

_lib.corint_engine_new_from_database.argtypes = [ctypes.c_char_p]
_lib.corint_engine_new_from_database.restype = ctypes.c_void_p

_lib.corint_engine_decide.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
_lib.corint_engine_decide.restype = ctypes.c_void_p  # Return as void pointer to manually manage

_lib.corint_engine_free.argtypes = [ctypes.c_void_p]
_lib.corint_engine_free.restype = None

_lib.corint_string_free.argtypes = [ctypes.c_void_p]  # Accept void pointer
_lib.corint_string_free.restype = None

_lib.corint_init_logging.argtypes = []
_lib.corint_init_logging.restype = None


class DecisionRequest:
    """Decision request with event data and optional features/API results"""

    def __init__(
        self,
        event_data: Dict[str, Any],
        features: Optional[Dict[str, Any]] = None,
        api: Optional[Dict[str, Any]] = None,
        service: Optional[Dict[str, Any]] = None,
        llm: Optional[Dict[str, Any]] = None,
        vars: Optional[Dict[str, Any]] = None,
        metadata: Optional[Dict[str, str]] = None,
        enable_trace: bool = False
    ):
        self.event_data = event_data
        self.features = features
        self.api = api
        self.service = service
        self.llm = llm
        self.vars = vars
        self.metadata = metadata or {}
        self.enable_trace = enable_trace

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization"""
        result = {
            "event_data": self.event_data,
            "metadata": self.metadata,
            "options": {"enable_trace": self.enable_trace}
        }

        if self.features is not None:
            result["features"] = self.features
        if self.api is not None:
            result["api"] = self.api
        if self.service is not None:
            result["service"] = self.service
        if self.llm is not None:
            result["llm"] = self.llm
        if self.vars is not None:
            result["vars"] = self.vars

        return result


class DecisionResponse:
    """Decision response from the engine"""

    def __init__(self, data: Dict[str, Any]):
        self._data = data

    def _result(self) -> Dict[str, Any]:
        result = self._data.get("result")
        return result if isinstance(result, dict) else {}

    @property
    def decision(self) -> str:
        """Get the final decision"""
        if "decision" in self._data:
            return self._data.get("decision", "")
        result = self._result()
        signal = result.get("signal")
        if isinstance(signal, dict):
            return signal.get("type", "") or ""
        if isinstance(signal, str):
            return signal
        return ""

    @property
    def actions(self) -> list:
        """Get the list of actions"""
        if "actions" in self._data:
            return self._data.get("actions", [])
        result = self._result()
        actions = result.get("actions", [])
        return actions if isinstance(actions, list) else []

    @property
    def trace(self) -> Optional[Dict[str, Any]]:
        """Get execution trace if enabled"""
        return self._data.get("trace")

    @property
    def metadata(self) -> Dict[str, Any]:
        """Get response metadata"""
        return self._data.get("metadata", {})

    @property
    def result(self) -> Dict[str, Any]:
        """Get raw result payload"""
        return self._result()

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary"""
        return self._data


class DecisionEngine:
    """CORINT Decision Engine for evaluating fraud detection rules"""

    def __init__(self, repository_path: Optional[str] = None, database_url: Optional[str] = None):
        """
        Initialize the decision engine

        Args:
            repository_path: Path to the file system repository (mutually exclusive with database_url)
            database_url: PostgreSQL database URL (mutually exclusive with repository_path)
        """
        if repository_path is None and database_url is None:
            raise ValueError("Either repository_path or database_url must be provided")

        if repository_path is not None and database_url is not None:
            raise ValueError("Cannot specify both repository_path and database_url")

        if repository_path is not None:
            path_bytes = repository_path.encode('utf-8')
            self._handle = _lib.corint_engine_new(path_bytes)
        else:
            url_bytes = database_url.encode('utf-8')
            self._handle = _lib.corint_engine_new_from_database(url_bytes)

        if not self._handle:
            raise RuntimeError("Failed to create decision engine")

    def decide(self, request: DecisionRequest) -> DecisionResponse:
        """
        Execute a decision

        Args:
            request: The decision request

        Returns:
            DecisionResponse with the result
        """
        if not self._handle:
            raise RuntimeError("Engine has been closed")

        # Convert request to JSON
        request_json = json.dumps(request.to_dict())
        request_bytes = request_json.encode('utf-8')

        # Call FFI function
        result_ptr = _lib.corint_engine_decide(self._handle, request_bytes)

        if not result_ptr:
            raise RuntimeError("Decision execution failed")

        # Copy the string before freeing
        result_json = ctypes.string_at(result_ptr).decode('utf-8')
        _lib.corint_string_free(result_ptr)

        result_data = json.loads(result_json)

        # Check for errors
        if "error" in result_data:
            raise RuntimeError(f"Decision error: {result_data['error']}")

        return DecisionResponse(result_data)

    def close(self):
        """Close the engine and free resources"""
        if self._handle:
            _lib.corint_engine_free(self._handle)
            self._handle = None

    def __enter__(self):
        """Context manager entry"""
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        """Context manager exit"""
        self.close()

    def __del__(self):
        """Destructor"""
        self.close()

    @staticmethod
    def init_logging():
        """Initialize the logging system"""
        _lib.corint_init_logging()

    @staticmethod
    def version() -> str:
        """Get the CORINT version"""
        version_ptr = _lib.corint_version()
        if not version_ptr:
            return "unknown"
        # Copy the string before freeing
        version_str = ctypes.string_at(version_ptr).decode('utf-8')
        _lib.corint_string_free(version_ptr)
        return version_str
