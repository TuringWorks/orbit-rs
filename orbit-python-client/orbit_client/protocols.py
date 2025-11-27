"""
Protocol definitions and base classes for Orbit-RS Python client.
"""

from abc import ABC, abstractmethod
from enum import Enum
from typing import Any, Dict, List, Optional, Union
from dataclasses import dataclass


class Protocol(Enum):
    """Supported protocol types."""
    POSTGRES = "postgres"
    MYSQL = "mysql"
    CQL = "cql"
    REDIS = "redis"
    CYPHER = "cypher"
    AQL = "aql"
    MCP = "mcp"


@dataclass
class ConnectionConfig:
    """Base connection configuration."""
    host: str = "127.0.0.1"
    port: int = 0
    username: Optional[str] = None
    password: Optional[str] = None
    database: Optional[str] = None
    ssl: bool = False
    timeout: int = 30


@dataclass
class PostgresConfig(ConnectionConfig):
    """PostgreSQL connection configuration."""
    port: int = 5432
    database: str = "postgres"


@dataclass
class MySQLConfig(ConnectionConfig):
    """MySQL connection configuration."""
    port: int = 3306
    database: str = "orbit"


@dataclass
class CQLConfig(ConnectionConfig):
    """CQL/Cassandra connection configuration."""
    port: int = 9042
    keyspace: Optional[str] = None


@dataclass
class RedisConfig(ConnectionConfig):
    """Redis/RESP connection configuration."""
    port: int = 6379
    db: int = 0


@dataclass
class CypherConfig(ConnectionConfig):
    """Cypher/Bolt connection configuration."""
    port: int = 7687
    database: str = "neo4j"


@dataclass
class AQLConfig(ConnectionConfig):
    """AQL/ArangoDB connection configuration."""
    port: int = 8529
    database: str = "_system"


@dataclass
class MCPConfig(ConnectionConfig):
    """MCP connection configuration."""
    port: int = 8080
    base_url: str = "http://127.0.0.1:8080"


class ProtocolClient(ABC):
    """Base class for protocol-specific clients."""
    
    def __init__(self, config: ConnectionConfig):
        self.config = config
        self._connected = False
    
    @abstractmethod
    def connect(self) -> None:
        """Establish connection to the server."""
        pass
    
    @abstractmethod
    def disconnect(self) -> None:
        """Close connection to the server."""
        pass
    
    @abstractmethod
    def is_connected(self) -> bool:
        """Check if client is connected."""
        pass
    
    @property
    def connected(self) -> bool:
        """Check connection status."""
        return self.is_connected()


class PostgresProtocol(ProtocolClient):
    """PostgreSQL protocol client."""
    
    def __init__(self, config: PostgresConfig):
        super().__init__(config)
        self._connection = None
        try:
            import psycopg2
            self.psycopg2 = psycopg2
        except ImportError:
            raise ImportError("psycopg2-binary is required for PostgreSQL support. Install with: pip install psycopg2-binary")
    
    def connect(self) -> None:
        """Connect to PostgreSQL server."""
        if self._connected:
            return
        
        config = self.config
        self._connection = self.psycopg2.connect(
            host=config.host,
            port=config.port,
            user=config.username or "orbit",
            password=config.password or "",
            database=config.database,
            connect_timeout=config.timeout,
        )
        self._connected = True
    
    def disconnect(self) -> None:
        """Disconnect from PostgreSQL server."""
        if self._connection:
            self._connection.close()
            self._connection = None
        self._connected = False
    
    def is_connected(self) -> bool:
        """Check if connected."""
        return self._connected and self._connection and not self._connection.closed
    
    def execute(self, query: str, params: Optional[tuple] = None) -> List[Dict[str, Any]]:
        """Execute SQL query and return results."""
        if not self.is_connected():
            self.connect()
        
        with self._connection.cursor() as cursor:
            cursor.execute(query, params)
            if cursor.description:
                columns = [desc[0] for desc in cursor.description]
                rows = cursor.fetchall()
                return [dict(zip(columns, row)) for row in rows]
            self._connection.commit()
            return []
    
    def execute_many(self, query: str, params_list: List[tuple]) -> None:
        """Execute query with multiple parameter sets."""
        if not self.is_connected():
            self.connect()
        
        with self._connection.cursor() as cursor:
            cursor.executemany(query, params_list)
            self._connection.commit()


class MySQLProtocol(ProtocolClient):
    """MySQL protocol client."""
    
    def __init__(self, config: MySQLConfig):
        super().__init__(config)
        self._connection = None
        try:
            import pymysql
            self.pymysql = pymysql
        except ImportError:
            raise ImportError("PyMySQL is required for MySQL support. Install with: pip install PyMySQL")
    
    def connect(self) -> None:
        """Connect to MySQL server."""
        if self._connected:
            return
        
        config = self.config
        self._connection = self.pymysql.connect(
            host=config.host,
            port=config.port,
            user=config.username or "orbit",
            password=config.password or "",
            database=config.database,
            connect_timeout=config.timeout,
        )
        self._connected = True
    
    def disconnect(self) -> None:
        """Disconnect from MySQL server."""
        if self._connection:
            self._connection.close()
            self._connection = None
        self._connected = False
    
    def is_connected(self) -> bool:
        """Check if connected."""
        return self._connected and self._connection and self._connection.open
    
    def execute(self, query: str, params: Optional[tuple] = None) -> List[Dict[str, Any]]:
        """Execute SQL query and return results."""
        if not self.is_connected():
            self.connect()
        
        with self._connection.cursor(self.pymysql.cursors.DictCursor) as cursor:
            cursor.execute(query, params)
            if cursor.description:
                return cursor.fetchall()
            self._connection.commit()
            return []
    
    def execute_many(self, query: str, params_list: List[tuple]) -> None:
        """Execute query with multiple parameter sets."""
        if not self.is_connected():
            self.connect()
        
        with self._connection.cursor() as cursor:
            cursor.executemany(query, params_list)
            self._connection.commit()


class CQLProtocol(ProtocolClient):
    """CQL/Cassandra protocol client."""
    
    def __init__(self, config: CQLConfig):
        super().__init__(config)
        self._cluster = None
        self._session = None
        try:
            from cassandra.cluster import Cluster
            from cassandra.auth import PlainTextAuthProvider
            self.Cluster = Cluster
            self.PlainTextAuthProvider = PlainTextAuthProvider
        except ImportError:
            raise ImportError("cassandra-driver is required for CQL support. Install with: pip install cassandra-driver")
    
    def connect(self) -> None:
        """Connect to CQL server."""
        if self._connected:
            return
        
        config = self.config
        auth_provider = None
        if config.username and config.password:
            auth_provider = self.PlainTextAuthProvider(username=config.username, password=config.password)
        
        self._cluster = self.Cluster(
            [config.host],
            port=config.port,
            auth_provider=auth_provider,
            connect_timeout=config.timeout,
        )
        self._session = self._cluster.connect()
        if config.keyspace:
            self._session.set_keyspace(config.keyspace)
        self._connected = True
    
    def disconnect(self) -> None:
        """Disconnect from CQL server."""
        if self._session:
            self._session.shutdown()
            self._session = None
        if self._cluster:
            self._cluster.shutdown()
            self._cluster = None
        self._connected = False
    
    def is_connected(self) -> bool:
        """Check if connected."""
        return self._connected and self._session is not None
    
    def execute(self, query: str, params: Optional[tuple] = None) -> List[Dict[str, Any]]:
        """Execute CQL query and return results."""
        if not self.is_connected():
            self.connect()
        
        result = self._session.execute(query, params)
        rows = []
        for row in result:
            rows.append(dict(row._asdict()))
        return rows
    
    def execute_many(self, query: str, params_list: List[tuple]) -> None:
        """Execute query with multiple parameter sets."""
        if not self.is_connected():
            self.connect()
        
        prepared = self._session.prepare(query)
        for params in params_list:
            self._session.execute(prepared, params)


class RedisProtocol(ProtocolClient):
    """Redis/RESP protocol client."""
    
    def __init__(self, config: RedisConfig):
        super().__init__(config)
        self._client = None
        try:
            import redis
            self.redis = redis
        except ImportError:
            raise ImportError("redis is required for Redis support. Install with: pip install redis")
    
    def connect(self) -> None:
        """Connect to Redis server."""
        if self._connected:
            return
        
        config = self.config
        self._client = self.redis.Redis(
            host=config.host,
            port=config.port,
            password=config.password,
            db=config.db,
            socket_connect_timeout=config.timeout,
            decode_responses=True,
        )
        # Test connection
        self._client.ping()
        self._connected = True
    
    def disconnect(self) -> None:
        """Disconnect from Redis server."""
        if self._client:
            self._client.close()
            self._client = None
        self._connected = False
    
    def is_connected(self) -> bool:
        """Check if connected."""
        if not self._connected or not self._client:
            return False
        try:
            self._client.ping()
            return True
        except:
            return False
    
    def execute(self, command: str, *args) -> Any:
        """Execute Redis command."""
        if not self.is_connected():
            self.connect()
        
        return self._client.execute_command(command, *args)
    
    def get(self, key: str) -> Optional[str]:
        """Get value by key."""
        return self.execute("GET", key)
    
    def set(self, key: str, value: str, ex: Optional[int] = None) -> bool:
        """Set key-value pair."""
        return self.execute("SET", key, value, ex=ex) if ex else self.execute("SET", key, value)
    
    def delete(self, *keys: str) -> int:
        """Delete keys."""
        return self.execute("DEL", *keys)
    
    def keys(self, pattern: str = "*") -> List[str]:
        """Get keys matching pattern."""
        return self.execute("KEYS", pattern)


class CypherProtocol(ProtocolClient):
    """Cypher/Bolt protocol client."""
    
    def __init__(self, config: CypherConfig):
        super().__init__(config)
        self._driver = None
        self._session = None
        try:
            from neo4j import GraphDatabase
            self.GraphDatabase = GraphDatabase
        except ImportError:
            raise ImportError("neo4j is required for Cypher support. Install with: pip install neo4j")
    
    def connect(self) -> None:
        """Connect to Bolt server."""
        if self._connected:
            return
        
        config = self.config
        uri = f"bolt://{config.host}:{config.port}"
        auth = None
        if config.username and config.password:
            auth = (config.username, config.password)
        
        self._driver = self.GraphDatabase.driver(uri, auth=auth)
        self._session = self._driver.session(database=config.database)
        self._connected = True
    
    def disconnect(self) -> None:
        """Disconnect from Bolt server."""
        if self._session:
            self._session.close()
            self._session = None
        if self._driver:
            self._driver.close()
            self._driver = None
        self._connected = False
    
    def is_connected(self) -> bool:
        """Check if connected."""
        return self._connected and self._driver is not None
    
    def execute(self, query: str, params: Optional[Dict[str, Any]] = None) -> List[Dict[str, Any]]:
        """Execute Cypher query and return results."""
        if not self.is_connected():
            self.connect()
        
        result = self._session.run(query, params or {})
        records = []
        for record in result:
            records.append(dict(record))
        return records


class AQLProtocol(ProtocolClient):
    """AQL/ArangoDB protocol client."""
    
    def __init__(self, config: AQLConfig):
        super().__init__(config)
        self._client = None
        try:
            from arango import ArangoClient
            self.ArangoClient = ArangoClient
        except ImportError:
            raise ImportError("python-arango is required for AQL support. Install with: pip install python-arango")
    
    def connect(self) -> None:
        """Connect to ArangoDB server."""
        if self._connected:
            return
        
        config = self.config
        self._client = self.ArangoClient(
            hosts=f"http://{config.host}:{config.port}",
        )
        
        if config.username and config.password:
            self._db = self._client.db(config.database, username=config.username, password=config.password)
        else:
            self._db = self._client.db(config.database)
        
        self._connected = True
    
    def disconnect(self) -> None:
        """Disconnect from ArangoDB server."""
        # ArangoDB client doesn't have explicit disconnect
        self._client = None
        self._db = None
        self._connected = False
    
    def is_connected(self) -> bool:
        """Check if connected."""
        return self._connected and self._db is not None
    
    def execute(self, query: str, bind_vars: Optional[Dict[str, Any]] = None) -> List[Dict[str, Any]]:
        """Execute AQL query and return results."""
        if not self.is_connected():
            self.connect()
        
        cursor = self._db.aql.execute(query, bind_vars=bind_vars or {})
        return list(cursor)


class MCPProtocol(ProtocolClient):
    """MCP (Model Context Protocol) client."""
    
    def __init__(self, config: MCPConfig):
        super().__init__(config)
        self._client = None
        try:
            import httpx
            self.httpx = httpx
        except ImportError:
            raise ImportError("httpx is required for MCP support. Install with: pip install httpx")
    
    def connect(self) -> None:
        """Connect to MCP server."""
        if self._connected:
            return
        
        config = self.config
        self._client = self.httpx.Client(
            base_url=config.base_url or f"http://{config.host}:{config.port}",
            timeout=config.timeout,
        )
        self._connected = True
    
    def disconnect(self) -> None:
        """Disconnect from MCP server."""
        if self._client:
            self._client.close()
            self._client = None
        self._connected = False
    
    def is_connected(self) -> bool:
        """Check if connected."""
        return self._connected and self._client is not None
    
    def execute(self, method: str, params: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        """Execute MCP method and return result."""
        if not self.is_connected():
            self.connect()
        
        response = self._client.post(
            "/mcp",
            json={
                "jsonrpc": "2.0",
                "id": 1,
                "method": method,
                "params": params or {},
            },
        )
        response.raise_for_status()
        result = response.json()
        if "error" in result:
            raise Exception(f"MCP error: {result['error']}")
        return result.get("result", {})

