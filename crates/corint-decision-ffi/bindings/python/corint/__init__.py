"""
CORINT Decision Engine - Python Bindings

A high-performance decision engine for fraud detection and risk management.
"""

from .engine import DecisionEngine, DecisionRequest, DecisionResponse

__version__ = "0.1.0"

__all__ = ["DecisionEngine", "DecisionRequest", "DecisionResponse"]
