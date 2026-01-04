#!/usr/bin/env python3
"""
Optimized ClickHouse data loader that batches INSERT statements for better performance.
This script reduces the number of HTTP requests by batching multiple INSERT statements together.
"""

import sys
import re
import subprocess
import os

def batch_load_clickhouse(sql_file: str, clickhouse_url: str, batch_size: int = 100):
    """Load SQL file into ClickHouse using batched INSERT statements."""
    
    with open(sql_file, 'r') as f:
        content = f.read()
    
    # Parse statements
    statements = []
    current_statement = ''
    
    for line in content.split('\n'):
        line_stripped = line.strip()
        # Skip empty lines and comments
        if not line_stripped or line_stripped.startswith('--'):
            continue
        
        current_statement += line + '\n'
        
        # Check if statement is complete (ends with semicolon)
        if line_stripped.endswith(';'):
            statements.append(current_statement.strip())
            current_statement = ''
    
    # Process statements: batch INSERT statements, execute others immediately
    insert_batch = []
    table_name = None
    errors = []
    
    for stmt in statements:
        stmt_upper = stmt.upper().strip()
        
        # Execute CREATE/TRUNCATE/DROP immediately
        if any(stmt_upper.startswith(cmd) for cmd in ['CREATE', 'TRUNCATE', 'DROP']):
            # Flush pending batch first
            if insert_batch and table_name:
                batch_sql = f'INSERT INTO {table_name} VALUES {", ".join(insert_batch)};'
                result = subprocess.run(
                    ['curl', '-s', '-X', 'POST', clickhouse_url + '/', '--data-binary', batch_sql],
                    capture_output=True,
                    text=True
                )
                if result.returncode != 0 or 'Exception' in result.stdout:
                    errors.append(f"Batch insert failed: {result.stdout}")
                insert_batch = []
                table_name = None
            
            # Execute non-INSERT statement
            result = subprocess.run(
                ['curl', '-s', '-X', 'POST', clickhouse_url + '/', '--data-binary', stmt],
                capture_output=True,
                text=True
            )
            if result.returncode != 0 or 'Exception' in result.stdout:
                errors.append(f"Statement failed: {result.stdout[:200]}")
        
        # Batch INSERT statements
        elif stmt_upper.startswith('INSERT INTO'):
            # Extract table name and VALUES
            match = re.search(r'INSERT INTO (\w+)\s+.*?VALUES\s*\((.+)\);', stmt, re.DOTALL | re.IGNORECASE)
            if match:
                current_table = match.group(1)
                values = '(' + match.group(2) + ')'
                
                # If table changed, flush current batch
                if table_name is not None and table_name != current_table:
                    if insert_batch:
                        batch_sql = f'INSERT INTO {table_name} VALUES {", ".join(insert_batch)};'
                        result = subprocess.run(
                            ['curl', '-s', '-X', 'POST', clickhouse_url + '/', '--data-binary', batch_sql],
                            capture_output=True,
                            text=True
                        )
                        if result.returncode != 0 or 'Exception' in result.stdout:
                            errors.append(f"Batch insert failed: {result.stdout[:200]}")
                    insert_batch = []
                
                table_name = current_table
                insert_batch.append(values)
                
                # Flush batch when it reaches batch_size
                if len(insert_batch) >= batch_size:
                    batch_sql = f'INSERT INTO {table_name} VALUES {", ".join(insert_batch)};'
                    result = subprocess.run(
                        ['curl', '-s', '-X', 'POST', clickhouse_url + '/', '--data-binary', batch_sql],
                        capture_output=True,
                        text=True
                    )
                    if result.returncode != 0 or 'Exception' in result.stdout:
                        errors.append(f"Batch insert failed: {result.stdout[:200]}")
                    insert_batch = []
            else:
                # Couldn't parse, execute as-is
                result = subprocess.run(
                    ['curl', '-s', '-X', 'POST', clickhouse_url + '/', '--data-binary', stmt],
                    capture_output=True,
                    text=True
                )
                if result.returncode != 0 or 'Exception' in result.stdout:
                    errors.append(f"Insert failed: {result.stdout[:200]}")
    
    # Flush remaining batch
    if insert_batch and table_name:
        batch_sql = f'INSERT INTO {table_name} VALUES {", ".join(insert_batch)};'
        result = subprocess.run(
            ['curl', '-s', '-X', 'POST', clickhouse_url + '/', '--data-binary', batch_sql],
            capture_output=True,
            text=True
        )
        if result.returncode != 0 or 'Exception' in result.stdout:
            errors.append(f"Final batch insert failed: {result.stdout[:200]}")
    
    # Report errors
    if errors:
        for error in errors[:10]:  # Show first 10 errors
            print(f"Warning: {error}", file=sys.stderr)
        if len(errors) > 10:
            print(f"... and {len(errors) - 10} more errors", file=sys.stderr)
        return 1
    
    return 0


if __name__ == '__main__':
    if len(sys.argv) != 3:
        print(f"Usage: {sys.argv[0]} <sql_file> <clickhouse_url>", file=sys.stderr)
        sys.exit(1)
    
    sql_file = sys.argv[1]
    clickhouse_url = sys.argv[2]
    
    if not os.path.exists(sql_file):
        print(f"Error: SQL file not found: {sql_file}", file=sys.stderr)
        sys.exit(1)
    
    sys.exit(batch_load_clickhouse(sql_file, clickhouse_url))

