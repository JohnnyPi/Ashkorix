import sqlite3
import sys

db = r"e:\CursorApps\YourAI\Data\ashkorix.db"
doc_id = sys.argv[1] if len(sys.argv) > 1 else "a7b14ec0ade8"
c = sqlite3.connect(db)
rows = c.execute(
    "SELECT id, chunk_index, section_title, heading_path, substr(text, 1, 120) "
    "FROM chunks WHERE document_id=? ORDER BY chunk_index",
    (doc_id,),
).fetchall()
for r in rows:
    print(r)
