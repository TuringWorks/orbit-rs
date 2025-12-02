import os
import sys
import mysql.connector
from mysql.connector import errorcode

# MySQL connection parameters
MYSQL_HOST = os.getenv("MYSQL_HOST", "localhost")
MYSQL_PORT = int(os.getenv("MYSQL_PORT", 3306))
MYSQL_USER = os.getenv("MYSQL_USER", "root")
MYSQL_PASSWORD = os.getenv("MYSQL_PASSWORD", "password")
MYSQL_DB = os.getenv("MYSQL_DB", "pci_test_db")

def run_checks():
    print("Running MySQL compatibility checks...")
    
    cnx = None
    cursor = None
    try:
        # 1. Connection Check
        print("1. Connection to MySQL...", end=" ")
        cnx = mysql.connector.connect(user=MYSQL_USER, password=MYSQL_PASSWORD,
                                      host=MYSQL_HOST, port=MYSQL_PORT)
        print(f"PASS (Server version: {cnx.get_server_info()})")
        
        cursor = cnx.cursor()
        
        # Setup database
        try:
            cursor.execute(f"CREATE DATABASE IF NOT EXISTS {MYSQL_DB}")
        except mysql.connector.Error as err:
            print(f"Failed creating database: {err}")
            sys.exit(1)
            
        cnx.database = MYSQL_DB
        
        checks = [
            ("Create Table", "CREATE TABLE IF NOT EXISTS employees (emp_no int NOT NULL AUTO_INCREMENT, first_name varchar(14), last_name varchar(16), hire_date date, PRIMARY KEY (emp_no))"),
            ("Insert Data", "INSERT INTO employees (first_name, last_name, hire_date) VALUES ('John', 'Doe', '2023-01-01')"),
            ("Select Data", "SELECT * FROM employees WHERE first_name = 'John'"),
            ("Update Data", "UPDATE employees SET first_name = 'Jane' WHERE emp_no = 1"),
            ("Delete Data", "DELETE FROM employees WHERE emp_no = 1"),
            ("Transaction", "START TRANSACTION; INSERT INTO employees (first_name, last_name, hire_date) VALUES ('Alice', 'Smith', '2023-02-01'); COMMIT;"),
            ("Show Tables", "SHOW TABLES"),
            ("Describe Table", "DESCRIBE employees"),
            ("Session Variables", "SET @my_var = 1; SELECT @my_var"),
        ]
        
        failed = 0
        for name, query in checks:
            print(f"Testing {name}...", end=" ")
            try:
                # Handle multiple statements if needed, but here we keep it simple
                for result in cursor.execute(query, multi=True):
                    pass
                print("PASS")
            except mysql.connector.Error as err:
                print(f"FAIL: {err}")
                failed += 1

        # Clean up
        cursor.execute(f"DROP DATABASE IF EXISTS {MYSQL_DB}")
        cnx.commit()

        if failed > 0:
            print(f"\n{failed} checks failed.")
            sys.exit(1)
        else:
            print("\nAll MySQL checks passed.")

    except mysql.connector.Error as err:
        print(f"\nCritical Error: {err}")
        sys.exit(1)
    finally:
        if cursor:
            cursor.close()
        if cnx:
            cnx.close()

if __name__ == "__main__":
    run_checks()
