"""
Orbit-RS Python Client Library

A unified Python client for accessing Orbit-RS multi-model database through all supported protocols:
- PostgreSQL (SQL)
- MySQL (SQL)
- CQL/Cassandra
- Redis/RESP
- Cypher/Bolt (Neo4j)
- AQL/ArangoDB
- MCP (Model Context Protocol)
"""

from orbit_client.client import OrbitClient
from orbit_client.protocols import (
    Protocol,
    PostgresProtocol,
    MySQLProtocol,
    CQLProtocol,
    RedisProtocol,
    CypherProtocol,
    AQLProtocol,
    MCPProtocol,
)

__version__ = "0.1.0"
__all__ = [
    "OrbitClient",
    "Protocol",
    "PostgresProtocol",
    "MySQLProtocol",
    "CQLProtocol",
    "RedisProtocol",
    "CypherProtocol",
    "AQLProtocol",
    "MCPProtocol",
]

