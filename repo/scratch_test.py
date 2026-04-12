import sqlite3
import os

db_path = "test.db"
if os.path.exists(db_path):
    os.remove(db_path)

conn = sqlite3.connect(db_path)
conn.execute("CREATE TABLE t (amount REAL)")
conn.execute("INSERT INTO t (amount) VALUES (?)", (500,))
conn.execute("INSERT INTO t (amount) VALUES (500)")
for row in conn.execute("SELECT amount, typeof(amount) FROM t"):
    print(row)
