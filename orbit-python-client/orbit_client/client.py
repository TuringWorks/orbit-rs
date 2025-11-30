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

    # Time Series convenience methods (Redis TimeSeries compatible)

    def ts_create(
        self,
        key: str,
        retention_ms: Optional[int] = None,
        labels: Optional[Dict[str, str]] = None,
        duplicate_policy: Optional[str] = None,
    ) -> bool:
        """
        Create a time series key (Redis only).

        Args:
            key: Time series key name
            retention_ms: Data retention in milliseconds (0 = no expiration)
            labels: Key-value labels for filtering
            duplicate_policy: Policy for duplicate timestamps (BLOCK, FIRST, LAST, MIN, MAX, SUM)

        Example:
            >>> client.ts_create("sensor:temp", retention_ms=86400000, labels={"location": "room1"})
        """
        if self._protocol != Protocol.REDIS:
            raise ValueError("ts_create() is only available for Redis protocol")

        args = [key]
        if retention_ms is not None:
            args.extend(["RETENTION", str(retention_ms)])
        if duplicate_policy:
            args.extend(["DUPLICATE_POLICY", duplicate_policy])
        if labels:
            args.append("LABELS")
            for k, v in labels.items():
                args.extend([k, v])

        return self._protocol_client.execute("TS.CREATE", *args)

    def ts_add(
        self,
        key: str,
        timestamp: Union[int, str],
        value: float,
        retention_ms: Optional[int] = None,
        labels: Optional[Dict[str, str]] = None,
    ) -> int:
        """
        Add a sample to a time series (Redis only).

        Args:
            key: Time series key name
            timestamp: Unix timestamp in ms, or "*" for auto-timestamp
            value: Sample value
            retention_ms: Optional retention override
            labels: Optional labels (creates key if not exists)

        Returns:
            Timestamp of the added sample

        Example:
            >>> client.ts_add("sensor:temp", "*", 23.5)
            >>> client.ts_add("sensor:temp", 1609459200000, 22.1)
        """
        if self._protocol != Protocol.REDIS:
            raise ValueError("ts_add() is only available for Redis protocol")

        args = [key, str(timestamp), str(value)]
        if retention_ms is not None:
            args.extend(["RETENTION", str(retention_ms)])
        if labels:
            args.append("LABELS")
            for k, v in labels.items():
                args.extend([k, v])

        return self._protocol_client.execute("TS.ADD", *args)

    def ts_get(self, key: str) -> Optional[tuple]:
        """
        Get the last sample from a time series (Redis only).

        Args:
            key: Time series key name

        Returns:
            Tuple of (timestamp, value) or None if empty

        Example:
            >>> ts, value = client.ts_get("sensor:temp")
        """
        if self._protocol != Protocol.REDIS:
            raise ValueError("ts_get() is only available for Redis protocol")
        return self._protocol_client.execute("TS.GET", key)

    def ts_range(
        self,
        key: str,
        from_ts: Union[int, str],
        to_ts: Union[int, str],
        count: Optional[int] = None,
        aggregation: Optional[str] = None,
        bucket_duration_ms: Optional[int] = None,
    ) -> List[tuple]:
        """
        Query a range of samples from a time series (Redis only).

        Args:
            key: Time series key name
            from_ts: Start timestamp (or "-" for minimum)
            to_ts: End timestamp (or "+" for maximum)
            count: Maximum number of samples to return
            aggregation: Aggregation type (AVG, SUM, MIN, MAX, RANGE, COUNT, FIRST, LAST, STD.P, VAR.P)
            bucket_duration_ms: Aggregation bucket size in milliseconds

        Returns:
            List of (timestamp, value) tuples

        Example:
            >>> samples = client.ts_range("sensor:temp", "-", "+")
            >>> samples = client.ts_range("sensor:temp", 0, 1000000, aggregation="AVG", bucket_duration_ms=60000)
        """
        if self._protocol != Protocol.REDIS:
            raise ValueError("ts_range() is only available for Redis protocol")

        args = [key, str(from_ts), str(to_ts)]
        if count is not None:
            args.extend(["COUNT", str(count)])
        if aggregation and bucket_duration_ms:
            args.extend(["AGGREGATION", aggregation, str(bucket_duration_ms)])

        return self._protocol_client.execute("TS.RANGE", *args)

    def ts_mrange(
        self,
        from_ts: Union[int, str],
        to_ts: Union[int, str],
        filters: List[str],
        count: Optional[int] = None,
        aggregation: Optional[str] = None,
        bucket_duration_ms: Optional[int] = None,
    ) -> List[Dict]:
        """
        Query a range from multiple time series by filter (Redis only).

        Args:
            from_ts: Start timestamp (or "-" for minimum)
            to_ts: End timestamp (or "+" for maximum)
            filters: List of label filters (e.g., ["location=room1", "sensor=temp"])
            count: Maximum number of samples per series
            aggregation: Aggregation type
            bucket_duration_ms: Aggregation bucket size

        Returns:
            List of series with their samples

        Example:
            >>> results = client.ts_mrange("-", "+", ["location=room1"])
        """
        if self._protocol != Protocol.REDIS:
            raise ValueError("ts_mrange() is only available for Redis protocol")

        args = [str(from_ts), str(to_ts)]
        if count is not None:
            args.extend(["COUNT", str(count)])
        if aggregation and bucket_duration_ms:
            args.extend(["AGGREGATION", aggregation, str(bucket_duration_ms)])
        args.append("FILTER")
        args.extend(filters)

        return self._protocol_client.execute("TS.MRANGE", *args)

    def ts_info(self, key: str) -> Dict[str, Any]:
        """
        Get metadata about a time series (Redis only).

        Args:
            key: Time series key name

        Returns:
            Dictionary with time series metadata

        Example:
            >>> info = client.ts_info("sensor:temp")
            >>> print(info["totalSamples"])
        """
        if self._protocol != Protocol.REDIS:
            raise ValueError("ts_info() is only available for Redis protocol")
        return self._protocol_client.execute("TS.INFO", key)

    def ts_del(self, key: str, from_ts: Union[int, str], to_ts: Union[int, str]) -> int:
        """
        Delete samples from a time series in a range (Redis only).

        Args:
            key: Time series key name
            from_ts: Start timestamp
            to_ts: End timestamp

        Returns:
            Number of samples deleted

        Example:
            >>> deleted = client.ts_del("sensor:temp", 0, 1000000)
        """
        if self._protocol != Protocol.REDIS:
            raise ValueError("ts_del() is only available for Redis protocol")
        return self._protocol_client.execute("TS.DEL", key, str(from_ts), str(to_ts))

    def ts_createrule(
        self,
        source_key: str,
        dest_key: str,
        aggregation: str,
        bucket_duration_ms: int,
    ) -> bool:
        """
        Create a compaction rule (Redis only).

        Args:
            source_key: Source time series key
            dest_key: Destination time series key
            aggregation: Aggregation type (AVG, SUM, MIN, MAX, etc.)
            bucket_duration_ms: Aggregation bucket size

        Example:
            >>> client.ts_createrule("sensor:temp", "sensor:temp:hourly", "AVG", 3600000)
        """
        if self._protocol != Protocol.REDIS:
            raise ValueError("ts_createrule() is only available for Redis protocol")
        return self._protocol_client.execute(
            "TS.CREATERULE", source_key, dest_key, "AGGREGATION", aggregation, str(bucket_duration_ms)
        )

    def ts_deleterule(self, source_key: str, dest_key: str) -> bool:
        """
        Delete a compaction rule (Redis only).

        Args:
            source_key: Source time series key
            dest_key: Destination time series key

        Example:
            >>> client.ts_deleterule("sensor:temp", "sensor:temp:hourly")
        """
        if self._protocol != Protocol.REDIS:
            raise ValueError("ts_deleterule() is only available for Redis protocol")
        return self._protocol_client.execute("TS.DELETERULE", source_key, dest_key)

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

