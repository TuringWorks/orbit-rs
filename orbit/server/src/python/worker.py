#!/usr/bin/env python3
"""
Orbit-RS Python UDF Worker Process

This worker runs as a long-lived subprocess and executes Python UDFs via MessagePack-RPC.
Features:
- MessagePack for fast serialization
- Connection pooling (reusable process)
- Warm start (pre-loaded libraries)
- Batch execution support
- Security restrictions (resource limits)
- Sandboxed execution
"""

import sys
import os
import traceback
import signal
from typing import Any, Dict, List, Optional
from datetime import datetime, date, time, timedelta

# Try to import msgpack, fallback to JSON if not available
try:
    import msgpack
    USE_MSGPACK = True
except ImportError:
    import json
    USE_MSGPACK = False
    print("WARNING: msgpack not available, falling back to JSON (slower)", file=sys.stderr)

# Pre-load common libraries (warm start)
PRELOADED_LIBS = {}
try:
    import numpy as np
    PRELOADED_LIBS['numpy'] = np
    PRELOADED_LIBS['np'] = np
except ImportError:
    pass

try:
    import pandas as pd
    PRELOADED_LIBS['pandas'] = pd
    PRELOADED_LIBS['pd'] = pd
except ImportError:
    pass

try:
    import math
    PRELOADED_LIBS['math'] = math
except ImportError:
    pass

try:
    import re
    PRELOADED_LIBS['re'] = re
except ImportError:
    pass

try:
    import decimal
    PRELOADED_LIBS['decimal'] = decimal
    PRELOADED_LIBS['Decimal'] = decimal.Decimal
except ImportError:
    pass

# Security setup
def setup_security_limits():
    """Set up resource limits for security."""
    try:
        import resource
        
        # Memory limit: 512MB (more generous than Lua since Python libraries need more)
        resource.setrlimit(resource.RLIMIT_AS, (512 * 1024 * 1024, 512 * 1024 * 1024))
        
        # CPU time limit: 30 seconds per request
        resource.setrlimit(resource.RLIMIT_CPU, (30, 30))
        
        # File size limit: 10MB
        resource.setrlimit(resource.RLIMIT_FSIZE, (10 * 1024 * 1024, 10 * 1024 * 1024))
        
        # Number of open files
        resource.setrlimit(resource.RLIMIT_NOFILE, (100, 100))
        
    except ImportError:
        # resource module not available on Windows
        pass
    except Exception as e:
        print(f"WARNING: Could not set resource limits: {e}", file=sys.stderr)

def timeout_handler(signum, frame):
    """Handle timeout signal."""
    raise TimeoutError("Function execution timed out")

# Safe builtins - whitelist approach
SAFE_BUILTINS = {
    'abs', 'all', 'any', 'ascii', 'bin', 'bool', 'bytearray', 'bytes',
    'chr', 'complex', 'dict', 'divmod', 'enumerate', 'filter', 'float',
    'format', 'frozenset', 'hash', 'hex', 'int', 'isinstance', 'issubclass',
    'iter', 'len', 'list', 'map', 'max', 'min', 'next', 'oct', 'ord',
    'pow', 'range', 'repr', 'reversed', 'round', 'set', 'slice', 'sorted',
    'str', 'sum', 'tuple', 'type', 'zip',
    # Exceptions
    'Exception', 'ValueError', 'TypeError', 'KeyError', 'IndexError',
    'ZeroDivisionError', 'RuntimeError',
    # Useful functions
    'print', 'len', 'range',
}

def create_safe_namespace():
    """Create a restricted namespace for function execution."""
    import builtins
    safe_builtins = {name: getattr(builtins, name, None) for name in SAFE_BUILTINS}
    
    # Add pre-loaded libraries
    namespace = {'__builtins__': safe_builtins}
    namespace.update(PRELOADED_LIBS)
    
    return namespace

def python_to_msgpack_compatible(obj):
    """Convert Python objects to MessagePack-compatible types."""
    if obj is None:
        return None
    elif isinstance(obj, (bool, int, float, str, bytes)):
        return obj
    elif isinstance(obj, (list, tuple)):
        return [python_to_msgpack_compatible(item) for item in obj]
    elif isinstance(obj, dict):
        return {k: python_to_msgpack_compatible(v) for k, v in obj.items()}
    elif isinstance(obj, datetime):
        return {'__type__': 'datetime', 'value': obj.isoformat()}
    elif isinstance(obj, date):
        return {'__type__': 'date', 'value': obj.isoformat()}
    elif isinstance(obj, time):
        return {'__type__': 'time', 'value': obj.isoformat()}
    elif isinstance(obj, timedelta):
        return {'__type__': 'timedelta', 'value': obj.total_seconds()}
    elif 'numpy' in sys.modules:
        import numpy as np
        if isinstance(obj, np.ndarray):
            return obj.tolist()
        elif isinstance(obj, (np.integer, np.floating)):
            return obj.item()
    elif 'pandas' in sys.modules:
        import pandas as pd
        if isinstance(obj, (pd.Series, pd.DataFrame)):
            return obj.to_dict()
    
    # Fallback: convert to string
    return str(obj)

def execute_function(request: Dict[str, Any]) -> Dict[str, Any]:
    """Execute a single UDF."""
    try:
        func_source = request['function_source']
        func_name = request['function_name']
        args = request.get('args', [])
        timeout_seconds = request.get('timeout', 30)
        
        # Set up timeout
        if hasattr(signal, 'SIGALRM'):  # Unix only
            signal.signal(signal.SIGALRM, timeout_handler)
            signal.alarm(timeout_seconds)
        
        # Create safe namespace
        namespace = create_safe_namespace()
        
        # Execute function definition
        exec(func_source, namespace)
        
        # Get the function
        func = namespace.get(func_name)
        if not func:
            return {'error': f'Function {func_name} not found in namespace'}
        
        # Execute function with args
        result = func(*args)
        
        # Cancel timeout
        if hasattr(signal, 'SIGALRM'):
            signal.alarm(0)
        
        # Convert result to MessagePack-compatible format
        result = python_to_msgpack_compatible(result)
        
        return {'result': result, 'error': None}
        
    except TimeoutError as e:
        return {'result': None, 'error': {'type': 'TimeoutError', 'message': str(e)}}
    except Exception as e:
        return {
            'result': None,
            'error': {
                'type': type(e).__name__,
                'message': str(e),
                'traceback': traceback.format_exc()
            }
        }
    finally:
        # Always cancel alarm
        if hasattr(signal, 'SIGALRM'):
            signal.alarm(0)

def execute_batch(requests: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
    """Execute multiple UDFs in a batch."""
    results = []
    for request in requests:
        result = execute_function(request)
        results.append(result)
    return results

def handle_request(request: Dict[str, Any]) -> Dict[str, Any]:
    """Handle different types of requests."""
    method = request.get('method', 'execute')
    
    if method == 'execute':
        # Single execution
        response = execute_function(request.get('params', {}))
        response['id'] = request.get('id')
        return response
        
    elif method == 'batch':
        # Batch execution
        requests = request.get('params', {}).get('requests', [])
        results = execute_batch(requests)
        return {
            'id': request.get('id'),
            'result': results,
            'error': None
        }
        
    elif method == 'ping':
        # Health check
        return {
            'id': request.get('id'),
            'result': 'pong',
            'error': None
        }
        
    elif method == 'info':
        # Return worker info
        return {
            'id': request.get('id'),
            'result': {
                'preloaded_libraries': list(PRELOADED_LIBS.keys()),
                'use_msgpack': USE_MSGPACK,
                'python_version': sys.version,
            },
            'error': None
        }
        
    else:
        return {
            'id': request.get('id'),
            'result': None,
            'error': f'Unknown method: {method}'
        }

def main():
    """Main event loop - process requests via stdin/stdout."""
    # Set up security
    setup_security_limits()
    
    # Print startup message to stderr
    print(f"Python UDF Worker started (MessagePack: {USE_MSGPACK})", file=sys.stderr)
    print(f"Pre-loaded libraries: {', '.join(PRELOADED_LIBS.keys()) or 'none'}", file=sys.stderr)
    sys.stderr.flush()
    
    # Use binary mode for MessagePack
    if USE_MSGPACK:
        stdin_binary = sys.stdin.buffer
        stdout_binary = sys.stdout.buffer
        unpacker = msgpack.Unpacker(stdin_binary, raw=False)
        
        for request in unpacker:
            try:
                response = handle_request(request)
                packed = msgpack.packb(response, use_bin_type=True)
                
                # Write length prefix (4 bytes) + data
                length = len(packed)
                stdout_binary.write(length.to_bytes(4, byteorder='big'))
                stdout_binary.write(packed)
                stdout_binary.flush()
                
            except Exception as e:
                error_response = {
                    'id': request.get('id') if isinstance(request, dict) else None,
                    'result': None,
                    'error': {'type': 'InternalError', 'message': str(e)}
                }
                packed = msgpack.packb(error_response, use_bin_type=True)
                length = len(packed)
                stdout_binary.write(length.to_bytes(4, byteorder='big'))
                stdout_binary.write(packed)
                stdout_binary.flush()
    else:
        # JSON fallback
        while True:
            try:
                line = sys.stdin.readline()
                if not line:
                    break
                
                request = json.loads(line)
                response = handle_request(request)
                
                print(json.dumps(response), flush=True)
                
            except json.JSONDecodeError as e:
                error_response = {
                    'id': None,
                    'result': None,
                    'error': {'type': 'JSONDecodeError', 'message': str(e)}
                }
                print(json.dumps(error_response), flush=True)
            except Exception as e:
                error_response = {
                    'id': None,
                    'result': None,
                    'error': {'type': 'InternalError', 'message': str(e)}
                }
                print(json.dumps(error_response), flush=True)

if __name__ == '__main__':
    main()
