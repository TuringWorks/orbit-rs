"""
Unified Orbit-RS client that supports all protocols.
"""

from typing import Any, Dict, List, Optional, Union
from orbit_client.protocols import (
    Protocol,
    PostgresProtocol,
    MySQLProtocol,
    CQLProtocol,
    RedisProtocol,
    CypherProtocol,
    AQLProtocol,
    MCPProtocol,
    PostgresConfig,
    MySQLConfig,
    CQLConfig,
    RedisConfig,
    CypherConfig,
    AQLConfig,
    MCPConfig,
    ConnectionConfig,
)


class OrbitClient:
    """
    Unified client for accessing Orbit-RS through any supported protocol.
    
    Example:
        >>> from orbit_client import OrbitClient, Protocol
        >>> 
        >>> # PostgreSQL
        >>> client = OrbitClient.postgres(host="127.0.0.1", port=5432)
        >>> results = client.execute("SELECT * FROM users")
        >>> 
        >>> # Redis
        >>> client = OrbitClient.redis(host="127.0.0.1", port=6379)
        >>> client.set("key", "value")
        >>> value = client.get("key")
        >>> 
        >>> # Cypher
        >>> client = OrbitClient.cypher(host="127.0.0.1", port=7687)
        >>> results = client.execute("MATCH (n) RETURN n LIMIT 10")
    """
    
    def __init__(self, protocol_client: Union[
        PostgresProtocol,
        MySQLProtocol,
        CQLProtocol,
        RedisProtocol,
        CypherProtocol,
        AQLProtocol,
        MCPProtocol,
    ]):
        """Initialize client with a protocol-specific client."""
        self._protocol_client = protocol_client
        self._protocol = self._detect_protocol()
    
    @classmethod
    def postgres(
        cls,
        host: str = "127.0.0.1",
        port: int = 5432,
        username: Optional[str] = None,
        password: Optional[str] = None,
        database: str = "postgres",
        ssl: bool = False,
        timeout: int = 30,
    ) -> "OrbitClient":
        """Create PostgreSQL client."""
        config = PostgresConfig(
            host=host,
            port=port,
            username=username,
            password=password,
            database=database,
            ssl=ssl,
            timeout=timeout,
        )
        return cls(PostgresProtocol(config))
    
    @classmethod
    def mysql(
        cls,
        host: str = "127.0.0.1",
        port: int = 3306,
        username: Optional[str] = None,
        password: Optional[str] = None,
        database: str = "orbit",
        ssl: bool = False,
        timeout: int = 30,
    ) -> "OrbitClient":
        """Create MySQL client."""
        config = MySQLConfig(
            host=host,
            port=port,
            username=username,
            password=password,
            database=database,
            ssl=ssl,
            timeout=timeout,
        )
        return cls(MySQLProtocol(config))
    
    @classmethod
    def cql(
        cls,
        host: str = "127.0.0.1",
        port: int = 9042,
        username: Optional[str] = None,
        password: Optional[str] = None,
        keyspace: Optional[str] = None,
        timeout: int = 30,
    ) -> "OrbitClient":
        """Create CQL/Cassandra client."""
        config = CQLConfig(
            host=host,
            port=port,
            username=username,
            password=password,
            keyspace=keyspace,
            timeout=timeout,
        )
        return cls(CQLProtocol(config))
    
    @classmethod
    def redis(
        cls,
        host: str = "127.0.0.1",
        port: int = 6379,
        password: Optional[str] = None,
        db: int = 0,
        timeout: int = 30,
    ) -> "OrbitClient":
        """Create Redis/RESP client."""
        config = RedisConfig(
            host=host,
            port=port,
            password=password,
            db=db,
            timeout=timeout,
        )
        return cls(RedisProtocol(config))
    
    @classmethod
    def cypher(
        cls,
        host: str = "127.0.0.1",
        port: int = 7687,
        username: Optional[str] = None,
        password: Optional[str] = None,
        database: str = "neo4j",
        timeout: int = 30,
    ) -> "OrbitClient":
        """Create Cypher/Bolt client."""
        config = CypherConfig(
            host=host,
            port=port,
            username=username,
            password=password,
            database=database,
            timeout=timeout,
        )
        return cls(CypherProtocol(config))
    
    @classmethod
    def aql(
        cls,
        host: str = "127.0.0.1",
        port: int = 8529,
        username: Optional[str] = None,
        password: Optional[str] = None,
        database: str = "_system",
        timeout: int = 30,
    ) -> "OrbitClient":
        """Create AQL/ArangoDB client."""
        config = AQLConfig(
            host=host,
            port=port,
            username=username,
            password=password,
            database=database,
            timeout=timeout,
        )
        return cls(AQLProtocol(config))
    
    @classmethod
    def mcp(
        cls,
        host: str = "127.0.0.1",
        port: int = 8080,
        base_url: Optional[str] = None,
        timeout: int = 30,
    ) -> "OrbitClient":
        """Create MCP client."""
        config = MCPConfig(
            host=host,
            port=port,
            base_url=base_url or f"http://{host}:{port}",
            timeout=timeout,
        )
        return cls(MCPProtocol(config))
    
    def _detect_protocol(self) -> Protocol:
        """Detect protocol type from client instance."""
        if isinstance(self._protocol_client, PostgresProtocol):
            return Protocol.POSTGRES
        elif isinstance(self._protocol_client, MySQLProtocol):
            return Protocol.MYSQL
        elif isinstance(self._protocol_client, CQLProtocol):
            return Protocol.CQL
        elif isinstance(self._protocol_client, RedisProtocol):
            return Protocol.REDIS
        elif isinstance(self._protocol_client, CypherProtocol):
            return Protocol.CYPHER
        elif isinstance(self._protocol_client, AQLProtocol):
            return Protocol.AQL
        elif isinstance(self._protocol_client, MCPProtocol):
            return Protocol.MCP
        else:
            raise ValueError("Unknown protocol client type")
    
    def connect(self) -> None:
        """Connect to the server."""
        self._protocol_client.connect()
    
    def disconnect(self) -> None:
        """Disconnect from the server."""
        self._protocol_client.disconnect()
    
    def is_connected(self) -> bool:
        """Check if connected."""
        return self._protocol_client.is_connected()
    
    def execute(self, query: str, **kwargs) -> Union[List[Dict[str, Any]], Any]:
        """
        Execute a query/command using the appropriate protocol.
        
        Args:
            query: Query string (SQL, CQL, Cypher, AQL) or command (Redis, MCP)
            **kwargs: Protocol-specific parameters
        
        Returns:
            Query results or command response
        """
        if not self.is_connected():
            self.connect()
        
        # Protocol-specific execution
        if self._protocol == Protocol.POSTGRES:
            params = kwargs.get("params")
            return self._protocol_client.execute(query, params)
        
        elif self._protocol == Protocol.MYSQL:
            params = kwargs.get("params")
            return self._protocol_client.execute(query, params)
        
        elif self._protocol == Protocol.CQL:
            params = kwargs.get("params")
            return self._protocol_client.execute(query, params)
        
        elif self._protocol == Protocol.REDIS:
            # For Redis, query is the command, kwargs are arguments
            args = kwargs.get("args", ())
            return self._protocol_client.execute(query, *args)
        
        elif self._protocol == Protocol.CYPHER:
            params = kwargs.get("params", {})
            return self._protocol_client.execute(query, params)
        
        elif self._protocol == Protocol.AQL:
            bind_vars = kwargs.get("bind_vars", {})
            return self._protocol_client.execute(query, bind_vars)
        
        elif self._protocol == Protocol.MCP:
            params = kwargs.get("params", {})
            return self._protocol_client.execute(query, params)
        
        else:
            raise ValueError(f"Unsupported protocol: {self._protocol}")
    
    # Protocol-specific convenience methods
    
    # Redis convenience methods
    def get(self, key: str) -> Optional[str]:
        """Get value by key (Redis only)."""
        if self._protocol != Protocol.REDIS:
            raise ValueError("get() is only available for Redis protocol")
        return self._protocol_client.get(key)
    
    def set(self, key: str, value: str, ex: Optional[int] = None) -> bool:
        """Set key-value pair (Redis only)."""
        if self._protocol != Protocol.REDIS:
            raise ValueError("set() is only available for Redis protocol")
        return self._protocol_client.set(key, value, ex)
    
    def delete(self, *keys: str) -> int:
        """Delete keys (Redis only)."""
        if self._protocol != Protocol.REDIS:
            raise ValueError("delete() is only available for Redis protocol")
        return self._protocol_client.delete(*keys)
    
    def keys(self, pattern: str = "*") -> List[str]:
        """Get keys matching pattern (Redis only)."""
        if self._protocol != Protocol.REDIS:
            raise ValueError("keys() is only available for Redis protocol")
        return self._protocol_client.keys(pattern)
    
    @property
    def protocol(self) -> Protocol:
        """Get current protocol."""
        return self._protocol
    
    def __enter__(self):
        """Context manager entry."""
        self.connect()
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        """Context manager exit."""
        self.disconnect()

